# Custom Clients

Because SlateDuck speaks the PostgreSQL wire protocol, any PostgreSQL-compatible client library can connect to it. This enables building custom tooling, monitoring dashboards, migration scripts, and administrative interfaces in any language with a PostgreSQL driver.

## Connection Details

SlateDuck accepts standard PostgreSQL protocol connections:

- **Protocol version:** 3.0 (the current PostgreSQL protocol)
- **Authentication:** `AuthenticationOk` (no auth) or `AuthenticationCleartextPassword` (if password is configured)
- **TLS:** SSLRequest negotiation supported

## Language Examples

### Python (psycopg2)

```python
import psycopg2

conn = psycopg2.connect(host="localhost", port=5432, dbname="slateduck")
cur = conn.cursor()
cur.execute("SELECT schema_name FROM ducklake_schemas()")
schemas = cur.fetchall()
```

### Go (pgx)

```go
conn, err := pgx.Connect(context.Background(), "postgres://localhost:5432/slateduck")
rows, err := conn.Query(context.Background(), "SELECT table_name FROM ducklake_tables(1)")
```

### Node.js (pg)

```javascript
const { Client } = require('pg');
const client = new Client({ host: 'localhost', port: 5432, database: 'slateduck' });
await client.connect();
const res = await client.query('SELECT * FROM ducklake_columns(1, 1)');
```

## Important Limitations

Custom clients can send only the specific SQL statements that SlateDuck's bounded SQL dispatcher recognizes. Arbitrary SQL will be rejected with an error. The supported statements are those emitted by DuckDB's `ducklake` extension — they are catalog metadata queries, not general-purpose SQL.

If you want to query the catalog with arbitrary SQL, export to NDJSON and query the export with DuckDB or any SQL engine.

## Use Cases for Custom Clients

**Monitoring:** Query snapshot counts and writer epoch from a monitoring script to detect anomalies.

**Migration tooling:** Script bulk catalog operations (e.g., renaming schemas across environments) using your preferred programming language.

**CI/CD integration:** Verify catalog state as part of a deployment pipeline (e.g., confirm a table exists before deploying an ETL job).

**Admin dashboards:** Build a web dashboard showing catalog contents, recent operations, and health status.
