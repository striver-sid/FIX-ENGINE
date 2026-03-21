# Velocitas FIX Engine — Performance Benchmark Suite

## 1. Benchmark Methodology

### 1.1 Principles

- All benchmarks run on **isolated hardware** (no co-tenancy)
- CPU frequency scaling disabled (`performance` governor)
- Turbo boost disabled for deterministic results
- System warmed up for 60 seconds before measurement
- Results reported as: **min, mean, median, p50, p90, p99, p99.9, p99.99, max**
- HDR Histogram used for latency recording (1 ns resolution, range 1 ns – 1 s)
- Each benchmark runs for minimum 60 seconds or 10 million iterations

### 1.2 Reference Hardware

| Component | Specification |
|---|---|
| CPU | Intel Xeon w7-3465X, 28 cores, 2.5 GHz base |
| RAM | 256 GB DDR5-4800 ECC |
| NIC | Solarflare X2522-25G (SFN8522-PLUS) |
| Storage | Intel Optane P5800X 800 GB |
| OS | RHEL 9.3, kernel 5.14.0-362 |
| Tuning | isolcpus=4-27, nohz_full=4-27, rcu_nocbs=4-27 |

### 1.3 Comparison Targets

| Engine | Version | Notes |
|---|---|---|
| QuickFIX/J | 2.3.1 | Open source, Java |
| QuickFIX/n | 1.11.1 | Open source, .NET |
| OnixS FIX Engine | 4.12 | Commercial, C++ |
| Chronicle FIX | 2.25 | Commercial, Java (low-latency) |
| LSEG (Refinitiv) RASH | 8.4 | Commercial, C++ |
| **Velocitas** | **1.0** | **This engine** |

---

## 2. Benchmark Definitions

### BM-01: Message Parse Latency

**Objective:** Measure time to parse a FIX message from a byte buffer into an accessible field structure.

**Messages Tested:**
| Message | Tag 35 | Approx Size | Field Count |
|---|---|---|---|
| Heartbeat | 0 | 60 bytes | 6 |
| NewOrderSingle | D | 220 bytes | 22 |
| ExecutionReport | 8 | 450 bytes | 42 |
| MarketDataIncrementalRefresh | X | 1,200 bytes | 85 (3 groups) |
| Large ExecutionReport (fills) | 8 | 4,500 bytes | 200 (50 fills) |

**Procedure:**
1. Pre-generate 1 million valid FIX messages of each type
2. Load into contiguous memory buffer
3. Parse each message, accessing MsgType (35), SenderCompID (49), and one business field
4. Record latency per parse via `rdtsc` / `cntvct_el0`
5. Repeat 10 runs, report aggregate histogram

**Target Results:**

| Message | Velocitas Target | Expected Commercial Best |
|---|---|---|
| Heartbeat | ≤ 80 ns | ~200 ns |
| NewOrderSingle | ≤ 300 ns | ~800 ns |
| ExecutionReport | ≤ 500 ns | ~1,500 ns |
| MD Incremental Refresh | ≤ 900 ns | ~3,000 ns |
| Large ExecutionReport | ≤ 2,500 ns | ~8,000 ns |

---

### BM-02: Message Serialization Latency

**Objective:** Measure time to build a FIX message from field values into a wire-format byte buffer.

**Procedure:**
1. Pre-allocate output buffer
2. Build message by setting fields programmatically
3. Serialize to wire format (including BodyLength, CheckSum computation)
4. Record latency per serialization

**Target Results:**

| Message | Velocitas Target | Expected Commercial Best |
|---|---|---|
| Heartbeat | ≤ 50 ns | ~150 ns |
| NewOrderSingle | ≤ 250 ns | ~700 ns |
| ExecutionReport | ≤ 400 ns | ~1,200 ns |

---

### BM-03: End-to-End (Wire-to-Wire) Latency

**Objective:** Measure time from first byte received on NIC to last byte transmitted on NIC, processing a NewOrderSingle and responding with an ExecutionReport.

**Setup:**
- Dedicated load generator on separate machine, connected via direct crossover cable
- Hardware timestamping enabled on both NICs
- Capture timestamps at NIC level (not application level)

**Procedure:**
1. Load generator sends NewOrderSingle
2. Engine parses, invokes application callback (no-op acknowledgment)
3. Engine serializes and sends ExecutionReport
4. Measure delta between NIC RX timestamp and NIC TX timestamp

**Target Results:**

| Percentile | Velocitas Target | Expected Commercial Best |
|---|---|---|
| Median (p50) | ≤ 3.0 µs | ~10 µs |
| p90 | ≤ 4.0 µs | ~15 µs |
| p99 | ≤ 5.0 µs | ~25 µs |
| p99.9 | ≤ 6.5 µs | ~50 µs |
| p99.99 | ≤ 8.0 µs | ~100 µs |
| Max | ≤ 15.0 µs | ~500 µs |

---

### BM-04: Sustained Throughput

**Objective:** Measure maximum sustained message rate with ≤ 10 µs p99 latency.

**Procedure:**
1. Open 64 concurrent sessions
2. Send NewOrderSingle messages at increasing rates
3. Measure at each rate for 60 seconds
4. Record the maximum rate where p99 ≤ 10 µs

**Target Results:**

| Config | Velocitas Target | Expected Commercial Best |
|---|---|---|
| 1 session, 1 core | ≥ 2,000,000 msg/s | ~600,000 msg/s |
| 64 sessions, 4 cores | ≥ 6,000,000 msg/s | ~2,000,000 msg/s |
| 256 sessions, 8 cores | ≥ 10,000,000 msg/s | ~3,500,000 msg/s |

---

### BM-05: Burst Handling

**Objective:** Measure latency degradation under burst load (10x normal rate for 10 ms).

**Procedure:**
1. Establish baseline at 500,000 msg/s sustained
2. Inject burst of 5,000,000 msg/s for 10 ms (50,000 messages)
3. Measure latency during burst and recovery time to baseline latency

**Target Results:**

| Metric | Velocitas Target | Expected Commercial Best |
|---|---|---|
| p99 during burst | ≤ 15 µs | ~100 µs |
| Recovery time | ≤ 5 ms | ~50 ms |
| Messages dropped | 0 | 0 |

---

### BM-06: Session Establishment

**Objective:** Measure time from TCP connect to first application message exchange.

**Procedure:**
1. Initiator connects to acceptor
2. Logon handshake (Logon → Logon ack)
3. Send first NewOrderSingle
4. Measure total time from `connect()` to first ExecutionReport received

**Target Results:**

| Metric | Velocitas Target | Expected Commercial Best |
|---|---|---|
| Logon handshake | ≤ 50 µs | ~300 µs |
| Time to first trade | ≤ 100 µs | ~500 µs |
| 1000 sessions startup | ≤ 2 s | ~15 s |

---

### BM-07: Gap Fill / Recovery

**Objective:** Measure sequence number recovery performance.

**Procedure:**
1. Establish session, exchange 100,000 messages
2. Simulate disconnect with 1,000 message gap
3. Reconnect and measure time to recover all 1,000 messages

**Target Results:**

| Metric | Velocitas Target | Expected Commercial Best |
|---|---|---|
| Recovery rate | ≥ 100,000 msg/s | ~10,000 msg/s |
| Time for 1,000 msg gap | ≤ 10 ms | ~100 ms |
| Time for 100,000 msg gap | ≤ 1 s | ~10 s |

---

### BM-08: Memory Footprint

**Objective:** Measure RSS memory consumption under various session counts.

| Sessions | Velocitas Target | Expected Commercial Best |
|---|---|---|
| 1 session, idle | ≤ 50 MB | ~200 MB |
| 100 sessions, active | ≤ 500 MB | ~2 GB |
| 1000 sessions, active | ≤ 4 GB | ~15 GB |

---

### BM-09: Coordinated Omission Resistance

**Objective:** Verify the engine doesn't suffer from coordinated omission bias in benchmarks.

**Procedure:**
1. Use Gil Tene's wrk2-style constant-rate load generation
2. Compare service time vs. response time histograms
3. Inject artificial 1 ms pause every 10 seconds
4. Verify histogram correctly captures pause impact

**Acceptance:** Response-time p99.99 must reflect injected pauses. Engine must not "hide" latency through backpressure-induced coordinated omission.

---

### BM-10: Jitter Analysis

**Objective:** Characterize latency distribution shape and identify jitter sources.

**Measurements:**
- Allan deviation of per-message latency
- Autocorrelation of latency time series
- Spectral analysis (FFT) to identify periodic jitter sources
- Min-max range over 1-second windows

**Target:** Coefficient of variation (σ/μ) ≤ 0.3 under sustained load.

---

## 3. Benchmark Results Template

```
=============================================================
VELOCITAS FIX ENGINE — BENCHMARK REPORT
=============================================================
Date:        2026-03-21
Engine:      Velocitas v1.0.0 (commit: abc1234)
Hardware:    Intel Xeon w7-3465X / 256 GB DDR5 / X2522 NIC
OS:          RHEL 9.3, kernel 5.14.0-362.el9
Transport:   DPDK 23.11
Cores:       4–11 (isolated)
Duration:    60 seconds per benchmark
Iterations:  10,000,000 per benchmark
=============================================================

BM-01: Parse Latency (NewOrderSingle, 220 bytes)
-------------------------------------------------------------
  Min:       _____ ns
  Mean:      _____ ns
  Median:    _____ ns
  p90:       _____ ns
  p99:       _____ ns
  p99.9:     _____ ns
  p99.99:    _____ ns
  Max:       _____ ns
  Samples:   10,000,000
  
  vs QuickFIX/J:     ___x faster (median)
  vs OnixS:          ___x faster (median)
  vs Chronicle FIX:  ___x faster (median)

BM-03: Wire-to-Wire Latency (NOS → ExecRpt, DPDK)
-------------------------------------------------------------
  Min:       _____ µs
  Mean:      _____ µs
  Median:    _____ µs
  p90:       _____ µs
  p99:       _____ µs
  p99.9:     _____ µs
  p99.99:    _____ µs
  Max:       _____ µs
  
  vs QuickFIX/J:     ___x faster (p99)
  vs OnixS:          ___x faster (p99)
  vs Chronicle FIX:  ___x faster (p99)

BM-04: Sustained Throughput (64 sessions, 4 cores)
-------------------------------------------------------------
  Max rate at p99 ≤ 10 µs: _____ msg/s
  
  vs QuickFIX/J:     ___x higher throughput
  vs OnixS:          ___x higher throughput  
  vs Chronicle FIX:  ___x higher throughput
=============================================================
```

---

## 4. Continuous Benchmark Pipeline

### 4.1 Nightly Benchmarks

- Run full benchmark suite (BM-01 through BM-10) nightly on dedicated hardware
- Store results in time-series database (InfluxDB/TimescaleDB)
- Alert on ≥ 5% regression in any p99 metric
- Grafana dashboard for historical trend analysis

### 4.2 PR Gate Benchmarks

- Run BM-01 (parse) and BM-02 (serialize) on every PR
- Compare against `main` branch baseline
- Block merge if median regresses by ≥ 3%
- Run on GitHub Actions with `--release` profile and `RUSTFLAGS="-C target-cpu=native"`

### 4.3 Quarterly Competitive Analysis

- Re-run all benchmarks against latest versions of comparison engines
- Publish internal report with updated comparison tables
- Track competitive position over time
