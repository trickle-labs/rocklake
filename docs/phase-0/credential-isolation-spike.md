# Object-Store and Credential Isolation Spike

## Objective

Validate that the sidecar (catalog plane) and DuckDB (data plane) can operate
under separate IAM policies with no cross-access.

## Design

```
┌─────────────────────────────────────────────┐
│              S3 Bucket: my-lake              │
├─────────────────────────────────────────────┤
│  catalogs/warehouse-a/   ← catalog-only IAM │
│  data/warehouse-a/       ← data-only IAM    │
└─────────────────────────────────────────────┘
```

### IAM Policies

**catalog-only** (sidecar):
```json
{
  "Effect": "Allow",
  "Action": ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"],
  "Resource": [
    "arn:aws:s3:::my-lake/catalogs/*"
  ],
  "Condition": {
    "StringLike": {"s3:prefix": ["catalogs/*"]}
  }
}
```

**data-only** (DuckDB):
```json
{
  "Effect": "Allow",
  "Action": ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"],
  "Resource": [
    "arn:aws:s3:::my-lake/data/*"
  ],
  "Condition": {
    "StringLike": {"s3:prefix": ["data/*"]}
  }
}
```

## Spike Validation (MinIO)

### Setup

1. Run MinIO locally with two IAM users:
   - `catalog-user`: access to `catalogs/` prefix only
   - `data-user`: access to `data/` prefix only

2. Configure SlateDuck sidecar with `catalog-user` credentials
3. Configure DuckDB with `data-user` credentials

### Results

| Scenario | Expected | Result |
|----------|----------|--------|
| Sidecar reads `catalogs/` | ✅ Allowed | ✅ PASS |
| Sidecar writes `catalogs/` | ✅ Allowed | ✅ PASS |
| Sidecar reads `data/` | ❌ Denied | ✅ PASS (403 Forbidden) |
| DuckDB reads `data/` | ✅ Allowed | ✅ PASS |
| DuckDB writes `data/` | ✅ Allowed | ✅ PASS |
| DuckDB reads `catalogs/` | ❌ Denied | ✅ PASS (403 Forbidden) |

### SQLSTATE Mapping for Permission Failures

| Error Source | HTTP Status | SQLSTATE | Message |
|-------------|-------------|----------|---------|
| S3 AccessDenied on catalog prefix | 403 | `42501` | "insufficient_privilege: catalog access denied" |
| S3 AccessDenied on data prefix | 403 | `42501` | "insufficient_privilege: data access denied" |
| S3 NoSuchBucket | 404 | `3D000` | "invalid_catalog_name: bucket not found" |
| S3 throttle (429) | 429 | `08006` | "connection_failure: object store throttled" |

### GC / Maintenance Job

The GC job requires **both** catalog and data access:
- Reads `ducklake_files_scheduled_for_deletion` from catalog
- Deletes orphaned Parquet files from data prefix

A separate `gc-user` IAM role with both policies is required:
```json
{
  "Effect": "Allow",
  "Action": ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"],
  "Resource": [
    "arn:aws:s3:::my-lake/catalogs/*",
    "arn:aws:s3:::my-lake/data/*"
  ]
}
```

## Conclusion

Credential isolation works as designed. The sidecar operates under a
least-privilege policy with no access to data files. DuckDB operates
with no access to catalog internals. This provides defense-in-depth
against catalog corruption from client-side bugs.
