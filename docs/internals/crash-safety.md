# Crash Safety

SlateDuck achieves crash safety without requiring an explicit recovery process. If the process crashes at any point during any operation, the catalog remains consistent — either the operation completed fully or it did not happen at all. This page explains how.

## The Foundation: Atomic PUT

Object storage provides atomic PUT semantics: a PUT either completes entirely or does not happen. There is no partial PUT. This is the fundamental building block of crash safety.

SlateDB leverages this by writing WAL segments as individual objects. Each WAL segment contains one or more write batches. If the process crashes before the PUT completes, the segment does not exist and the write batch is lost. If the PUT completes, the entire batch is durable.

## Write Path Crash Safety

A SlateDuck write transaction follows these steps:

1. Allocate a snapshot ID (increment counter)
2. Build all key-value pairs for the transaction (schema rows, table rows, column rows, etc.)
3. Create a SlateDB WriteBatch containing all pairs
4. Commit the WriteBatch (one atomic WAL segment PUT)

If the process crashes at step 1, 2, or 3: no WAL segment was written. The next writer will allocate a new snapshot ID (which may skip the crashed one — this is fine, gaps in snapshot IDs are harmless).

If the process crashes during step 4: the PUT either completed or it did not. If it completed, the transaction is durable. If it did not complete, the transaction never happened.

There is no in-between state where "half a transaction" is visible.

## Read Path Crash Safety

Readers access immutable SST files. A crash during a read simply terminates the read — no state is modified, no cleanup is needed. The next read starts fresh from the manifest.

## Compaction Crash Safety

SlateDB's compaction (merging SST files) is crash-safe because it follows the "new before old" pattern:

1. Write the new (merged) SST file
2. Update the manifest to reference the new file
3. Delete old SST files

If the process crashes at step 1: the new file is orphaned garbage (cleaned up by SlateDB's garbage collection).
If the process crashes at step 2: the manifest still references old files; the merge is retried.
If the process crashes at step 3: old files are garbage but harmless (cleaned up eventually).

## No WAL Replay

Unlike traditional databases (PostgreSQL, MySQL), SlateDuck does not have a WAL replay step on startup. There is nothing to replay because:

- Committed data is already in immutable SSTs
- Uncommitted data (incomplete WAL segments) never existed (the PUT did not complete)
- The manifest is always consistent (it is updated atomically)

This means startup is instantaneous: open the manifest, read the counter values, and start serving requests.

## Implications

- No `pg_resetwal` equivalent needed
- No "recovery mode" or "crash recovery time"
- No risk of WAL corruption causing unrecoverable state
- Restarting after a crash takes milliseconds, not minutes
