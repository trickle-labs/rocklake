# Upgrades

This page covers upgrading SlateDuck to new versions, including version compatibility guarantees, the upgrade procedure, and rollback strategies.

## Version Compatibility

SlateDuck follows semantic versioning. The catalog format version (stored as a system key) determines binary compatibility:

- **Same format version:** Any binary that supports that format version can read/write the catalog
- **New format version:** Only binaries that know the new format can read it; old binaries refuse with `FormatVersionMismatch`

Currently there is only format version 1. Future versions will include migration tooling.

## Upgrade Procedure

1. Take an NDJSON backup: `slateduck export --storage s3://bucket/catalog/ --output pre-upgrade.ndjson`
2. Stop the SlateDuck process
3. Replace the binary with the new version
4. Start the new version: it will detect the existing catalog and resume
5. Verify: `slateduck inspect --storage s3://bucket/catalog/`

If the new version requires a format migration, it will either perform it automatically on startup or provide a separate migration command (documented in release notes).

## Rollback

If problems occur after upgrade:
- If no format migration occurred: simply replace the binary with the old version
- If a format migration occurred: restore from the NDJSON backup taken in step 1
