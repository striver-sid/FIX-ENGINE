use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::transport::{Transport, TransportConfig, TransportEvent};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ChannelKey {
    channel: String,
    stream_id: i32,
}

struct AeronFrame {
    sender_id: u64,
    payload: Vec<u8>,
}

#[derive(Default)]
struct AeronBus {
    frames: VecDeque<AeronFrame>,
}

type SharedBus = Arc<Mutex<AeronBus>>;

static BUS_REGISTRY: OnceLock<Mutex<HashMap<ChannelKey, SharedBus>>> = OnceLock::new();
static NEXT_ENDPOINT_ID: AtomicU64 = AtomicU64::new(1);

fn registry() -> &'static Mutex<HashMap<ChannelKey, SharedBus>> {
    BUS_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Aeron-style transport for the standard colocated integration path.
///
/// This implementation intentionally keeps the surface area small: each FIX wire
/// message is transported as a single Aeron frame on an IPC-style channel.
pub struct AeronTransport {
    endpoint_id: u64,
    connected: bool,
    config: TransportConfig,
    bus: Option<SharedBus>,
}

impl AeronTransport {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            endpoint_id: NEXT_ENDPOINT_ID.fetch_add(1, Ordering::Relaxed),
            connected: false,
            config,
            bus: None,
        }
    }

    fn bus(&self) -> io::Result<&SharedBus> {
        self.bus.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "Aeron transport is not connected",
            )
        })
    }

    fn channel_key(&self, address: &str, port: u16) -> ChannelKey {
        let channel = self
            .config
            .aeron_channel
            .clone()
            .unwrap_or_else(|| format!("aeron:udp?endpoint={address}:{port}"));

        ChannelKey {
            channel,
            stream_id: self.config.aeron_stream_id,
        }
    }

    fn open(&mut self, address: &str, port: u16) {
        let key = self.channel_key(address, port);
        let bus = {
            let mut registry = registry().lock().unwrap();
            registry
                .entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(AeronBus::default())))
                .clone()
        };

        self.bus = Some(bus);
        self.connected = true;
    }
}

impl Transport for AeronTransport {
    fn connect(&mut self, address: &str, port: u16) -> io::Result<()> {
        self.open(address, port);
        Ok(())
    }

    fn bind(&mut self, address: &str, port: u16) -> io::Result<()> {
        self.open(address, port);
        Ok(())
    }

    fn send(&mut self, data: &[u8]) -> io::Result<usize> {
        let bus = self.bus()?.clone();
        let mut bus = bus.lock().unwrap();
        bus.frames.push_back(AeronFrame {
            sender_id: self.endpoint_id,
            payload: data.to_vec(),
        });
        Ok(data.len())
    }

    fn recv(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let bus = self.bus()?.clone();
        let mut bus = bus.lock().unwrap();
        let Some(index) = bus
            .frames
            .iter()
            .position(|frame| frame.sender_id != self.endpoint_id)
        else {
            return Ok(0);
        };

        let frame = bus.frames.remove(index).unwrap();
        if frame.payload.len() > buffer.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "receive buffer too small for Aeron frame",
            ));
        }

        buffer[..frame.payload.len()].copy_from_slice(&frame.payload);
        Ok(frame.payload.len())
    }

    fn close(&mut self) -> io::Result<()> {
        self.connected = false;
        Ok(())
    }

    fn poll(&mut self) -> io::Result<Option<TransportEvent>> {
        let bus = self.bus()?.clone();
        let bus = bus.lock().unwrap();
        Ok(bus
            .frames
            .iter()
            .find(|frame| frame.sender_id != self.endpoint_id)
            .map(|frame| TransportEvent::DataReceived(frame.payload.len())))
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI32, Ordering};

    use super::*;

    static NEXT_STREAM_ID: AtomicI32 = AtomicI32::new(20_000);

    fn test_config() -> TransportConfig {
        TransportConfig::aeron_ipc(NEXT_STREAM_ID.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn test_aeron_transport_round_trip() {
        let config = test_config();
        let mut initiator = AeronTransport::new(config.clone());
        let mut acceptor = AeronTransport::new(config);

        initiator.connect("127.0.0.1", 0).unwrap();
        acceptor.bind("127.0.0.1", 0).unwrap();

        initiator
            .send(b"8=FIX.4.4\x019=5\x0135=0\x0110=000\x01")
            .unwrap();

        let mut buf = [0u8; 128];
        let n = acceptor.recv(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"8=FIX.4.4\x019=5\x0135=0\x0110=000\x01");
    }

    #[test]
    fn test_aeron_transport_does_not_receive_own_frames() {
        let config = test_config();
        let mut initiator = AeronTransport::new(config.clone());
        let mut acceptor = AeronTransport::new(config);

        initiator.connect("127.0.0.1", 0).unwrap();
        acceptor.bind("127.0.0.1", 0).unwrap();

        initiator.send(b"ping").unwrap();

        let mut buf = [0u8; 16];
        assert_eq!(initiator.recv(&mut buf).unwrap(), 0);
        assert_eq!(acceptor.recv(&mut buf).unwrap(), 4);
    }
}
