# Networking

This page covers network topology considerations, firewall rules, and service discovery for SlateDuck deployments.

## Network Requirements

SlateDuck needs two network paths:

1. **Inbound:** TCP connections from DuckDB clients to SlateDuck's PG-wire port (default 5432)
2. **Outbound:** HTTPS connections from SlateDuck to object storage (S3, GCS, Azure Blob)

No other network access is required. SlateDuck does not communicate with external services, does not phone home, and does not require internet access beyond reaching your object storage endpoint.

## Topology Options

### Same-Host (Development)

DuckDB and SlateDuck run on the same machine. Connection is via localhost. No network security needed.

```
DuckDB → localhost:5432 → SlateDuck → (internet) → S3
```

### Same-VPC (Production)

DuckDB and SlateDuck run in the same VPC/VNET. Connection is via private IP. Firewall allows port 5432 from DuckDB security group to SlateDuck security group.

```
DuckDB (10.0.1.x) → slateduck.internal:5432 → SlateDuck (10.0.2.x) → (VPC endpoint) → S3
```

### Cross-VPC / Cross-Region

DuckDB and SlateDuck are in different VPCs or regions. Use VPC peering, Transit Gateway, or PrivateLink. Enable TLS.

### Public Internet

SlateDuck accessible over the public internet. Requires TLS, authentication (password), and ideally IP allowlisting or a VPN.

## Firewall Rules

### Inbound (to SlateDuck)

| Source | Port | Protocol | Purpose |
|--------|------|----------|---------|
| DuckDB clients | 5432/tcp | PostgreSQL wire | Catalog queries |
| Monitoring | (metrics port)/tcp | HTTP | Prometheus scraping |

### Outbound (from SlateDuck)

| Destination | Port | Protocol | Purpose |
|-------------|------|----------|---------|
| S3/GCS/Azure | 443/tcp | HTTPS | Object storage access |

## VPC Endpoints (AWS)

For S3 access without traversing the public internet, use a VPC endpoint:

```bash
aws ec2 create-vpc-endpoint \
  --vpc-id vpc-xxxxx \
  --service-name com.amazonaws.us-east-1.s3 \
  --route-table-ids rtb-xxxxx
```

This reduces latency (no NAT gateway hop), eliminates data transfer costs, and improves security (traffic stays on the AWS backbone).

## Service Discovery

In Kubernetes, SlateDuck is discoverable via DNS: `slateduck.<namespace>.svc.cluster.local`. For non-Kubernetes deployments, use your preferred service discovery mechanism (Consul, Cloud Map, DNS).

## Connection Pooling

SlateDuck supports up to `--max-sessions` concurrent connections (default 64). If you have more DuckDB instances than the session limit, place a TCP connection pooler (like PgBouncer in TCP mode) in front of SlateDuck. Note that PgBouncer's transaction-mode pooling does not work because DuckDB's ducklake extension maintains session state.
