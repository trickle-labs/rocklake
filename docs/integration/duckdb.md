# DuckDB Integration

DuckDB is the primary client for SlateDuck. The integration uses DuckDB's `ducklake` extension, which connects to SlateDuck over the PostgreSQL wire protocol to manage lakehouse catalog metadata while executing analytical queries locally against Parquet data files in object storage.

## Prerequisites

- DuckDB v1.2.0 or later (with the `ducklake` extension)
- A running SlateDuck instance (see [Deployment](../deployment/index.md))

## Connecting

```sql
-- Install the ducklake extension (first time only)
INSTALL ducklake;
LOAD ducklake;

-- Attach a SlateDuck catalog
ATTACH 'ducklake:host=localhost;port=5432' AS my_lake;

-- Start using the lakehouse
USE my_lake;
CREATE SCHEMA analytics;
CREATE TABLE analytics.events (
    event_id BIGINT,
    event_type VARCHAR,
    timestamp TIMESTAMP,
    payload JSON
);
```

## Connection String Parameters

The connection string follows the `ducklake:` scheme with PostgreSQL-style parameters:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `host` | `localhost` | SlateDuck server hostname or IP |
| `port` | `5432` | SlateDuck server port |
| `password` | (none) | Authentication password (if configured) |
| `sslmode` | `prefer` | TLS mode: `disable`, `prefer`, `require`, `verify-ca`, `verify-full` |
| `connect_timeout` | `10` | Connection timeout in seconds |

## What Happens Under the Hood

When you execute DDL or DML through DuckDB against a DuckLake-attached catalog, the following occurs:

1. DuckDB's query planner recognizes the operation targets a DuckLake catalog
2. The `ducklake` extension translates the operation into catalog SQL and sends it to SlateDuck over PG-wire
3. SlateDuck classifies the SQL, executes the catalog operation, and returns results
4. For data operations (INSERT, SELECT), DuckDB reads/writes Parquet files directly in object storage — SlateDuck only provides the file locations

This means SlateDuck never sees your actual data. It only manages metadata: which tables exist, what columns they have, and where the data files are stored.

## Supported Operations

All DuckLake catalog operations are supported:

- Schema management (CREATE/DROP/ALTER SCHEMA)
- Table management (CREATE/DROP/ALTER TABLE)
- View and macro management
- Data file registration and deregistration
- Table statistics and column statistics
- Transaction management (BEGIN/COMMIT)
- Time travel (reading historical catalog state)

## Performance Characteristics

Each catalog operation requires at least one network round-trip to SlateDuck. A typical DuckDB query that touches one table involves:

1. List schemas (1 round-trip)
2. Get table metadata (1 round-trip)
3. List columns for the table (1 round-trip)
4. List data files for the table (1 round-trip)
5. Get column statistics for partition pruning (1 round-trip)

Total: ~5 round-trips × 1-5ms local network latency = 5-25ms catalog overhead. This is negligible for analytical queries that scan gigabytes of Parquet data (seconds to minutes of execution time).

## Multiple DuckDB Instances

Multiple DuckDB instances can connect to the same SlateDuck catalog simultaneously. All read catalog state at the same snapshot and see a consistent view. Only one instance at a time should perform write operations (coordinated by DuckLake's transaction semantics).
