# Getting Started

Welcome to SlateDuck. This section takes you from zero to a working lakehouse catalog in minutes, whether you are experimenting on your laptop or deploying to production cloud storage. Each guide builds on the previous one, but you can jump directly to whichever matches your situation.

## Choose Your Path

If you have never used SlateDuck before, start with **What is SlateDuck?** for a high-level orientation. It explains what problem SlateDuck solves, how it fits into the DuckLake ecosystem, and what makes it different from PostgreSQL-backed catalogs.

Once you understand the concept, **Quickstart (Local)** walks you through a complete workflow on your local machine using the filesystem as the object store. You will create a catalog, register tables, insert data, and query it through DuckDB in under five minutes. No cloud credentials required.

**Quickstart (Cloud)** extends the local workflow to real object storage. You will provision an S3 bucket (or GCS, or Azure Blob), configure credentials, and see the same workflow running against durable cloud storage. This is what production looks like.

Finally, **Your First Lakehouse** ties everything together into a realistic scenario: multiple tables, schema evolution, time travel queries, and garbage collection. By the end you will have a solid mental model of how SlateDuck operates day-to-day.

## Prerequisites

SlateDuck requires:

- **Rust 1.75+** (if building from source) or a pre-built binary from the releases page
- **DuckDB 1.2+** with the `ducklake` extension installed
- For cloud deployments: appropriate credentials (AWS IAM, GCP service account, or Azure SPN)

## What You Will Learn

By the end of this section you will be able to:

1. Explain what SlateDuck does and why it exists
2. Run a local catalog and execute DuckLake queries against it
3. Deploy to S3/GCS/Azure with proper credential configuration
4. Perform schema evolution, time travel, and basic operational tasks
