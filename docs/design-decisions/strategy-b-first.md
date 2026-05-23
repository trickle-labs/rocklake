# Strategy B First

SlateDuck supports three deployment strategies: Strategy B (PG-wire sidecar), Strategy C (native DuckDB extension via FFI), and DataFusion integration. The project chose to build and stabilize Strategy B first, even though Strategy C offers better raw performance. This page explains why.

## The Three Strategies

**Strategy B — PG-Wire Sidecar:** A standalone process that speaks PostgreSQL wire protocol. DuckDB connects to it over TCP like any PostgreSQL server. Clear process boundary, independent lifecycle, language-agnostic protocol.

**Strategy C — Native Extension:** A shared library (`.so`/`.dylib`/`.dll`) loaded into DuckDB's process. Catalog operations are in-process function calls. No network overhead, no serialization, microsecond latency.

**DataFusion Integration:** A Rust library providing DataFusion's `CatalogProvider` trait. For Rust applications using DataFusion directly, without DuckDB in the picture.

## Why Strategy B First?

The decision was driven by five factors:

**Debugging and observability.** A standalone process with its own logging, metrics, and lifecycle is dramatically easier to debug than code running inside DuckDB's process. During development, being able to attach a debugger to SlateDuck independently, inspect its memory, and restart it without affecting DuckDB was invaluable.

**Protocol validation.** Strategy B forced us to implement the complete PostgreSQL wire protocol interaction correctly. This protocol implementation would be needed eventually anyway (for pg-tide-relay and other PG-compatible clients). Building it first meant we validated the protocol semantics thoroughly before adding the complexity of FFI.

**DuckDB version independence.** A sidecar communicates over a stable protocol (PostgreSQL wire format). It does not link against DuckDB's internal APIs, does not need to match DuckDB's build system, and does not break when DuckDB releases a new version. Strategy C, by contrast, must match DuckDB's extension ABI exactly — a brittle coupling that requires coordinated releases.

**Deployment flexibility.** A sidecar runs anywhere: containers, VMs, serverless functions, different machines than DuckDB. It can be upgraded independently of DuckDB. It can serve multiple DuckDB instances simultaneously. Strategy C is locked to DuckDB's process and lifecycle.

**Correctness before performance.** Strategy B has higher latency per operation (network round-trip) but identical correctness requirements. By building the slower path first and making it correct, we established a reference implementation against which Strategy C can be validated. Any difference in behavior between B and C is a bug in C.

## The Cost

Strategy B adds network latency to every catalog operation. For a typical DuckDB session that issues 10-50 catalog queries to plan and execute a query, this adds 5-25ms of overhead (assuming local network). For batch operations that register hundreds of files, the overhead is more significant.

However, this overhead is dwarfed by the object storage latency already inherent in SlateDuck's design. A catalog read that takes 50ms in SlateDB (S3 round-trip) takes 51ms with network overhead — a negligible difference.

## Strategy C Status

Strategy C (the native extension via slateduck-ffi) is implemented and functional. It provides the same catalog operations as Strategy B without network overhead. It is appropriate for deployments where:

- Latency is critical (sub-millisecond catalog operations)
- DuckDB and SlateDuck have the same lifecycle (start/stop together)
- The operational simplicity of a single process outweighs the debugging benefits of separation
