# Quickstart (Cloud)

This guide extends the local quickstart to real cloud object storage. You will configure credentials, start SlateDuck against an S3 bucket, and see the same DuckLake workflow running against durable cloud storage. The steps are nearly identical for GCS and Azure; differences are noted where they apply.

## Prerequisites

- SlateDuck binary installed
- DuckDB 1.2+ with the `ducklake` extension
- An S3 bucket (or GCS bucket, or Azure Blob container) with write access
- Appropriate credentials configured in your environment

## Configuring Credentials

SlateDuck uses the standard credential discovery mechanisms for each cloud provider. You do not pass credentials directly to SlateDuck; instead, you configure them in your environment the same way you would for the AWS CLI, `gsutil`, or `az` commands.

### AWS S3

Set the standard AWS environment variables or ensure your `~/.aws/credentials` file is configured:

```bash
export AWS_ACCESS_KEY_ID=AKIA...
export AWS_SECRET_ACCESS_KEY=...
export AWS_REGION=us-east-1
```

Alternatively, if you are running on EC2, ECS, or Lambda, the instance role or task role is used automatically. SlateDuck supports IAM Roles for Service Accounts (IRSA) on EKS as well.

### Google Cloud Storage

```bash
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json
```

Or use application default credentials if running on GCE/GKE:

```bash
gcloud auth application-default login
```

### Azure Blob Storage

```bash
export AZURE_STORAGE_ACCOUNT=myaccount
export AZURE_STORAGE_KEY=...
```

Or use Azure AD authentication with a service principal or managed identity.

## Step 1: Start SlateDuck

Point SlateDuck at your cloud storage location:

```bash
# AWS S3
slateduck --storage s3://my-lakehouse-bucket/catalog/ --bind 0.0.0.0:5432

# Google Cloud Storage
slateduck --storage gs://my-lakehouse-bucket/catalog/ --bind 0.0.0.0:5432

# Azure Blob Storage
slateduck --storage az://my-container/catalog/ --bind 0.0.0.0:5432
```

On first start, SlateDuck initializes the catalog in the specified path. This creates the SlateDB manifest, WAL, and initial system keys. Subsequent starts detect the existing catalog and resume from the latest state.

The output looks similar to the local version:

```
SlateDuck v0.8.0
Catalog: s3://my-lakehouse-bucket/catalog/
Listening: 0.0.0.0:5432
Writer epoch: 1
```

## Step 2: Connect and Use

The DuckDB workflow is identical to the local quickstart. The only difference is in the ATTACH statement if you have TLS or authentication configured on SlateDuck:

```sql
LOAD ducklake;
ATTACH '' AS lakehouse (TYPE ducklake, PG 'host=my-server port=5432');
USE lakehouse;

CREATE SCHEMA production;
CREATE TABLE production.orders (
    order_id BIGINT,
    customer_id BIGINT,
    total_amount DECIMAL(10, 2),
    order_date DATE,
    status VARCHAR
);
```

## Step 3: Verify Cloud Persistence

You can verify that the catalog state is persisted in cloud storage using your provider's CLI:

```bash
# AWS
aws s3 ls s3://my-lakehouse-bucket/catalog/ --recursive

# GCS
gsutil ls -r gs://my-lakehouse-bucket/catalog/

# Azure
az storage blob list --container-name my-container --prefix catalog/
```

You will see the SlateDB manifest, WAL segments, and SST files. These are the only artifacts SlateDuck creates. Data files (Parquet) written by DuckDB appear alongside them in the same bucket.

## Performance Considerations

Cloud object storage has higher latency than local filesystem (typically 20-50ms per request for S3, similar for GCS and Azure). SlateDuck mitigates this through several mechanisms:

- **Batched writes:** Multiple catalog changes in a single DuckDB transaction are combined into one SlateDB batch, which becomes a single PUT to the WAL
- **Hot key optimization:** Frequently-accessed metadata (current snapshot ID, table file counts) is cached in a single "hot key" that requires only one GET on cold start
- **Prefix scans:** Table listings use prefix-bounded key ranges, limiting the number of keys scanned

For most interactive workloads, you will observe 50-200ms per catalog operation against S3. Bulk operations (registering hundreds of files in a single transaction) are amortized by batching.

## S3 Express One Zone

For the lowest latency on AWS, SlateDuck supports S3 Express One Zone (directory buckets). These provide single-digit millisecond PUT/GET latency at higher cost. Use the `s3express://` URI scheme:

```bash
slateduck --storage s3express://my-express-bucket--use1-az1--x-s3/catalog/ --bind 0.0.0.0:5432
```

This reduces catalog operation latency from 50-200ms to 5-20ms, making interactive workflows feel nearly local.

## Security

In production, always enable TLS and authentication:

```bash
slateduck \
    --storage s3://my-lakehouse-bucket/catalog/ \
    --bind 0.0.0.0:5432 \
    --tls-cert /etc/ssl/slateduck.crt \
    --tls-key /etc/ssl/slateduck.key \
    --auth-user ducklake \
    --auth-password "${SLATEDUCK_PASSWORD}"
```

DuckDB connects with credentials in the connection string:

```sql
ATTACH '' AS lakehouse (TYPE ducklake, PG 'host=my-server port=5432 user=ducklake password=secret sslmode=require');
```

## Next Steps

- [Your First Lakehouse](first-lakehouse.md) — Schema evolution, time travel, and garbage collection
- [Deployment Guide](../deployment/index.md) — Production deployment patterns for Docker, Kubernetes, and serverless
- [Configuration](../deployment/configuration.md) — Full reference for all server options
