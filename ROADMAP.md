# RockLake roadmap

**Current baseline:** v0.47.17

RockLake already has a working DuckLake catalog, a PostgreSQL wire sidecar,
snapshot reads, writer fencing, recovery checks, and support for local and
cloud object storage.

The next phase is about making that product dependable and pleasant to use.
It is not about adding more engines, bindings, or storage layers.

This roadmap is a guide, not a release contract. Work can move when real usage
or measurements change the priority.

## Product direction

- Keep the DuckLake catalog sidecar as the main product.
- Make the local path work with one command and one documented connection example.
- Keep safe defaults for network access, authentication, and logging.
- Keep one writer and many readers as the operating model.
- Support fewer interfaces well instead of collecting compatibility claims.
- Preserve the existing correctness guarantees before changing storage behavior.
- Measure large-catalog behavior before adding caching or more concurrency.

## DuckLake compatibility target

Guarantee complete compatibility with DuckLake 1.0 for every declared DuckDB
target. Do not claim support for a target until its live compatibility tests
pass. This covers released targets, not future versions that have not been
tested.

- Declare DuckLake 1.0 / Catalog Version 7 as stable, with DuckDB 1.5.3 as the
  minimum supported client.
- Treat every released DuckDB version from 1.5.3 onward as its own target. The
  DuckDB version and its matching ducklake extension version are one pair.
- At the current baseline, test DuckDB 1.5.3, 1.5.4, and 1.5.5. Add each
  later stable release without dropping the older targets.
- Run the real DuckDB extension against RockLake for catalog creation, reads,
  writes, snapshots, schema changes, deletes, transactions, recovery, and
  supported object stores.
- Check every catalog table, type, and SQL operation that the extension uses.
- Turn each incompatibility into a regression test.
- Run the minimum and newest DuckDB targets on pull requests. Run every
  promised target nightly and before a release.
- Test new upstream releases when they matter. Keep the last known-good targets
  working, and reject unsupported catalog versions clearly.
- Block the DuckLake compatibility claim for any release if one promised target
  fails its live test suite.

## Now: make the current product trustworthy

The first goal is simple: a new user can run a local catalog, and an operator
can tell what is supported and how the process is exposed.

### Fix unsafe defaults

- Bind the main listener to 127.0.0.1 by default.
- Remove raw SQL from normal logs.
- Remove the secondary DataFusion listener, or give it the same bind,
  authentication, TLS, and access rules as the main listener.
- Use SCRAM-SHA-256 by default when password authentication is enabled.
- Keep public network access as an explicit choice.

### Use one CLI path

- Parse arguments with Clap.
- Dispatch directly from the typed commands.
- Delete the legacy parser and synthetic argument conversion.
- Keep an old flag or command name only when a real user still needs it.

### Make the product claims true

- Update the README to describe v0.47.17 and the commands that exist today.
- Remove documentation for unimplemented configuration files, mutual TLS, certificate
  reload, Docker images, and other unsupported features.
- State the tested DuckDB, DuckLake, Rust, and object-store versions.
- Keep the local quickstart short and run that exact flow in CI.
- Add a short security reporting guide.

### Keep releases boring

- Commit the version before creating the tag.
- Run formatting, linting, tests, and the local DuckDB smoke test before release.
- Build artifacts from the tagged commit.
- Publish checksums for the artifacts.

That is enough release machinery for now. Add more checks when a real failure
shows that one is missing.

## Next: improve daily operation

### Make startup clear

On successful startup, print the version, catalog, mode, listen address, TLS
state, authentication state, and readiness. Do not print secrets.

### Add a small preflight command

Add the doctor command when the basic local path is stable.

It should check:

- catalog URI.
- object-store credentials.
- read and write access.
- catalog format.
- current snapshot.
- unsafe runtime settings.

Keep it read-only by default. Add checks when they answer a common support
question.

### Make recovery understandable

Use the existing export, import, checkpoint, and verification operations as the
first backup and recovery workflow.

Document and test this path:

1. Export a catalog.
2. Import it into a new catalog.
3. Verify the restored state.
4. Make a new write.

Add separate backup and restore command names only if the existing commands are
confusing in real use.

### Keep output useful

Add JSON output to commands that operators need to automate, starting with
doctor, inspect, verify, and recovery operations.

Do not add a configuration file until flags and environment variables have
shown a real limitation.

Ship a Docker image only when the project can build, test, and publish one.
Until then, remove Docker commands from the product documentation.

## Later: scale with evidence

The current catalog reader returns data-file listings as one in-memory list. That is a
reasonable small-catalog default, but it may not hold for very large catalogs.

Start with a reproducible benchmark that measures:

- catalog size.
- time to first row.
- total latency.
- peak memory.
- object-store requests.
- bytes transferred.

If the measurements show a problem:

1. Add snapshot-aware pagination to the catalog API.
2. Add streaming with cancellation and backpressure.
3. Bound PG-wire result buffering.
4. Add resource limits where users can create unbounded work.
5. Improve metrics for the bottleneck that the benchmark found.

Run a small AWS and GCS validation workload after the local behavior is
measured. Run a long soak only when there is a deployment that needs it. Cloud
testing is evidence for support claims, not a reason to delay unrelated fixes.

## Keep deferred

Do not make these roadmap work:

- a general PostgreSQL implementation.
- a native DuckDB extension.
- new language bindings without a named user and maintainer.
- new engines without executable compatibility tests.
- a general-purpose fact-store API.
- a tiered NVMe cache without profiling evidence.
- a large PG-wire rewrite without a change that needs it.
- a 1.0 deadline.

## How we choose the next task

Work belongs in the active queue when it does at least one of these things:

- fixes a correctness or security problem.
- makes the supported user path work.
- removes a confusing or duplicate interface.
- answers a real operator question.
- fixes a measured bottleneck.

If a task does none of these, defer it.
