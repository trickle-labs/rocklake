# Roadmap

This section outlines SlateDuck's development trajectory: where the project is headed, what has been accomplished, and what is planned for future releases.

## Current Status

SlateDuck is in active development (v0.8.x). The core architecture is stable, all major features are implemented, and the project is suitable for evaluation and development use. Production use is possible for teams comfortable with pre-1.0 software and willing to track releases closely.

## Near-Term (v0.9 - v0.10)

- **Performance optimization:** Secondary index implementation for hot-path queries, reducing scan amplification for large catalogs
- **Automated GC:** Built-in background garbage collection with configurable retention policies (no manual `slateduck gc` needed)
- **Enhanced monitoring:** OpenTelemetry tracing integration, distributed trace correlation
- **Encryption at rest:** AES-256-GCM encryption of value payloads before writing to object storage

## Medium-Term (v0.11 - v0.12)

- **Multi-catalog management:** A registry of catalogs with shared configuration and centralized monitoring
- **Partitioned writes:** Multiple independent writers for different datasets within one logical catalog
- **Connection pooling:** Built-in connection multiplexing for high-concurrency scenarios
- **S3 Express optimizations:** Specific optimizations leveraging S3 Express One Zone's directory semantics

## Long-Term (v1.0 and Beyond)

- **Stable catalog format:** Commit to format version 1 stability with long-term backward compatibility guarantees
- **Stable API:** Commit to the FFI and PG-wire interface stability
- **Ecosystem integrations:** Apache Iceberg metadata bridge, Delta Lake catalog compatibility
- **Cloud-managed service:** Hosted SlateDuck with automatic scaling, monitoring, and management

## Design Principles for Roadmap

All roadmap items are evaluated against these criteria:

1. **Does it simplify operations?** Features that reduce operational burden are prioritized.
2. **Does it maintain the single-binary promise?** Features that require additional infrastructure are deprioritized.
3. **Does it serve the DuckLake use case?** Features that generalize SlateDuck beyond its core purpose are carefully evaluated.
4. **Is it reversible?** We prefer decisions that can be undone if they turn out to be wrong.

## Contributing to the Roadmap

If you have a feature request or want to contribute to a roadmap item, open a GitHub Discussion. We prioritize based on real-world use cases — if you can describe a concrete scenario where a feature would help, it is much more likely to be implemented.
