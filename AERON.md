# Aeron Integration Guide

This is the standard integration path for Velocitas.

Use Aeron when you are wiring colocated services into the FIX engine. Use the TCP client/server wrappers only when you explicitly need socket-based venue or counterparty connectivity.

## What "Aeron" Means Here

The crate exposes an `AeronTransport` backend and makes it the default transport selected by `TransportConfig::default()`.

The current implementation is an in-process Aeron-style IPC transport built into the crate. That keeps local integration simple:

- no socket listener setup
- no media-driver bootstrap step
- no extra dependency or daemon to start

Each FIX wire message is sent as one Aeron frame.

## Quick Start

Build the project with the default transport:

```bash
cargo build --release
```

Run the default Aeron demo:

```bash
cargo run --release --bin aeron_demo
```

Run the focused Aeron end-to-end regression test:

```bash
cargo test --test aeron_integration
```

## Default Behavior

These are the important defaults from `TransportConfig`:

- `TransportConfig::default()` selects `TransportMode::Aeron`
- the default channel is `aeron:ipc`
- the default stream ID is `1001`

If you want an explicit stream ID, use `TransportConfig::aeron_ipc(stream_id)`.

If you need TCP instead, use `TransportConfig::kernel_tcp()`.

## Standard Pattern

The simplest setup is:

1. Create an Aeron transport for the acceptor.
2. Bind it.
3. Create an Aeron transport for the initiator.
4. Connect it.
5. Create `Session` values for each side.
6. Pass both transports into `FixEngine`.

```rust
use std::io;
use std::time::Duration;

use velocitas_fix::engine::{FixApp, FixEngine};
use velocitas_fix::session::{SequenceResetPolicy, Session, SessionConfig, SessionRole};
use velocitas_fix::transport::{build_transport, TransportConfig};

fn build_acceptor() -> io::Result<FixEngine<Box<dyn velocitas_fix::transport::Transport>>> {
    let mut transport = build_transport(TransportConfig::aeron_ipc(1001))?;
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

    Ok(FixEngine::new_acceptor(transport, session))
}

fn build_initiator() -> io::Result<FixEngine<Box<dyn velocitas_fix::transport::Transport>>> {
    let mut transport = build_transport(TransportConfig::aeron_ipc(1001))?;
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

    Ok(FixEngine::new_initiator(transport, session))
}
```

## Initiator And Acceptor Behavior

There is one important difference between the Aeron path and the TCP wrapper path:

- with Aeron, `run_acceptor()` can consume the initial inbound Logon directly
- with the TCP wrappers, the server still uses the socket-oriented pre-read path before handing control to the engine

That means Aeron integrations can stay entirely on the `FixEngine` API without going through `FixServer`.

## Recommended Integration Shape

For most local application integrations:

1. Keep your business logic in a `FixApp` implementation.
2. Use `build_transport(TransportConfig::default())` or `aeron_ipc(stream_id)`.
3. Construct `FixEngine` directly.
4. Reserve `FixClient` and `FixServer` for TCP edge connections.

This keeps the colocated path consistent across tests, demos, and production embeddings.

## When To Use TCP Instead

Use the TCP wrappers when you need:

- a listening socket
- remote connectivity over TCP/IP
- compatibility with existing FIX counterparties

In that case, opt in explicitly:

```rust
use velocitas_fix::transport::TransportConfig;

let tcp = TransportConfig::kernel_tcp();
```

## Reference Files

- `src/transport.rs` — transport defaults, modes, and factory
- `src/transport_aeron.rs` — Aeron transport backend
- `src/engine.rs` — acceptor/initiator engine behavior
- `src/bin/aeron_demo.rs` — runnable end-to-end Aeron example
- `tests/aeron_integration.rs` — focused regression coverage
