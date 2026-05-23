# Your First Lakehouse

This guide walks through a realistic lakehouse scenario that exercises the core features of SlateDuck: multiple tables with relationships, schema evolution over time, time travel queries, garbage collection, and basic operational tasks. By the end, you will have a solid mental model of how SlateDuck behaves in production.

## Scenario

You are building an analytics platform for an e-commerce company. The lakehouse tracks customers, orders, and order items. Over time, the schema evolves as business requirements change, and you need to query historical states for audit and debugging purposes.

## Setting Up

Start SlateDuck and connect DuckDB as described in the quickstart guides. We will use local storage for simplicity, but everything works identically on cloud storage.

```bash
slateduck --storage file:///tmp/ecommerce-lakehouse --bind 127.0.0.1:5432
```

```sql
LOAD ducklake;
ATTACH '' AS ecom (TYPE ducklake, PG 'host=127.0.0.1 port=5432');
USE ecom;
```

## Creating the Initial Schema

```sql
CREATE SCHEMA store;

CREATE TABLE store.customers (
    customer_id BIGINT,
    email VARCHAR,
    name VARCHAR,
    created_at TIMESTAMP
);

CREATE TABLE store.orders (
    order_id BIGINT,
    customer_id BIGINT,
    total_amount DECIMAL(12, 2),
    order_date DATE,
    status VARCHAR
);

CREATE TABLE store.order_items (
    item_id BIGINT,
    order_id BIGINT,
    product_name VARCHAR,
    quantity INTEGER,
    unit_price DECIMAL(10, 2)
);
```

At this point, SlateDuck has created snapshot 1 (initial empty catalog), snapshot 2 (schema created), and snapshots 3-5 (one per table creation). Each snapshot is an immutable record of the catalog state at that moment.

## Loading Data

```sql
INSERT INTO store.customers VALUES
    (1, 'alice@example.com', 'Alice Johnson', '2024-01-01 00:00:00'),
    (2, 'bob@example.com', 'Bob Smith', '2024-01-15 00:00:00'),
    (3, 'carol@example.com', 'Carol Williams', '2024-02-01 00:00:00');

INSERT INTO store.orders VALUES
    (101, 1, 299.99, '2024-03-01', 'completed'),
    (102, 1, 149.50, '2024-03-15', 'completed'),
    (103, 2, 89.99, '2024-03-20', 'shipped'),
    (104, 3, 450.00, '2024-04-01', 'pending');

INSERT INTO store.order_items VALUES
    (1001, 101, 'Wireless Headphones', 1, 199.99),
    (1002, 101, 'USB-C Cable', 2, 49.99),
    (1003, 102, 'Keyboard', 1, 149.50),
    (1004, 103, 'Mouse Pad', 3, 29.99),
    (1005, 104, 'Monitor Stand', 1, 450.00);
```

Each INSERT causes DuckDB to write a Parquet file and register it with SlateDuck. The catalog now contains data file entries with paths, row counts, file sizes, and column statistics for predicate pushdown.

## Schema Evolution

A month later, the business needs a `phone` column on customers and wants to rename `total_amount` to `order_total` on orders. DuckLake handles schema evolution through versioned columns:

```sql
ALTER TABLE store.customers ADD COLUMN phone VARCHAR;
ALTER TABLE store.orders RENAME COLUMN total_amount TO order_total;
```

In SlateDuck's catalog model, adding a column creates a new `ColumnRow` with a `begin_snapshot` set to the current snapshot. Renaming creates a new version of the column row (old one gets `end_snapshot`, new one with updated name gets `begin_snapshot`). Historical queries still see the old schema because they read at older snapshot IDs where the new column does not exist and the old name is still active.

## Time Travel

Now you can query the catalog at different points in history:

```sql
-- Current state: customers has 4 columns (including phone)
SELECT column_name FROM information_schema.columns
WHERE table_name = 'customers' ORDER BY ordinal_position;

-- State before the ALTER: customers has 3 columns
-- (Use the snapshot ID from before the ALTER)
SELECT * FROM ducklake_snapshots();
```

Time travel in SlateDuck is not a special feature bolted on after the fact. It is the natural consequence of the immutability model: every row has visibility bounds, and reading at a specific snapshot simply means applying those bounds as a filter. There is no extra storage cost because old versions are never copied or moved.

## Garbage Collection

Over time, you may accumulate snapshots that are no longer needed for auditing. SlateDuck's garbage collection is a two-phase process that gives you full control:

**Phase 1: Advance the retention horizon.** This marks old snapshots as "logically deleted" but does not remove any data:

```bash
slateduck gc --storage file:///tmp/ecommerce-lakehouse --retain-days 30
```

After this command, snapshots older than 30 days are no longer visible to time travel queries. However, the actual key-value pairs are still present in SlateDB.

**Phase 2 (optional): Excise.** This physically removes the superseded rows from the catalog:

```bash
slateduck excise --storage file:///tmp/ecommerce-lakehouse --before-snapshot 5
```

Excision is a destructive operation that permanently removes historical data. It is optional and only needed if you have compliance requirements (data deletion mandates) or want to reduce storage costs for very large catalogs.

## Inspecting Catalog State

At any point, you can inspect the internal state of the catalog:

```bash
slateduck inspect --storage file:///tmp/ecommerce-lakehouse
```

This shows you the current snapshot ID, the number of schemas, tables, columns, data files, and delete files, the writer epoch, the retention horizon, and the format version. It is an invaluable tool for understanding what is happening inside your catalog.

## What You Have Learned

1. **Schema creation** allocates unique IDs and creates versioned rows in the catalog
2. **Data loading** writes Parquet files and registers them with path, size, and statistics
3. **Schema evolution** creates new row versions with visibility bounds, preserving history
4. **Time travel** reads at a specific snapshot by filtering on begin/end bounds
5. **Garbage collection** is a two-phase process: advance retention, then optionally excise
6. **Inspection** gives you full visibility into the catalog's internal state

## Next Steps

- [Concepts](../concepts/index.md) — Deep understanding of immutability, MVCC, and the single-writer model
- [Operations](../operations/index.md) — Production operational procedures
- [Architecture](../architecture/index.md) — How the crates fit together
