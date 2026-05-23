# pg-tide-relay

pg-tide-relay is a concept for relaying DuckLake catalog traffic through existing PostgreSQL infrastructure. Rather than connecting DuckDB directly to SlateDuck, the traffic passes through a PostgreSQL-compatible proxy or middleware layer that provides additional features like connection pooling, audit logging, or routing.

## Concept

```
DuckDB → pg-tide-relay → SlateDuck
```

The relay sits between DuckDB and SlateDuck, intercepting PostgreSQL wire protocol messages. It can:

- Log all catalog operations for audit purposes
- Route traffic to different SlateDuck instances based on catalog name
- Add authentication/authorization beyond what SlateDuck provides
- Pool connections to reduce session overhead on SlateDuck

## When This Is Useful

**Multi-tenant routing:** If you run multiple SlateDuck instances (one per tenant), the relay can route connections to the correct instance based on the database name in the startup message.

**Enhanced security:** If you need fine-grained access control that SlateDuck does not provide, the relay can inspect queries and reject unauthorized operations.

**Audit compliance:** If you need a complete audit trail of all catalog operations with tamper-evident logging, the relay can capture all traffic before forwarding to SlateDuck.

## Implementation Status

pg-tide-relay is currently a design concept. The protocol compatibility between DuckDB, SlateDuck, and standard PostgreSQL proxies has been validated — any TCP proxy that passes PostgreSQL wire protocol transparently (like HAProxy, pgbouncer in TCP mode, or a custom Rust proxy) works as a relay.

## Building Your Own Relay

Because SlateDuck uses standard PostgreSQL wire protocol, building a relay is straightforward with any language that has a PostgreSQL protocol library:

1. Accept incoming TCP connections
2. Parse the startup message to extract routing information (database name, user, etc.)
3. Open a connection to the appropriate SlateDuck instance
4. Forward messages bidirectionally, optionally logging or modifying them
5. Clean up when either side disconnects

The `pgwire` Rust crate (which SlateDuck itself uses) is an excellent foundation for building such a relay in Rust.
