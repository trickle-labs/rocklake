# What SlateDuck Is Not

Defining what a system does NOT do is as important as defining what it does. Explicit non-goals prevent scope creep, set correct expectations, and help users choose the right tool. This page lists the things SlateDuck deliberately does not do, with explanations for each non-goal.

## Not a Query Engine

SlateDuck does not execute analytical queries. It does not scan Parquet files, perform joins, evaluate predicates, or compute aggregations. These are DuckDB's responsibilities. SlateDuck only stores and serves the metadata that tells DuckDB where to find data and what schema it has.

**Why not?** Because combining a metadata catalog with a query engine would create a monolithic system that is harder to scale, harder to debug, and harder to evolve. The separation of concerns between catalog (SlateDuck) and execution (DuckDB) means each can be optimized, deployed, and upgraded independently.

## Not a General-Purpose Database

You cannot use SlateDuck to store application data, run OLTP workloads, or serve as a backend for web applications. Its SQL support is intentionally limited to the ~50 statement patterns that DuckLake needs. It does not support CREATE USER, GRANT, prepared statement caching, connection pooling, or any of the features you would expect from a general-purpose PostgreSQL-compatible database.

**Why not?** Because general-purpose databases already exist and are excellent at their job. SlateDuck exists to solve a specific problem (serverless DuckLake catalog) that existing databases solve poorly. Adding general database features would dilute focus and introduce complexity that the target use case does not need.

## Not a Distributed System

SlateDuck does not implement consensus, leader election, replication, or cross-node coordination. There is one writer and any number of independent readers. If you need multi-writer access, you partition into multiple catalogs.

**Why not?** Because distributed coordination is the single greatest source of complexity, bugs, and operational burden in database systems. For a metadata catalog that processes a few writes per minute, distributed coordination is massive overkill. The single-writer model is correct, simple, and sufficient.

## Not a Data Lake Manager

SlateDuck does not manage the lifecycle of Parquet data files. It records their existence in the catalog but does not create them, compact them, optimize their layout, or delete them when they are no longer needed. Data file management is DuckDB's responsibility (or your ETL pipeline's responsibility).

**Why not?** Because data file lifecycle management is a complex topic with workload-specific trade-offs (compaction strategy, file size targets, partition pruning). These decisions belong with the query engine and data pipeline, not the metadata catalog.

## Not an Access Control System

SlateDuck provides optional password authentication for connections but does not implement fine-grained access control (table-level permissions, row-level security, column masking). If you need access control, implement it at the network level (VPC, security groups) or in a proxy layer in front of SlateDuck.

**Why not?** Because the DuckLake protocol does not include access control concepts. DuckDB's `ducklake` extension connects as a single user and has full access to the catalog. Implementing access control in SlateDuck without corresponding support in DuckDB would be security theater.

## Not a Streaming System

SlateDuck does not support change data capture (CDC), event streaming, or real-time notification of catalog changes. If you need to react to catalog mutations in real time, poll the snapshot counter or build an event system on top of the audit log.

**Why not?** Because streaming adds significant complexity (connection management, backpressure, delivery guarantees) for a feature that most DuckLake users do not need. The audit log provides an after-the-fact record of changes that can be polled efficiently.

## Not Multi-Tenant

SlateDuck does not support multiple isolated tenants within a single instance. Each instance serves one catalog (or a set of partitioned datasets registered in one registry). If you need multi-tenancy, run separate SlateDuck instances per tenant with separate storage paths.

**Why not?** Because multi-tenancy requires isolation guarantees (one tenant's operations cannot affect another), resource accounting, and often complex configuration. These concerns are better handled at the infrastructure level (containers, namespaces) than within the application.
