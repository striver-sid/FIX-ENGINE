# Velocitas FIX Engine — Simple Explanation

## What Is This Repository About? (The Big Picture)
This repository contains a **software engine** (a powerful program) written in **Rust** (a fast, safe programming language). The engine is called **Velocitas FIX Engine**, and it's designed to handle **FIX protocol** messages super quickly and reliably.

- **FIX Protocol**: Think of FIX as a "language" that computers in the financial world (like stock exchanges, banks, and brokers) use to talk to each other. It's like a standardized way to send messages about buying/selling stocks, checking prices, or confirming trades. For example, a message might say: "Buy 100 shares of Apple stock at $150 each." FIX messages are text-based, with codes like `35=D` (meaning "this is a new order") and `55=AAPL` (the stock symbol).

- **Why "Trading Engine"?**: In stock markets, trades happen in milliseconds. This engine processes these FIX messages at lightning speed (e.g., handling millions of messages per second) without wasting memory or getting stuck. It's built for **institutional trading**—big players like hedge funds or banks that need ultra-fast, error-free systems to avoid losing money.

The repository is open-source, meaning anyone can use or study the code. It's not a full trading app but a "building block" that developers can integrate into their own trading systems.

## What Does the Code Do? (Core Functionality)
The engine acts like a **high-speed translator and manager** for FIX messages. Here's what it handles in simple terms:

- **Parsing Messages**: When a FIX message arrives (as text), the engine reads it instantly and turns it into usable data. For example, it extracts the stock symbol, quantity, and price from the message. It does this **zero-allocation** (no extra memory waste) and uses **SIMD** (special CPU tricks) for speed—parsing a message in under 1 microsecond.

- **Serializing Messages**: The opposite—taking data (like an order) and turning it into a FIX message to send out. Again, super fast (e.g., 28 nanoseconds for a simple "heartbeat" message).

- **Managing Sessions**: FIX requires "sessions" like phone calls. The engine handles logging in/out, sending "heartbeats" (to keep the connection alive), and fixing errors if messages arrive out of order. It ensures everything is sequenced correctly.

- **Handling Transports**: Messages travel over networks. The engine supports:
  - **TCP**: Standard internet connections (like how websites work).
  - **Aeron IPC**: Fast, shared-memory communication for systems on the same machine (no network delays).
  - **DPDK**: Ultra-fast network cards that bypass slow parts of the operating system.

- **Advanced Features**:
  - **Repeating Groups**: For complex messages, like orders with multiple legs (e.g., "buy Apple and sell Google at the same time").
  - **Metrics and Monitoring**: Tracks performance (e.g., "how many messages per second?") and exposes it via a web dashboard or Prometheus (a monitoring tool).
  - **Journaling**: Saves messages to disk for recovery if the system crashes.
  - **Clustering**: Runs multiple copies for high availability (no single point of failure).
  - **Dictionary Compiler**: Reads FIX "dictionaries" (rules for message formats) and compiles them into fast lookup tables.

- **Demos and Tests**: The code includes examples (like the TCP demo we ran) to show it working, plus benchmarks to prove its speed.

In short, it's like a **super-efficient mailroom** for financial messages: receives, sorts, processes, and sends them without delays or mistakes.

## Why Is This Important? (Real-World Use)
- **Speed Matters in Trading**: A delay of 1 millisecond can cost millions. This engine achieves sub-microsecond latency (e.g., 15 microseconds for a full trade round-trip).
- **Reliability**: Zero bugs from memory issues (thanks to Rust), lock-free design (no slowdowns from waiting), and crash recovery.
- **Scalability**: Handles 2+ million messages per second on a single core.
- **Compliance**: Follows rules like MiFID II (EU trading laws) and SEC rules (US regulations).
- **Use Cases**: Banks use it for order routing, exchanges for matching trades, or HFT (high-frequency trading) firms for rapid decisions.

It's not for casual users—it's for pros building mission-critical systems.

## How Does It Work? (Architecture Overview)
The code is layered like an onion (from the README):

- **Transport Layer**: Handles how messages travel (TCP, Aeron, etc.).
- **Session Layer**: Manages connections, logins, and sequencing.
- **Message Layer**: Parses/serializes FIX data.
- **Application Layer**: High-level logic like order routing or risk checks.

Key design principles:
- **Zero-Allocation Hot Path**: No memory creation during busy times—pre-allocates pools.
- **Lock-Free Concurrency**: Multiple threads work without blocking each other.
- **Mechanical Sympathy**: Code optimized for how CPUs and memory work (e.g., cache-friendly layouts).
- **Deterministic Execution**: Predictable performance, no garbage collection pauses.

The code uses Rust's safety features to prevent crashes, with extensive tests (unit, integration, fuzzing).

## Prerequisites to Understand It Theoretically
To grasp this deeply, you need some background knowledge. Here's a list in order of importance (start with basics):

- **Basic Programming**: Know variables, loops, functions. Rust is the language, so learn Rust basics (ownership, borrowing) via the official Rust book (free online).

- **Networking Fundamentals**: Understand TCP/IP (how data travels over networks), sockets, and client-server models. FIX is network-based.

- **FIX Protocol Knowledge**:
  - What FIX is: Read the FIX Trading Community's intro (fixtrading.org).
  - Message Structure: Tags (e.g., 35=MsgType), values, delimiters (SOH character).
  - Sessions: Logon, Heartbeat, Logout flows.
  - Books: "FIX Protocol" by Orenstein or online specs.

- **Financial Trading Basics**:
  - Orders: Market, limit, stop-loss.
  - Executions: Fills, partial fills.
  - Concepts: Latency, throughput, HFT.

- **Performance Concepts**:
  - Latency vs. Throughput: Why microseconds matter.
  - Memory Management: Allocation, pools, garbage collection.
  - Concurrency: Threads, locks, lock-free algorithms.
  - SIMD: Vectorized processing for speed.

- **Rust-Specific**:
  - Async Programming: For non-blocking I/O.
  - Unsafe Code: For performance hacks (used sparingly here).
  - Cargo: Rust's build tool.

- **Advanced Topics** (Optional):
  - Aeron IPC: Shared memory messaging.
  - DPDK: Kernel-bypass networking.
  - Prometheus Metrics: Monitoring systems.
  - Raft Consensus: For clustering.

Start with the Rust book and FIX docs. The repository's README, SPECIFICATION.md, and BENCHMARKS.md are great for diving in.