# Value Encoding

Every value stored in SlateDuck's catalog is wrapped in a lightweight envelope format that provides corruption detection, forward compatibility, and efficient serialization. The actual row data is encoded using Protocol Buffers (protobuf), which gives SlateDuck schema evolution capabilities and compact binary representation.

## Envelope Format

All values follow this byte layout:

```
┌─────────────────────┬──────────────┬─────────────────────────┐
│ encoding_version: 1B│ magic: 4B    │ payload: variable       │
│ (currently 0x01)    │ "SDKV"       │ (protobuf or raw bytes) │
└─────────────────────┴──────────────┴─────────────────────────┘
```

**Encoding version (1 byte):** Currently `0x01`. Allows future format changes without breaking existing readers. A reader encountering an unknown version fails with `UnsupportedVersion` rather than silently misinterpreting data.

**Magic bytes (4 bytes):** The ASCII string `SDKV` (SlateDuck Key-Value). Serves as a corruption canary: if these bytes are not present at the expected offset, the value has been corrupted (bit-flip, truncation, or wrong data). The magic check provides early detection of storage corruption before attempting protobuf deserialization.

**Payload (variable):** The actual row data, typically serialized as a protobuf message. The payload format depends on the key's tag: tag `0x05` values contain `TableRow` protobuf messages, tag `0x06` values contain `ColumnRow` messages, and so on.

## Why Protobuf?

SlateDuck uses Protocol Buffers for row serialization because they provide several properties that matter for a long-lived catalog:

**Compact binary encoding.** Protobuf uses variable-length integer encoding, field tags, and no padding. A typical TableRow is 50-100 bytes. A ColumnRow is 30-80 bytes. A DataFileRow is 80-200 bytes. This compactness matters because catalogs can contain millions of rows.

**Forward and backward compatibility.** Protobuf's field numbering and optional fields allow the row schema to evolve without breaking existing data. New fields can be added (old readers ignore them). Old fields can be deprecated (new readers handle their absence). This is critical for a catalog that may contain data written by older versions of SlateDuck.

**Fast serialization/deserialization.** Protobuf encoding and decoding is O(n) in the size of the data with no parsing overhead (unlike JSON or XML). For SlateDuck's hot path (scanning hundreds of data file rows to answer a query), deserialization speed matters.

**Language-neutral schema definition.** While SlateDuck is written in Rust, the protobuf schemas could be used to read catalog data from other languages if needed (Python tooling, Go monitoring, etc.).

## Row Types

Each tag has a corresponding protobuf message type. The full set includes:

- `MetadataRow` — Key-value metadata (scope, key, value)
- `SnapshotRow` — Snapshot record (id, schema_version, time, author, message)
- `SnapshotChangesRow` — Per-snapshot change log
- `SchemaRow` — Schema definition (id, name, begin/end snapshot)
- `TableRow` — Table definition (id, schema_id, name, data_path, begin/end snapshot)
- `ColumnRow` — Column definition (id, table_id, name, type, index, nullable, default, begin/end)
- `ViewRow`, `MacroRow`, `MacroImplRow`, `MacroParametersRow` — View and macro definitions
- `DataFileRow` — Data file registration (id, table_id, path, format, row_count, size, snapshot_id)
- `DeleteFileRow` — Delete file registration
- `TableStatsRow` — Aggregate table statistics (row_count, file_count, total_size)
- `FileColumnStatsRow` — Per-column per-file statistics (min, max, null_count, has_nan)
- `InlinedInsertRow`, `InlinedDeleteRow` — Small row data stored directly in catalog
- `HotKeyValue` — Packed current state for cold-start optimization
- `SecondaryIndexEntry` — File lookup acceleration

## Special Value Types

Not all values use protobuf. Some use simpler encodings:

**Counter values:** `encoding_version | magic | u64_big_endian`. Counters store a single 64-bit integer representing the next available ID. The big-endian encoding is not strictly necessary for values (only keys need lexicographic ordering) but is used for consistency.

**Format version:** `encoding_version | magic | u32_big_endian`. The catalog format version (currently 1).

**Raw values:** `encoding_version | magic | arbitrary_bytes`. Used for audit log entries (JSON) and metadata values where protobuf would be overkill.

## Size Limits

SlateDuck enforces a maximum value size of 64 MiB (`MAX_INLINED_VALUE_SIZE`). This limit exists because SlateDB writes values to WAL segments and SSTs that have practical size constraints. In practice, catalog values are tiny (under 1 KB) with the exception of inlined data rows, which can be up to 64 MiB for storing small tables directly in the catalog without Parquet files.

If a write would exceed 64 MiB, it fails with `ValueTooLarge` (SQLSTATE `54001`).

## Corruption Detection

The envelope format provides three layers of corruption detection:

1. **Magic bytes:** If `SDKV` is not present at bytes 1-4, the value is corrupt. This catches random bit-flips, zero-fills, and cross-contamination between unrelated data.

2. **Version check:** If the encoding version is not recognized, the value was written by an incompatible future version. Fail loudly rather than misinterpreting data.

3. **Protobuf parsing:** If the magic and version pass but the protobuf bytes do not decode correctly, the payload is corrupt. Protobuf has internal consistency checks (field types, lengths) that catch many corruption patterns.

If corruption is detected, SlateDuck surfaces it as a `ValueError` with the specific failure mode (InvalidMagic, UnsupportedVersion, or DecodeError). The `verify` and `repair` tools use these error types to identify and classify corruption.
