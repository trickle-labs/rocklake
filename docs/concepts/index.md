# Concepts

This section explains the foundational ideas behind SlateDuck. Understanding these concepts will help you reason about the system's behavior, predict its performance characteristics, and make informed decisions about configuration and operations.

SlateDuck sits at the intersection of several well-understood ideas from database systems and distributed storage: immutable append-only data structures, multi-version concurrency control, LSM-tree storage engines, and the lakehouse architecture pattern. Each concept page takes one of these ideas and explains how SlateDuck applies it, what trade-offs result, and what that means for you as an operator or developer.

## Core Concepts

The concepts are organized from general to specific. Start with the high-level architectural ideas and work your way down to the implementation details:

- **[Bounded SQL](bounded-sql.md)** explains why SlateDuck does not implement a general SQL engine, what "bounded" means in practice, and how this design choice affects security, correctness, and performance.

- **[Catalog vs Data](catalog-vs-data.md)** draws the line between what SlateDuck manages (metadata) and what DuckDB manages (data), and explains why this separation matters for scalability and operational simplicity.

- **[Immutability](immutability.md)** describes the append-only data model that underpins time travel, crash safety, and horizontal read scale-out.

- **[Key-Value Mapping](key-value-mapping.md)** explains how relational catalog concepts (schemas, tables, columns) are encoded into a key-value store with lexicographically ordered keys.

- **[MVCC](mvcc.md)** covers multi-version concurrency control as applied to catalog entries: how visibility is determined, how versions accumulate, and how garbage collection reclaims space.

- **[Object Store Durability](object-store-durability.md)** explains why object storage is a good fit for catalog persistence, what durability guarantees you get, and how SlateDB bridges the gap between a key-value API and object storage semantics.

- **[Single Writer, Many Readers](single-writer-many-readers.md)** describes the concurrency model, why it was chosen, and how to work around its limitations through dataset partitioning.

- **[Snapshots](snapshots.md)** explains the snapshot model in detail: what a snapshot represents, how they are created, how they enable time travel, and how they interact with garbage collection.
