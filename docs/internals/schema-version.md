# Schema Version

SlateDuck stores a format version in the catalog to ensure that binaries do not accidentally read or write incompatible data. This page documents how format versions work, what triggers a version change, and how migrations are handled.

## Current Format Version

The current catalog format version is **1**. This version has been stable since SlateDuck's initial release and encompasses:

- The tag allocation scheme (which byte values mean which table types)
- The key encoding format (tag + big-endian u64 components)
- The value envelope format (1 byte version + 4 bytes magic "SDKV" + protobuf payload)
- The protobuf field numbering for each row type
- The system key naming and semantics

## How It Is Stored

The format version is stored as a system key:

```
Key:   0xFF | "catalog-format-version"
Value: SDKV envelope containing the integer 1
```

Every SlateDuck binary reads this key on startup. If the value does not match the binary's expected version, the binary refuses to operate with error `FormatVersionMismatch` (SQLSTATE 0A000).

## What Triggers a Version Bump

A new format version would be required for:

- Changing the tag allocation (reassigning what a tag byte means)
- Changing the key encoding scheme (e.g., switching from big-endian to varint)
- Changing the value envelope format
- Renumbering protobuf fields in row types (breaking deserialization)
- Changing system key semantics in an incompatible way

The following do NOT require a version bump:

- Adding new tags (expansion into unused tag space)
- Adding new protobuf fields to existing row types (protobuf is forward-compatible)
- Adding new system keys
- Changing application logic without changing the storage format

## Migration Process

When a format version change is necessary (expected to be rare), the migration process will be:

1. The new binary detects the old format version
2. It either migrates in-place (updating the format version key after transforming data) or requires an explicit migration command
3. After migration, the catalog is only readable by the new binary version
4. Rollback requires restoring from a pre-migration backup

## Forward Compatibility

Adding new protobuf fields is always forward-compatible: old binaries simply ignore unknown field numbers. This means most catalog enhancements (adding new columns to existing tables, new statistics fields, etc.) can be done without a format version change.
