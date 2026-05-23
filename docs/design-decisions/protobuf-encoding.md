# Protobuf Encoding

SlateDuck uses Protocol Buffers (protobuf) for serializing catalog row data within the value envelope. This page documents why protobuf was chosen over alternatives and what consequences follow.

## The Requirements

The serialization format needed to:

1. Be compact (catalogs can have millions of rows; each byte matters at scale)
2. Support schema evolution (add fields without breaking existing data)
3. Be fast to encode/decode (hot path for every catalog read)
4. Have robust Rust support (preferably code-generated, not hand-written)
5. Be deterministic (same input produces same bytes, for testing)

## Alternatives Considered

**JSON:** Human-readable, excellent tooling, but verbose (3-10x larger than binary formats), slow to parse, and no schema enforcement. Unsuitable for a hot-path serialization format where millions of rows may be scanned.

**MessagePack:** Compact binary format, schema-less. Faster than JSON but no built-in schema evolution story. Field renaming or reordering would break deserialization. Also lacks code generation — requires hand-written serialize/deserialize logic.

**FlatBuffers:** Zero-copy deserialization, very fast reads. However, complex API, poor ergonomics in Rust, and requires careful alignment handling. The zero-copy benefit is less relevant when values are already small (50-200 bytes).

**Cap'n Proto:** Similar to FlatBuffers with better schema evolution. However, the Rust implementation (`capnp`) has a complex API and limited ecosystem compared to `prost`.

**Raw struct serialization (bincode, postcard):** Extremely fast, very compact, but no schema evolution. Adding a field breaks all existing data. Unacceptable for a long-lived catalog that may span years of SlateDuck versions.

**Apache Avro:** Good schema evolution, compact, self-describing. But heavier runtime than protobuf, less mature Rust support, and the self-describing nature adds per-value overhead that is unnecessary when the schema is known at compile time.

## Why Protobuf Won

Protobuf hits the sweet spot across all requirements:

- **Compact:** Variable-length integer encoding, no field names in the wire format, no padding. Typical catalog rows are 50-200 bytes.
- **Schema evolution:** Fields are numbered, not named. Adding new fields (with new numbers) is always backward-compatible. Removing fields is forward-compatible (unknown fields are silently ignored by `prost`).
- **Fast:** `prost` generates zero-allocation encode/decode code. Benchmarks show sub-microsecond encode/decode for typical catalog rows.
- **Excellent Rust support:** `prost` is the de facto standard for protobuf in Rust. It generates idiomatic Rust structs with derive macros. The generated code is type-safe and allocation-free for most cases.
- **Deterministic:** `prost` encodes fields in field-number order with deterministic varint encoding. Same struct produces same bytes.

## Consequences

**Positive:**
- Catalog data written by SlateDuck v0.1 will be readable by future versions (forward compatibility via field numbering)
- Small catalog footprint (millions of rows fit in megabytes, not gigabytes)
- Fast scan performance (decode overhead is negligible compared to I/O)
- Type safety: the generated Rust code catches type errors at compile time

**Negative:**
- Not human-readable. Debugging requires decoding tools (the `inspect` command provides this).
- `.proto` schema must be maintained alongside the Rust types (currently expressed as `prost::Message` derives in `rows.rs` rather than separate `.proto` files — a pragmatic choice for a single-language project)
- Protobuf's optional field semantics mean every field is `Option<T>` in Rust, requiring explicit unwrapping even for fields that are always present

## The Envelope Wrapper

Raw protobuf bytes are wrapped in a 5-byte envelope (1 byte version + 4 bytes magic "SDKV") before writing to SlateDB. This adds negligible size overhead but provides corruption detection and version gating. See [Architecture: Value Encoding](../architecture/value-encoding.md) for details.
