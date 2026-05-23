# Key Design Rationale

This page explains the reasoning behind specific choices in SlateDuck's binary key encoding, beyond the general principles described in [Architecture: Key Layout](../architecture/key-layout.md).

## Why Tag-First?

The tag byte is the first byte of every key. This was chosen over alternatives (tag as suffix, tag in value, no tag) for prefix scan efficiency. When you want all rows of a specific table type (e.g., all schemas), you scan the prefix `0x04`. The tag byte acts as a perfect namespace separator — the scan never accidentally crosses into a different table type.

The alternative (hierarchical keys like `schema/1/table/42`) would require string parsing, variable-length keys, and delimiter handling. The fixed-byte tag approach is simpler, faster, and equally expressive.

## Why Big-Endian?

All multi-byte integers in keys are stored in big-endian (most significant byte first). This ensures that lexicographic byte comparison produces the same ordering as numeric comparison. When SlateDB iterates keys in sorted order, they come out in ascending ID order, which is the natural ordering for scan results.

If we used little-endian encoding (like x86 native), the key `0x0000000000000001` would sort AFTER `0x0000000000000100` because the first byte of 1 (0x01) is greater than the first byte of 256 (0x00 in big-endian, but 0x00 in little-endian it would be 0x00 and 0x01). This would make scan results unpredictable.

## Why Fixed-Width u64?

All ID fields (snapshot_id, schema_id, table_id, column_id, file_id) use 8-byte u64 even though most catalogs will never use values above a few million. This was chosen for:

**Simplicity:** Every key can be parsed with fixed offsets. No variable-length integer decoding, no length prefixes for ID fields, no special handling for large values.

**Uniformity:** All keys of the same tag have the same length, which simplifies debugging tools and size estimation.

**Future-proofing:** A u64 counter at one increment per second will not overflow for 584 billion years. There is no practical concern about running out of IDs.

The cost is 4 bytes of waste per ID field compared to a variable-length encoding. For a catalog with millions of rows, this adds up to a few MB — negligible relative to overall catalog size.

## Why begin_snapshot in Versioned Keys?

For versioned tables (schema, table, column, view, macro), the `begin_snapshot` is the last component of the key. This means multiple versions of the same logical entity have different keys and coexist as separate key-value pairs in SlateDB.

The alternative would be a single key per logical entity with multiple versions stored in the value (e.g., a list of versions). This was rejected because:

1. It would require read-modify-write for every version update (read the current list, append, write back) — not atomic without transactions
2. It would make prefix scans return a mix of current and historical data that must be parsed from within values rather than filtered at the key level
3. It would violate the principle that each key-value pair is independently readable

## Why Counters in a Separate Tag?

Counter values (next_snapshot_id, next_catalog_id, next_file_id) live under tag `0xFE` rather than being embedded in the system key space (`0xFF`). This is purely organizational: counters are updated on every write transaction (they are hot keys), while system keys are updated rarely. Keeping them in a separate tag makes monitoring and debugging clearer.

## Why System Keys Use String Suffixes?

Most keys use numeric components, but system keys use human-readable strings (`"writer-epoch"`, `"retain-from"`, `"hot-key"`). This was chosen because:

1. System keys are few in number (fewer than 20) and accessed by exact lookup, never by prefix scan
2. Human-readable suffixes make debugging with hex dumps trivial — you can see `FF 77 72 69 74 65 72 2D 65 70 6F 63 68` and immediately recognize "writer-epoch"
3. There is no performance concern because these keys are accessed by exact GET, not by sorted iteration
