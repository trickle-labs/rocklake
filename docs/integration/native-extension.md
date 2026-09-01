# Native DuckDB Extension

The native DuckDB extension is not a supported v0.51.1 deployment path.

The repository contains an experimental C++ ABI wrapper and the
`rocklake-ffi` library, but it is not a complete DuckDB catalog extension and
is not published through the DuckDB extension repository. Do not copy the
wrapper into DuckDB's extension directory or use it for production.

For DuckDB, use the supported PostgreSQL wire sidecar:

```sql
INSTALL ducklake;
LOAD ducklake;
ATTACH 'ducklake:postgres:host=127.0.0.1 port=5432' AS lake;
```

For embedded applications, use the C ABI or one of the language bindings as
documented in [Client Library](client-library.md). The C ABI is a direct
catalog API; it does not make the experimental wrapper a DuckDB extension.
