# Transaction Model

SlateDuck implements a simple but effective transaction model that maps DuckDB's logical catalog transactions to batched atomic writes in SlateDB. Transactions provide atomicity (all operations succeed or none do), consistency (MVCC invariants are maintained), isolation (readers at different snapshots see consistent views), and durability (committed transactions survive crashes).

## How DuckDB Uses Transactions

When DuckDB's `ducklake` extension performs a catalog mutation (creating a table, registering data files, updating statistics), it wraps the operation in a transaction:

```
BEGIN;
INSERT INTO ducklake_table (...) VALUES (...);
INSERT INTO ducklake_column (...) VALUES (...);  -- repeated for each column
INSERT INTO ducklake_column (...) VALUES (...);
INSERT INTO ducklake_snapshot (...) VALUES (...);
INSERT INTO ducklake_snapshot_changes (...) VALUES (...);
COMMIT;
```

This entire sequence should either succeed completely (all rows visible at the new snapshot) or fail completely (no partial state visible). SlateDuck guarantees this.

## Transaction Buffering

When SlateDuck receives a `BEGIN`, it transitions the session into "in transaction" state. Subsequent write statements are not immediately applied to the catalog. Instead, they are buffered in a `PendingCatalogTxn` structure:

```
PendingCatalogTxn {
    ops: Vec<BufferedOp>,      // All write operations in order
    estimated_size: usize,     // Running total of serialized size
}
```

Each `BufferedOp` represents one catalog write operation with all its parameters extracted and validated. The buffer accumulates operations until `COMMIT` is received.

## The Commit Sequence

On `COMMIT`, SlateDuck executes the following sequence atomically:

1. **Acquire the catalog mutex.** Only one transaction can commit at a time (single-writer).

2. **Check writer epoch.** Verify that this writer is still the authorized writer. If another writer has taken over (epoch mismatch), abort with `WriterFenced`.

3. **Allocate IDs.** For operations that need new IDs (schemas, tables, columns, files, snapshots), allocate from the counter system. The new snapshot ID is determined here.

4. **Build the write batch.** For each buffered operation:
   - Construct the key (tag + fields, big-endian encoded)
   - Serialize the row as protobuf
   - Wrap in the value envelope (version + magic + payload)
   - Add to SlateDB's `WriteBatch`
   - Also add counter updates and any secondary indexes

5. **Commit the batch.** Submit the `WriteBatch` to SlateDB, which writes it atomically to the WAL. This single WAL write is the commit point: if it completes, the transaction is committed. If it fails, no bytes were written.

6. **Release the mutex.** The transaction is complete.

## Atomicity Guarantee

The key insight is that SlateDB's `WriteBatch` is atomic: all key-value pairs in the batch are written together in a single WAL segment. There is no possibility of a partial commit where some rows are visible but others are not. Either the entire batch (all catalog rows for the transaction, plus counter updates, plus secondary indexes) makes it to the WAL, or none of it does.

This is why SlateDuck buffers all operations and applies them in one batch at commit time, rather than writing each INSERT individually. Individual writes would not provide transaction atomicity.

## Size Limits

The buffered transaction has a maximum size limit of 64 MiB (`MAX_BATCH_SIZE`). This prevents pathological cases (e.g., registering millions of files in a single transaction) from consuming excessive memory or creating oversized WAL segments. If the estimated size exceeds the limit during buffering, the session returns an error (SQLSTATE `54001`).

In practice, typical DuckDB transactions are well under 1 MiB (even bulk file registration transactions with hundreds of files are only a few hundred KB of catalog data).

## Auto-Commit Mode

When DuckDB sends write statements without an explicit `BEGIN`/`COMMIT` wrapper, SlateDuck operates in auto-commit mode: each individual statement is treated as its own transaction (buffered, committed immediately). This is equivalent to wrapping every statement in `BEGIN; statement; COMMIT;`.

## Read Transactions

SlateDuck does not need explicit read transactions because reads are bound to a specific snapshot ID. A reader at snapshot N always sees consistent state at snapshot N, regardless of concurrent writes creating snapshot N+1, N+2, etc. There is no "read transaction" that could be affected by a concurrent write.

## Rollback

On `ROLLBACK` (or connection close while in a transaction), SlateDuck simply discards the buffered operations. Nothing was written to SlateDB during the transaction (writes only happen at commit), so rollback is instantaneous and has no side effects.

## Crash During Commit

If the SlateDuck process crashes during the commit sequence:

- **Before the WAL write:** The buffered operations are lost (in memory only). The catalog is unchanged. DuckDB will receive a connection error and can retry.
- **During the WAL write:** Atomic PUT to object storage means either the bytes are fully written (committed) or not present at all (aborted). No partial state.
- **After the WAL write:** The transaction is committed. On restart, SlateDuck will see the committed state.

There is no "recovery" phase on startup — the catalog state is always consistent as stored in SlateDB.

## Relationship to DuckLake Snapshots

Every committed write transaction creates exactly one new DuckLake snapshot. The snapshot ID is allocated during commit and becomes the `begin_snapshot` for all new rows in the transaction. This means DuckLake snapshots and SlateDuck write transactions are one-to-one: each snapshot represents the result of exactly one atomic transaction.
