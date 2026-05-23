# TLS Configuration

SlateDuck supports TLS encryption for client connections, protecting catalog data in transit between DuckDB and the SlateDuck server. This is essential for deployments where the network between DuckDB and SlateDuck is untrusted (cross-region, cross-VPC, or over the public internet).

## Enabling TLS

Provide a certificate and private key:

```bash
slateduck --storage s3://bucket/catalog/ \
  --bind 0.0.0.0:5432 \
  --tls-cert /etc/slateduck/tls.crt \
  --tls-key /etc/slateduck/tls.key
```

Or via environment variables:

```bash
SLATEDUCK_TLS_CERT=/etc/slateduck/tls.crt \
SLATEDUCK_TLS_KEY=/etc/slateduck/tls.key \
slateduck --storage s3://bucket/catalog/
```

When TLS is enabled, SlateDuck negotiates TLS during the PostgreSQL startup sequence (it advertises `SupportSSL` in response to the client's `SSLRequest` message). Clients that do not request TLS can still connect in plaintext unless you also set `SLATEDUCK_REQUIRE_TLS=true`.

## Certificate Sources

### Self-Signed (Development)

Generate a self-signed certificate for development:

```bash
openssl req -x509 -newkey rsa:4096 -keyout tls.key -out tls.crt \
  -days 365 -nodes -subj '/CN=slateduck'
```

DuckDB must be configured to trust this certificate (or disable certificate verification, which is only acceptable for development).

### Let's Encrypt (Production)

Use certbot or similar ACME client to obtain a certificate from Let's Encrypt. This requires a DNS name that resolves to the SlateDuck instance.

### Cloud Certificate Manager

For cloud deployments, use your provider's certificate manager:
- **AWS:** ACM certificates (terminate TLS at ALB, plain TCP to SlateDuck)
- **GCP:** Google-managed certificates (via Cloud Load Balancer)
- **Azure:** Azure Key Vault certificates

## TLS at the Load Balancer

For Kubernetes and cloud deployments, it is common to terminate TLS at a load balancer or ingress controller rather than in SlateDuck itself. This simplifies certificate management and offloads the cryptographic work:

```
DuckDB → (TLS) → Load Balancer → (plaintext) → SlateDuck
```

This is acceptable when the network between the load balancer and SlateDuck is trusted (same VPC, same node, service mesh with mTLS).

## DuckDB Connection with TLS

When connecting from DuckDB to a TLS-enabled SlateDuck instance:

```sql
ATTACH 'ducklake:host=slateduck.example.com;port=5432;sslmode=require' AS my_lake;
```

The `sslmode=require` parameter tells DuckDB's PostgreSQL client to negotiate TLS. Other modes (`verify-ca`, `verify-full`) provide additional certificate validation.
