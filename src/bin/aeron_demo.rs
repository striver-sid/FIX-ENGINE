/// Aeron transport demo — the standard/default colocated integration path.
///
/// Runs an initiator and acceptor FIX engine over the in-process Aeron-style
/// transport so the default integration can be exercised without any socket or
/// media-driver setup.
use std::io;
use std::thread;
use std::time::Duration;

use velocitas_fix::engine::{EngineContext, FixApp, FixEngine};
use velocitas_fix::message::MessageView;
use velocitas_fix::serializer;
use velocitas_fix::session::{SequenceResetPolicy, Session, SessionConfig, SessionRole};
use velocitas_fix::tags;
use velocitas_fix::timestamp::{HrTimestamp, TimestampSource};
use velocitas_fix::transport::{build_transport, TransportConfig};

fn main() {
    let stream_id = 4_242;

    let acceptor = thread::spawn(move || -> io::Result<()> {
        let mut transport = build_transport(TransportConfig::aeron_ipc(stream_id))?;
        transport.bind("127.0.0.1", 0)?;

        let session = Session::new(SessionConfig {
            session_id: "AERON-ACCEPTOR".into(),
            fix_version: "FIX.4.4".into(),
            sender_comp_id: "EXCHANGE".into(),
            target_comp_id: "BANK_OMS".into(),
            role: SessionRole::Acceptor,
            heartbeat_interval: Duration::from_secs(30),
            reconnect_interval: Duration::ZERO,
            max_reconnect_attempts: 0,
            sequence_reset_policy: SequenceResetPolicy::Daily,
            validate_comp_ids: true,
            max_msg_rate: 50_000,
        });

        let mut engine = FixEngine::new_acceptor(transport, session);
        let mut app = AcceptorApp;
        engine.run_acceptor(&mut app)
    });

    let initiator = thread::spawn(move || -> io::Result<()> {
        thread::sleep(Duration::from_millis(10));

        let mut transport = build_transport(TransportConfig::aeron_ipc(stream_id))?;
        transport.connect("127.0.0.1", 0)?;

        let session = Session::new(SessionConfig {
            session_id: "AERON-INITIATOR".into(),
            fix_version: "FIX.4.4".into(),
            sender_comp_id: "BANK_OMS".into(),
            target_comp_id: "EXCHANGE".into(),
            role: SessionRole::Initiator,
            heartbeat_interval: Duration::from_secs(30),
            reconnect_interval: Duration::from_secs(1),
            max_reconnect_attempts: 3,
            sequence_reset_policy: SequenceResetPolicy::Daily,
            validate_comp_ids: true,
            max_msg_rate: 50_000,
        });

        let mut engine = FixEngine::new_initiator(transport, session);
        let mut app = InitiatorApp;
        engine.run_initiator(&mut app)
    });

    initiator.join().unwrap().unwrap();
    acceptor.join().unwrap().unwrap();
}

struct AcceptorApp;

impl FixApp for AcceptorApp {
    fn on_logon(&mut self, _ctx: &mut EngineContext<'_>) -> io::Result<()> {
        println!("acceptor: logon acknowledged over Aeron");
        Ok(())
    }

    fn on_message(
        &mut self,
        msg_type: &[u8],
        msg: &MessageView<'_>,
        ctx: &mut EngineContext<'_>,
    ) -> io::Result<()> {
        if msg_type == b"D" {
            let ts = HrTimestamp::now(TimestampSource::System).to_fix_timestamp();
            let seq = ctx.next_seq_num();
            let sender = ctx.session().config().sender_comp_id.clone();
            let target = ctx.session().config().target_comp_id.clone();
            let cl_ord_id = msg.get_field(tags::CL_ORD_ID).unwrap_or(b"?");
            let symbol = msg.get_field(tags::SYMBOL).unwrap_or(b"?");
            let qty = msg.get_field_i64(tags::ORDER_QTY).unwrap_or(100);

            let mut buf = [0u8; 2048];
            let len = serializer::build_execution_report(
                &mut buf,
                b"FIX.4.4",
                sender.as_bytes(),
                target.as_bytes(),
                seq,
                &ts,
                b"AERON-ORD-1",
                b"AERON-EXEC-1",
                cl_ord_id,
                symbol,
                b'1',
                qty,
                qty,
                b"178.55",
                0,
                qty,
                b"178.55",
                b'F',
                b'2',
            );
            ctx.send_raw(&buf[..len])?;
            println!("acceptor: filled order via Aeron transport");
        }
        Ok(())
    }
}

struct InitiatorApp;

impl FixApp for InitiatorApp {
    fn on_logon(&mut self, ctx: &mut EngineContext<'_>) -> io::Result<()> {
        let ts = HrTimestamp::now(TimestampSource::System).to_fix_timestamp();
        let seq = ctx.next_seq_num();
        let sender = ctx.session().config().sender_comp_id.clone();
        let target = ctx.session().config().target_comp_id.clone();

        let mut buf = [0u8; 1024];
        let len = serializer::build_new_order_single(
            &mut buf,
            b"FIX.4.4",
            sender.as_bytes(),
            target.as_bytes(),
            seq,
            &ts,
            b"ORD-AERON-1",
            b"AAPL",
            b'1',
            1_000,
            b'2',
            b"178.55",
        );
        ctx.send_raw(&buf[..len])?;
        println!("initiator: sent NewOrderSingle over Aeron");
        Ok(())
    }

    fn on_message(
        &mut self,
        msg_type: &[u8],
        msg: &MessageView<'_>,
        ctx: &mut EngineContext<'_>,
    ) -> io::Result<()> {
        if msg_type == b"8" {
            let cl_ord_id = msg.get_field_str(tags::CL_ORD_ID).unwrap_or("?");
            println!("initiator: received ExecutionReport for {cl_ord_id}");
            ctx.request_stop();
        }
        Ok(())
    }
}
