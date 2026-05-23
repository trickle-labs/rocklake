# Internals

This section documents SlateDuck's internal implementation details. These pages are for contributors, advanced users who need to understand exactly what happens under the hood, and anyone debugging unusual behavior. You do not need to read this section to use SlateDuck effectively — it is for those who want to understand how the sausage is made.

## Pages

- **[MVCC Filter](mvcc-filter.md)** — How multi-version concurrency control works at the key-value level
- **[Tag Allocation](tag-allocation.md)** — How tag bytes are assigned and organized
- **[Type-Aware Statistics](type-aware-stats.md)** — How column statistics are encoded by DuckDB type
- **[SQLSTATE Mapping](sqlstate-mapping.md)** — How internal errors map to PostgreSQL error codes
- **[Wire Corpus](wire-corpus.md)** — The test corpus of actual DuckDB wire protocol sessions
- **[Schema Version](schema-version.md)** — How catalog format versions are managed
- **[Inlined Data](inlined-data.md)** — How small data files are inlined directly into the catalog
- **[Crash Safety](crash-safety.md)** — How crash safety is achieved without explicit recovery
