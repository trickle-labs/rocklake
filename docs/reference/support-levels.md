# Support levels

The v0.63.1 public-surface manifest is the authoritative list of supported
interfaces. A compatibility promise requires tests, documentation, a named
owner, and a migration policy.

| Interface | Level | Boundary |
| --- | --- | --- |
| `rocklake` binary and PostgreSQL wire surface | Supported | DuckLake 1.0 and the versions named in the compatibility matrix |
| Rust client, read-only API, DataFusion | Preview | Usable, but not part of the stable v1.x promise |
| Python, Node.js, and Java bindings | Experimental | Smoke-tested only; compatibility may change |
| Native extension | Unsupported | No release support or compatibility promise |

Stable commands, configuration fields, metrics, JSON schemas, and artifact
names remain compatible through v1.x. A deprecation must be documented, keep
working for at least one minor release, and include a migration path. Format
changes require an explicit migration and release note.
