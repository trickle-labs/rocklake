# Design Decisions

Every system is the product of trade-offs. This section documents the major architectural choices made in SlateDuck, the alternatives that were considered, the reasoning behind each decision, and the consequences (both positive and negative) that follow. These pages are written with honesty about costs, not just benefits — understanding what you give up is as important as understanding what you get.

Reading these pages will help you evaluate whether SlateDuck is appropriate for your use case, predict its behavior in edge cases, and understand why certain limitations exist.

## Decision Pages

- **[Why SlateDB?](why-slatedb.md)** — The choice of persistence engine and what it means for durability, performance, and operational model.
- **[Strategy B First](strategy-b-first.md)** — Why the PG-wire sidecar was built before the native extension, and what that prioritization reveals about design values.
- **[Bounded SQL](bounded-sql.md)** — The decision to support only a finite set of SQL statements rather than implementing a general query engine.
- **[Protobuf Encoding](protobuf-encoding.md)** — Why Protocol Buffers for value serialization instead of JSON, MessagePack, FlatBuffers, or raw structs.
- **[Immutability Trade-offs](immutability-tradeoffs.md)** — The costs of never modifying data in place and how they are managed.
- **[Single-Writer Model](single-writer.md)** — The choice of serialized writes and its implications for throughput and availability.
- **[Key Design Rationale](key-design-rationale.md)** — The reasoning behind the specific binary key encoding scheme.
- **[What SlateDuck Is Not](what-slateduck-is-not.md)** — Explicit non-goals that shaped the architecture.
