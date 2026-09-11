# Security Guide

This page describes the security configuration options for RockLake's PG-Wire
server, the risks of each configuration, and the recommended mitigations.

## Authentication

RockLake supports SCRAM-SHA-256 authentication for PG-Wire connections.
Production binary deployments can load a SCRAM verifier, without storing the
plaintext password, from a permission-restricted JSON file:

```json
{"username":"admin","scram_verifier":"v=1,i=4096,s=...,sk=...,sv=..."}
```

Pass it with `--auth-verifier-file` or
`ROCKLAKE_AUTH_VERIFIER_FILE`. The legacy password options remain available for
library compatibility and local development; do not use plaintext passwords in
production configuration.

When no `--auth-user` is set, the server accepts all connections without
authentication. This is appropriate for local development and single-host
deployments where network access is already restricted.

## TLS Encryption

TLS is configured via `--tls-cert` and `--tls-key` flags pointing to a PEM
certificate and private-key file respectively. Setting `--tls-required`
refuses all non-TLS connections (including plain-text clients).

## Auth Without TLS — Security Risk

> **Warning:** TLS is still required for transport and server-identity
> protection. Only the explicit cleartext compatibility path transmits
> credentials in plaintext; the release binary uses SCRAM-SHA-256.

When RockLake starts with `--auth-user` set but without `--tls-cert` /
`--tls-key`, it emits a startup warning. SCRAM and cleartext compatibility
configurations use different warning text; the cleartext path is:

```
WARN rocklake_pgwire::server: Cleartext password authentication is enabled
without TLS. Credentials will be sent in plaintext. Use --tls-cert /
--tls-key to enable TLS.
```

SCRAM prevents a passive observer from reading the password, but TLS is still
needed to protect the connection and authenticate the server. A passive
observer can read the username and traffic metadata. The explicit cleartext
compatibility path exposes the password in the PG-Wire `PasswordMessage`.

### Mitigations

| Scenario | Recommended action |
|----------|--------------------|
| Internet-facing or multi-tenant | **Always** enable TLS with `--tls-cert` and `--tls-key`. |
| Private LAN / same host | Acceptable without TLS; consider firewall rules. |
| Development / local loop | No TLS needed; omit `--auth-user` or keep the listener on loopback. |

### Enabling TLS

```bash
rocklake serve \
  --tls-cert /path/to/cert.pem \
  --tls-key  /path/to/key.pem  \
  --tls-required               \
  --auth-user admin             \
  --auth-verifier-file /run/secrets/rocklake-auth-verifier.json
```

Self-signed certificates work for development. For production, use a
certificate signed by a trusted CA (Let's Encrypt, your organisation's PKI,
etc.).

## Clock Skew and Lease Expiry

Snapshot leases use wall-clock time (`SystemTime::now()`) for expiry checks.
In distributed deployments where multiple clients hold leases against the same
catalog:

- Clock skew between nodes can cause a lease holder to see its lease as expired
  before the catalog server's clock agrees.
- The recommended maximum clock skew is **≤ 5 seconds** for the default 1-hour
  lease TTL.
- Use NTP or a similar time-synchronisation service on all nodes.

Lease logic is tested against a `MockClock` (from `rocklake_core::clock`)
that eliminates real-time dependencies in unit tests.
