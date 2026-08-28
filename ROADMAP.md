# RockLake Roadmap

- **Status:** Active
- **Current baseline:** v0.47.17
- **Roadmap horizon:** v0.48.0 through v0.53.x
- **1.0:** Deferred intentionally
- **Primary objective:** Simplification, hardening, operational usability, bounded scale, and real-world validation

---

## Purpose

RockLake has completed its primary correctness-hardening phase through v0.47.17.

The project now has mature foundations for:

- atomic catalog commits.
- writer fencing and conflict handling.
- snapshot-correct reads.
- metadata isolation.
- checkpoint and retention safety.
- catalog export and import fidelity.
- read-only operation.
- path correctness.
- DataFusion scan correctness.
- fault injection.
- backend lifecycle testing.
- DuckLake value-level conformance.
- production failure certification.

The next development phase should not optimize for feature count.

The goal is to turn the existing core into a product that is:

- smaller.
- easier to understand.
- easier to install.
- safer by default.
- easier to operate.
- easier to maintain.
- more transparent about what is supported.
- bounded under large workloads.
- validated against real object-storage environments.

The guiding rule for this roadmap is:

> **Reduce first. Harden second. Simplify operation third. Scale only where measurements justify it.**

---

## Roadmap principles

### Correctness remains non-negotiable

No usability, performance, or compatibility change may weaken the correctness guarantees established through v0.47.17.

Future releases must preserve:

- atomic logical commits.
- retry-safe failure behavior.
- writer fencing.
- rollback integrity.
- snapshot isolation.
- historical-read correctness.
- table and metadata isolation.
- fail-closed reader behavior.
- recovery invariants.
- export and import fidelity.

The v0.47.17 production-failure certification suite remains a permanent regression gate.

---

### Prefer removal to expansion

Before adding a new:

- CLI command.
- configuration option.
- network listener.
- crate.
- integration.
- cache.
- background service.
- storage abstraction.
- language binding.

the project should first determine whether an existing interface can be simplified or removed.

A new feature must justify its long-term testing and maintenance cost.

---

### Pre-1.0 compatibility is selective

RockLake remains pre-1.0.

This period should be used to clean up interfaces that would become expensive to preserve later.

Compatibility should be maintained when:

- real users depend on the interface.
- the behavior is explicitly documented as supported.
- removal would create significant migration cost.
- the maintenance burden is low.

Historical interfaces should not survive solely because they once existed.

---

### One canonical path

Every important workflow should have one recommended interface.

Examples:

```text
doctor → serve → ATTACH
```

```text
backup create → restore plan → restore apply
```

```text
gc plan → gc apply
```

Alternative low-level APIs may exist, but documentation should lead users toward one safe, tested path.

---

### Safe defaults

Safe behavior should require less configuration than unsafe behavior.

Examples:

- loopback binding by default.
- SCRAM for password authentication.
- explicit opt-in to public network exposure.
- no unauthenticated secondary listeners.
- secrets sourced from environment variables or files rather than process arguments.
- destructive operations use plan and apply.
- reader mode carries no mutation capability where practical.

---

### Documentation is a tested interface

Documentation is part of the product.

A documented command that does not work is a regression.

A documented security capability that is not implemented is a defect.

A documented compatibility claim without executing coverage is not a supported claim.

Primary examples and quickstarts should therefore run in CI.

---

### Measurement before optimization

Performance work should proceed in this order:

1. reproduce a workload.
2. measure it.
3. profile it.
4. identify the dominant bottleneck.
5. implement the smallest appropriate change.
6. measure again.

Complex caching or storage tiers should not be built speculatively.

---

## Priority definitions

Every roadmap item is assigned a priority.

### P0: Release blocking

A P0 item must be complete before the release is considered finished.

P0 items generally address:

- correctness.
- security.
- unsafe defaults.
- misleading product behavior.
- release integrity.
- core usability.

---

### P1: Required

P1 work is expected in the release and should only move when there is a concrete reason.

P1 items generally address:

- maintainability.
- operator experience.
- testing depth.
- observability.
- important cleanup.

---

### P2: Opportunistic

P2 work is desirable but may be deferred without changing the central purpose of the release.

P2 work should not delay P0 or P1 completion.

---

## Release overview

| Release | Theme | Primary outcome |
|---|---|---|
| **v0.48.0** | Interface reduction and safe defaults | Fewer public interfaces with immediate network and logging risks closed |
| **v0.49.0** | Authentication and release integrity | Secure authentication and a trusted release chain |
| **v0.50.0** | First-run experience | Clear diagnostics, startup, and local development |
| **v0.51.0** | Operator workflows and distribution | Tested backup, maintenance, and container workflows |
| **v0.52.0** | Bounded scale and observability | Predictable behavior on large catalogs |
| **v0.53.x** | Real-cloud validation and maintenance | Production confidence based on executed cloud workloads |

Planning window:

- **Now:** v0.48.0.
- **Next:** v0.49.0 through v0.51.0.
- **Later:** v0.52.0 and v0.53.x.

---

## v0.48.0: Interface reduction and safe defaults

### Objective

v0.48.0 closes known network and logging risks, removes duplicate interfaces, and makes the primary documentation executable.

The release should delete more public interfaces than it adds.

---

### P0: Bind to loopback by default

Default:

```text
127.0.0.1:5432
```

Public exposure requires an explicit bind address:

```text
--bind 0.0.0.0:5432
```

#### Acceptance criteria

- [ ] The default configuration is inaccessible from remote network interfaces.
- [ ] The README reflects the safe default.
- [ ] Container deployments explicitly configure their bind address.
- [ ] Network tests verify both default and explicit behavior.

---

### P0: Remove unconditional raw SQL logging

Remove direct SQL printing from normal output.

Structured tracing may include:

- operation kind.
- query fingerprint.
- duration.
- affected catalog object.
- trace ID.

Raw SQL requires explicit debug configuration and redaction.

#### Acceptance criteria

- [ ] Normal output contains no raw SQL statement text.
- [ ] Logs omit secrets and literals by default.
- [ ] Logs use the tracing infrastructure.
- [ ] Query fingerprints and trace IDs preserve correlation.

---

### P0: Remove or secure the DataFusion secondary listener

Remove the second network listener unless a supported client requires it.

If retained, it must inherit:

- bind policy.
- TLS.
- authentication.
- access mode.
- session limits.
- tracing.
- security tests.

#### Acceptance criteria

Either:

- [ ] The secondary listener is removed.

Or all of the following:

- [ ] No configuration creates an unauthenticated public secondary endpoint by accident.
- [ ] Tests cover inherited security configuration.
- [ ] Documentation names the supported client that requires the listener.

---

### P0: Replace the dual CLI implementation

#### Problem

The CLI currently has both:

- typed Clap parsing; and
- legacy manual argument parsing.

Successfully parsed Clap structures may be converted back into synthetic argument arrays before command execution.

This creates:

- duplicate validation.
- duplicated flag definitions.
- inconsistent error behavior.
- unnecessary code.
- compatibility complexity.
- increased testing burden.

#### Requirements

- Remove fallback legacy argument parsing.
- Remove typed-command-to-synthetic-argv conversion.
- Dispatch directly from typed Clap command structures.
- Each command must have a typed configuration object.
- Command handlers must accept typed configuration rather than raw argument arrays.
- Unknown arguments must fail through Clap.
- Supported legacy names may temporarily exist only as explicit aliases.

#### Target architecture

```text
argv
  ↓
Clap
  ↓
Typed command
  ↓
Typed handler
  ↓
Catalog API
```

#### Acceptance criteria

- [ ] Exactly one top-level CLI parser exists.
- [ ] No production command reconstructs synthetic `argv`.
- [ ] No command handler reparses raw arguments.
- [ ] `rocklake --help` and every subcommand help page are generated by Clap.
- [ ] Invalid arguments have deterministic exit codes.
- [ ] CLI conformance tests cover every public flag.
- [ ] Legacy aliases retained for compatibility are explicitly listed and marked deprecated.

---

### P0: Eliminate snapshot sentinel ambiguity

#### Problem

Numeric snapshot IDs must not have hidden meanings such as `0 == latest`.

A snapshot number should always refer to a snapshot.

#### Requirements

Introduce an explicit snapshot selector.

Preferred model:

```rust
pub enum SnapshotRef {
    Latest,
    At(SnapshotId),
}
```

Equivalent separate APIs are acceptable where more ergonomic.

#### Required scope

Review:

- `rocklake-client`.
- FFI.
- DataFusion interfaces.
- CLI snapshot arguments.
- reader helper APIs.
- bindings.

#### Acceptance criteria

- [ ] Public Rust APIs do not use `0` as the recommended latest-snapshot sentinel.
- [ ] Documentation distinguishes `Latest` from an explicit snapshot.
- [ ] Snapshot `0`, when accepted internally, has exactly one documented semantic.
- [ ] Tests verify both latest and explicit historical reads.
- [ ] FFI and bindings expose unambiguous behavior.

---

### P0: Repository documentation truthfulness audit

#### Requirements

Audit every primary product claim against current implementation.

At minimum verify:

- current version.
- install instructions.
- binary names.
- Docker image availability.
- container base and runtime assumptions.
- environment variables.
- authentication modes.
- TLS behavior.
- mTLS behavior.
- certificate reload behavior.
- supported object stores.
- supported DuckDB versions.
- supported DuckLake versions.
- DataFusion behavior.
- read-replica behavior.
- backup and restore semantics.
- benchmark environment.
- language bindings.

Every capability must be labeled as one of:

- Stable.
- Supported.
- Experimental.
- Internal.
- Planned.

Unsupported claims must be removed rather than preserved aspirationally.

#### Acceptance criteria

- [ ] README describes the current release.
- [ ] README examples use current supported installation paths.
- [ ] No primary documentation describes unimplemented CLI flags.
- [ ] No primary documentation claims mTLS unless mTLS is implemented and tested.
- [ ] No primary documentation claims certificate reload unless it is implemented and tested.
- [ ] Compatibility documentation matches executed tests.
- [ ] Docker documentation matches the actual shipped image.
- [ ] Documentation build runs with strict link and reference validation.

---

### P0: Execute the primary quickstart in CI

#### Required scenario

CI must execute an end-to-end user workflow using the public binary.

Minimum sequence:

1. start RockLake.
2. connect DuckDB.
3. load DuckLake.
4. attach catalog.
5. create schema.
6. create table.
7. insert rows.
8. query rows.
9. inspect snapshots.
10. historical read.
11. run a diagnostic operation.
12. terminate RockLake gracefully.

#### Acceptance criteria

- [ ] The public local quickstart maps directly to an executable CI script.
- [ ] Commands are copied from or generated from the same canonical source as documentation.
- [ ] CI fails if documentation uses an invalid command.
- [ ] Expected values, not merely process exit codes, are checked.
- [ ] The quickstart is included in release-blocking checks.

---

### P1: Reduce operator CLI commands

Review every top-level command.

Current capabilities should be grouped conceptually into:

#### Server

```text
rocklake serve
rocklake doctor
```

#### Data protection

```text
rocklake backup ...
rocklake restore ...
rocklake checkpoint ...
```

#### Maintenance

```text
rocklake gc ...
rocklake excise ...
rocklake repair ...
```

#### Inspection

```text
rocklake inspect ...
rocklake verify ...
```

#### Development and internal

```text
rocklake dev corpus ...
rocklake dev compatibility ...
```

Exact naming may differ, but developer-only tooling should not compete with normal operational commands.

#### Candidates for consolidation

- `export`.
- `export-catalog`.
- `diagnose`.
- selected `inspect` functionality.
- migration subcommands.
- corpus tooling.

#### Acceptance criteria

- [ ] Every top-level command has a documented operator persona and use case.
- [ ] Duplicate export concepts are eliminated.
- [ ] Developer and conformance operations are separated from ordinary operator workflows.
- [ ] Destructive commands use a consistent `plan and apply` model.
- [ ] Machine-consumable commands support structured output where appropriate.

---

### P1: Replace the historical roadmap with a live roadmap

The live roadmap should contain:

- current baseline.
- Now.
- Next.
- Later.
- explicit non-goals.
- release acceptance criteria.

Historical release history belongs in `CHANGELOG.md`.

Old implementation assessments and completed project plans should be removed from normal repository navigation.

#### Recommended root

```text
README.md
CHANGELOG.md
ROADMAP.md
CONTRIBUTING.md
SECURITY.md
LICENSE
```

Additional root documents require a durable reason to exist.

#### Acceptance criteria

- [x] `ROADMAP.md` contains only active and future work and brief historical context.
- [ ] Completed one-off implementation reports are removed from the repository root.
- [ ] Superseded planning documents are archived or removed.
- [ ] Documentation navigation does not expose historical implementation archaeology to normal users by default.

---

### P2: Remove stale compatibility aliases

Identify flags and command forms retained only for historical reasons.

For each alias:

- document real use.
- retain temporarily with warning; or
- remove.

Pre-1.0 aliases should not become permanent without justification.

---

### v0.48.0 release gate

v0.48.0 is complete when:

- [ ] one CLI parser exists.
- [ ] snapshot selection is explicit.
- [ ] the default listener binds to loopback.
- [ ] normal logs contain no raw SQL.
- [ ] no unauthenticated secondary listener can be exposed by accident.
- [ ] primary documentation matches actual implementation.
- [ ] local quickstart executes in CI.
- [ ] duplicate operator commands are reduced.
- [ ] repository documentation is materially smaller.
- [ ] legacy interfaces without demonstrated need are removed.
- [ ] v0.47.17 correctness certification remains green.

---

## v0.49.0: Authentication and release integrity

### Objective

Make password authentication safe and connect source control, certification, tagging, and published artifacts into one enforceable release process.

---

### P0: SCRAM as default password authentication

#### Requirements

When password authentication is enabled:

- SCRAM-SHA-256 is the default protocol.
- plaintext password authentication is not the default.
- insecure compatibility modes must be explicit.
- unsafe auth-without-TLS configurations produce strong warnings or fail unless intentionally overridden.

#### Acceptance criteria

- [ ] normal authenticated server startup uses SCRAM.
- [ ] SCRAM behavior is network-tested.
- [ ] DuckDB supported client versions connect successfully.
- [ ] compatibility behavior is documented.
- [ ] cleartext password transport cannot occur silently.

---

### P0: Protect the default branch

Repository controls should require:

- pull request workflow.
- release-critical status checks.
- no force pushes.
- restricted direct push.
- controlled automation exceptions.

#### Acceptance criteria

- [ ] main branch protection enabled.
- [ ] correctness and release gates configured as required checks.
- [ ] failed certification prevents merge where designated.
- [ ] release automation does not bypass source review except for explicitly controlled actions.

---

### P0: Replace post-tag source mutation

Release version must be committed before tagging.

Required process:

```text
prepare release commit
        ↓
run full certification
        ↓
certified SHA
        ↓
create version tag
        ↓
build tagged SHA
        ↓
publish
```

#### Acceptance criteria

- [ ] release workflow never pushes a version bump to main after a tag event.
- [ ] binary `--version` equals release tag.
- [ ] Cargo workspace version equals release tag.
- [ ] published source tag contains the exact version.
- [ ] release artifacts are built from the tagged SHA.

---

### P0: One release certification workflow

Create a reusable certification workflow.

Full certification should cover:

- fmt.
- clippy.
- workspace tests.
- v0.47.17 production failure suite.
- DuckLake value conformance.
- public interface manifest.
- LocalFS.
- MinIO.
- GCS emulator.
- Azure emulator.
- Windows.
- security tests.
- docs smoke.
- Miri.
- sanitizers where applicable.
- compatibility manifest.

External infrastructure required for certification must either:

- run successfully; or
- block certification.

It must not silently become optional.

#### Acceptance criteria

- [ ] a single certification status represents the complete release matrix.
- [ ] releases require certification for the exact SHA being tagged.
- [ ] build-only jobs cannot satisfy execution gates.
- [ ] certification report is generated from job evidence rather than manually asserted.

---

### P1: Secret handling policy

Prefer:

```text
ROCKLAKE_AUTH_PASSWORD
ROCKLAKE_AUTH_PASSWORD_FILE
ROCKLAKE_ENCRYPTION_KEY_FILE
```

Exact names may differ.

Command-line secret arguments should be removed, deprecated, or clearly classified as unsafe development conveniences.

#### Acceptance criteria

- [ ] production documentation does not recommend secret CLI arguments.
- [ ] secrets can be supplied from files or environment variables.
- [ ] startup errors never echo secret values.
- [ ] diagnostic output redacts secrets.

---

### P1: Dependency advisory lifecycle

Every ignored advisory must include:

- advisory ID.
- affected dependency.
- reason for temporary exception.
- upstream tracking reference.
- mitigation.
- review date or expiration date.

#### Acceptance criteria

- [ ] no advisory ignore exists without justification.
- [ ] dependency upgrades remove resolved exceptions.
- [ ] new advisories fail CI unless explicitly reviewed.
- [ ] security review is part of release preparation.

---

### P1: Supply-chain provenance

Add where practical:

- SBOM.
- GitHub artifact attestations.
- signed release tags.
- signed container images.
- build metadata.
- provenance.

Critical release workflow actions should be pinned more strictly over time.

#### Acceptance criteria

- [ ] release artifacts can be traced to source SHA.
- [ ] checksums are published.
- [ ] SBOM is attached to releases or otherwise distributed.
- [ ] provenance or attestation exists for primary release artifacts.

---

### P1: Add `SECURITY.md`

Include:

- supported versions.
- vulnerability reporting path.
- expected response process.
- disclosure policy.
- dependency security policy.

---

### v0.49.0 release gate

- [ ] password auth defaults to SCRAM.
- [ ] secrets have production-safe input paths.
- [ ] main is protected.
- [ ] releases build exact tagged and certified source.
- [ ] full certification is release-enforced.
- [ ] dependency exceptions have lifecycle ownership.

---

## v0.50.0: First-run experience

### Objective

Make RockLake usable without knowledge of SlateDB internals, catalog key structure, writer epochs, or object-store implementation details.

The primary user journey should become:

```text
install
  ↓
doctor
  ↓
serve
  ↓
ATTACH
```

---

### P0: Introduce `rocklake doctor`

Example:

```bash
rocklake doctor s3://my-bucket/catalog
```

or an equivalent canonical syntax.

#### Checks

At minimum:

- URI validity.
- credentials.
- object-store connectivity.
- catalog prefix existence.
- read permission.
- write permission where required.
- list permission where required.
- catalog format.
- migration state.
- snapshot state.
- reader and writer eligibility.
- encryption configuration.
- DuckLake compatibility.
- known unsafe runtime configuration.
- basic storage latency.

#### Output

Human-readable by default.

Machine-readable:

```text
--output json
```

#### Exit behavior

- `0`: ready.
- non-zero: actionable failure.

#### Acceptance criteria

- [ ] fresh local catalog passes appropriate preflight.
- [ ] valid cloud catalog passes without mutation unless explicitly requested.
- [ ] permission failures identify the missing capability.
- [ ] format or migration incompatibility is clearly reported.
- [ ] JSON schema is stable for the release series.

---

### P0: Improve server startup UX

Startup output should communicate:

- version.
- catalog URI.
- serving mode.
- supported DuckLake version.
- listen address.
- TLS state.
- auth state.
- metrics state.
- readiness.

Where practical, print a copyable DuckDB connection example.

Example:

```text
RockLake 0.50.0

Catalog       s3://example/catalog
Mode          writer
DuckLake      1.0
Listener      127.0.0.1:5432
TLS           disabled
Authentication disabled
Status        ready

DuckDB:
ATTACH 'host=127.0.0.1 port=5432' AS lake (TYPE ducklake);
```

#### Acceptance criteria

- [ ] successful startup produces concise actionable output.
- [ ] unsafe configurations produce visible warnings.
- [ ] machine logs remain available through tracing.
- [ ] startup messages do not expose secrets.

---

### P0: Zero-friction local development

Preferred command:

```bash
rocklake serve ./lake
```

or equivalent.

It should:

- create the local catalog if appropriate.
- bind safely.
- use development-appropriate defaults.
- clearly state security status.
- output DuckDB connection instructions.

#### Acceptance criteria

- [ ] new user can create and query a local catalog without cloud credentials.
- [ ] no additional required configuration for the basic case.
- [ ] the path is tested on Linux, macOS, and Windows where supported.

---

### P1: Typed configuration file

Add a canonical configuration format:

```text
rocklake.toml
```

Suggested precedence:

```text
built-in defaults
    ↓
configuration file
    ↓
environment variables
    ↓
CLI
```

#### Requirements

- strict validation.
- unknown keys rejected or explicitly warned.
- secrets may reference files or environment variables.
- generated example configuration.
- effective configuration inspection with redaction.

Possible command:

```bash
rocklake config check
```

#### Acceptance criteria

- [ ] equivalent CLI and configuration values produce identical behavior.
- [ ] invalid configuration fails before catalog mutation.
- [ ] effective configuration can be inspected safely.
- [ ] configuration schema is documented from source where practical.

---

### v0.50.0 release gate

A first-time user must be able to:

1. install RockLake.
2. run `doctor`.
3. start a local catalog.
4. copy the provided DuckDB attach command.
5. create and query data.
6. diagnose a bad configuration.

The v0.47.17 correctness certification must remain green.

---

## v0.51.0: Operator workflows and distribution

### Objective

Give operators tested workflows for data protection, maintenance automation, and container deployment.

---

### P0: Make backup and restore first-class concepts

Low-level export and import may remain internally, but operators should see:

```text
rocklake backup create
rocklake backup inspect

rocklake restore plan
rocklake restore apply
```

#### Backup requirements

- snapshot-consistent.
- versioned metadata.
- integrity metadata.
- source catalog identity.
- creation time.
- snapshot identifier.
- checksum where practical.

#### Restore requirements

- validate before mutation.
- plan before apply.
- atomic publication.
- reconstruct counters and indexes.
- verify post-restore invariants.
- refuse unsafe overwrite without explicit action.

#### Acceptance criteria

- [ ] backup → new catalog restore → next write is tested end-to-end.
- [ ] interrupted restore cannot expose partial catalog state.
- [ ] restore plan reports exactly what will change.
- [ ] successful restore automatically runs verification.
- [ ] docs distinguish backup, checkpoint, export, and migration.

---

### P1: Standardize operational output

Operator commands should support:

```text
--output human
--output json
```

where useful.

Potential commands:

- doctor.
- inspect.
- verify.
- backup.
- restore.
- gc plan.
- excise plan.
- repair plan.

JSON output should avoid human-format scraping.

---

### P1: Uniform plan and apply semantics

High-impact operations should behave consistently.

```text
rocklake gc plan
rocklake gc apply

rocklake excise plan
rocklake excise apply

rocklake repair plan
rocklake repair apply

rocklake restore plan
rocklake restore apply
```

#### Acceptance criteria

- [ ] plan mode makes no persistent changes.
- [ ] apply validates that assumptions have not materially changed.
- [ ] JSON plan can be archived.
- [ ] destructive actions are explicit.

---

### P0: Make Docker either real or absent

If RockLake claims an official container image, it must be a first-class release artifact.

#### Required container support

- GHCR publication.
- version tag.
- immutable digest.
- tested startup.
- non-root execution.
- current CA bundle.
- correct environment handling.
- health check.
- multi-architecture images where practical.
- SBOM.
- signing and attestation.

The container should rely on RockLake's own environment parsing rather than shell-variable expansion inside JSON-form `CMD`.

If this standard is not met, official-container claims should be removed until it is.

#### Acceptance criteria

- [ ] documented `docker run` command works verbatim.
- [ ] image tag matches binary version.
- [ ] image is created by release workflow.
- [ ] image startup is tested before publication.
- [ ] health check exercises real RockLake readiness.
- [ ] container docs are version-current.

---

### P2: Installation ergonomics

Consider after the core distribution story is stable:

- shell installer.
- Homebrew.
- cargo install documentation.
- package-manager integrations.

Do not add package channels that cannot be continuously maintained.

---

### v0.51.0 release gate

- [ ] Backup and restore pass an end-to-end recovery test.
- [ ] Destructive operations use consistent plan and apply semantics.
- [ ] Operator commands expose stable structured output where needed.
- [ ] The documented container command runs the published image.
- [ ] The v0.47.17 correctness certification remains green.

---

## v0.52.0: Bounded scale and observability

### Objective

Ensure large catalogs do not cause unbounded memory use or unnecessarily delay first results.

Optimize behavior before introducing speculative caching.

---

### P0: Paginated data-file listing

Introduce snapshot-aware pagination.

Conceptual interface:

```rust
list_data_files_paged(
    table_id,
    snapshot,
    page_size,
    continuation_token
)
```

#### Continuation token requirements

Tokens should be:

- opaque.
- validated.
- snapshot-aware.
- independent of public knowledge of internal key encoding.
- rejected if incompatible with request context.

#### Acceptance criteria

- [ ] 100k+ file listing can be traversed without one `Vec` containing every row.
- [ ] page traversal returns exactly-once logical coverage for a stable snapshot.
- [ ] invalid tokens fail cleanly.
- [ ] historical snapshot pagination is correct.
- [ ] page-size limit is enforced.

---

### P0: Async streaming API

Provide streaming for high-cardinality operations.

Conceptual:

```rust
stream_data_files(...)
```

#### Requirements

- bounded channel and buffer.
- cancellation safety.
- backpressure.
- error propagation.
- snapshot consistency.

#### Acceptance criteria

- [ ] consumer may process files incrementally.
- [ ] producer does not unboundedly outrun consumer.
- [ ] cancellation releases resources.
- [ ] mid-stream storage failure propagates as error rather than truncated success.

---

### P0: PG-wire incremental result delivery

Large metadata responses should stream where possible.

#### Metrics

Measure:

- time to first row.
- total response time.
- rows per second.
- bytes per second.
- peak buffered rows.
- peak RSS.

#### Acceptance criteria

- [ ] large scans do not require full response materialization.
- [ ] slow clients apply backpressure.
- [ ] disconnected clients cancel remaining work.
- [ ] resource use remains bounded by documented limits.

---

### P0: Explicit resource limits

Introduce or consolidate limits for:

- active sessions.
- active scans.
- stream queue depth.
- maximum page size.
- buffered rows.
- relevant response memory.
- operational concurrency.

#### Acceptance criteria

- [ ] every potentially unbounded user-controlled collection has a limit or streaming behavior.
- [ ] limits are observable.
- [ ] exhaustion produces explicit errors.
- [ ] defaults are safe for modest deployments.

---

### P1: Observability redesign

Prefer a small useful metric set over many low-value counters.

#### Core metrics

##### Request

- request duration histogram.
- SQL classification latency.
- response rows.
- time to first row.

##### Catalog

- snapshot read latency.
- commit latency.
- conflicts.
- current snapshot.
- reader refresh lag.

##### Object store

- operations by type.
- bytes read.
- bytes written.
- latency.
- retries.
- errors.

##### Process

- RSS.
- active sessions.
- active scans.
- queue depth.
- stream backpressure.
- task and bridge pressure where relevant.

#### Acceptance criteria

- [ ] standard dashboard can identify CPU, memory, storage latency, and queue bottlenecks.
- [ ] histograms are true histogram instruments.
- [ ] metric names and labels have documented cardinality constraints.
- [ ] no metric embeds uncontrolled table or query values in labels.

---

### P1: End-to-end trace correlation

Trace:

```text
connection
  ↓
request
  ↓
SQL classifier
  ↓
executor
  ↓
catalog operation
  ↓
SlateDB
  ↓
object store
```

#### Acceptance criteria

- [ ] one trace ID can correlate a slow user request with catalog and storage work.
- [ ] errors record the relevant trace ID.
- [ ] tracing does not include sensitive SQL values by default.
- [ ] tracing overhead is measured.

---

### P1: Slow-operation reporting

Provide configurable logging for operations exceeding thresholds.

Examples:

- slow PG query.
- slow snapshot open.
- slow file scan.
- slow object-store request.
- slow commit.

Use operation identifiers or fingerprints rather than raw sensitive payloads.

---

### P1: Large-catalog benchmark suite

Minimum scenarios:

- 10k files.
- 100k files.
- 1M files where practical.
- wide schemas.
- many tables.
- historical snapshots.
- paginated reads.
- streaming reads.
- concurrent readers.

Measure:

- p50, p95, p99, and p999.
- time to first row.
- peak RSS.
- object-store operations.
- bytes transferred.

---

### v0.52.0 release gate

- [ ] high-cardinality listings support pagination.
- [ ] large reads support bounded streaming.
- [ ] PG-wire large responses are bounded.
- [ ] cancellation and backpressure is tested.
- [ ] memory and resource limits are explicit.
- [ ] operational metrics identify primary bottlenecks.
- [ ] large-scale benchmark includes memory and first-row measurements.

---

## v0.53.x: Real-cloud validation and maintenance

### Objective

Validate RockLake under realistic cloud conditions and use those results to determine the next architectural priorities.

v0.53 should be a release series rather than one feature bundle.

Example:

```text
v0.53.0: AWS baseline
v0.53.1: GCS baseline
v0.53.2: multi-node soak
v0.53.3: dependency and storage upgrade
...
```

Exact sequencing should follow engineering needs.

---

### P0: Real AWS S3 benchmark

Use current production dependencies and a documented environment.

#### Minimum topology

- 1 writer.
- 1 reader.
- 4 readers.
- 16 readers.

#### Workloads

- catalog open.
- latest snapshot refresh.
- create tables.
- add files.
- list files.
- historical reads.
- backup.
- verification.
- writer replacement.

#### Scale points

At minimum:

- small catalog.
- 10k files.
- 100k files.
- larger scale where cost permits.

#### Report

Record:

- region.
- availability zone topology.
- EC2 instance.
- S3 class.
- RockLake SHA and version.
- SlateDB version.
- request counts.
- bytes.
- p50, p95, p99, and p999.
- RSS.
- cold-start latency.
- estimated cost.

#### Acceptance criteria

- [ ] raw benchmark procedure is committed.
- [ ] results are reproducible.
- [ ] no projected or local values are labeled AWS measurements.
- [ ] reader and writer correctness invariants are checked during load.

---

### P0: Real GCS benchmark

Run comparable scenarios on GCS.

The purpose is not to force identical latency between clouds.

The purpose is to verify:

- correctness.
- lifecycle behavior.
- error behavior.
- operational viability.
- cost and performance characteristics.

---

### P0: Multi-node soak

Run sustained workloads against real object storage.

Target duration:

**24 hours** for formal soak certification where feasible.

#### Workload

Continuously perform:

- commits.
- reads.
- historical reads.
- reader refresh.
- writer restart.
- reader restart.
- checkpoint creation.
- verification.
- backup.
- GC where safe.

Inject:

- process kill.
- network delay.
- object-store throttling.
- transient errors.
- writer replacement.
- reader rolling restart.
- credential refresh and expiration scenarios where feasible.

#### Acceptance criteria

- [ ] no invariant violations.
- [ ] no silent wrong results.
- [ ] no unbounded RSS trend.
- [ ] all committed snapshots remain readable within retention policy.
- [ ] reader convergence remains bounded.
- [ ] writer takeover behaves correctly.
- [ ] expected transient failures recover.

---

### P1: Benchmark execution, not JSON validation

Benchmark files may remain as historical artifacts.

However, release and performance gates should execute benchmark code for important baselines.

#### Required benchmark metadata

Every published result must contain:

- version.
- commit SHA.
- date.
- Rust version.
- SlateDB version.
- object_store version.
- machine.
- backend.
- dataset.
- workload.
- repetitions.
- raw results.
- summary.

#### Acceptance criteria

- [ ] CI or designated benchmark infrastructure runs actual benchmark workloads.
- [ ] committed JSON alone cannot satisfy a performance certification gate.
- [ ] results identify projections explicitly.
- [ ] stale baselines are retired.

---

### P0: Dependency modernization

Use v0.53.x to review core dependencies.

Priority:

- SlateDB.
- object_store.
- DataFusion.
- pgwire.
- sqlparser.
- Rust MSRV.
- cryptography and TLS dependencies.

Goals:

- remove ignored advisories.
- reduce duplicated dependency versions.
- retire compatibility shims.
- validate performance after upgrades.

Each significant storage upgrade must rerun:

- production failure certification.
- backend matrix.
- import and export.
- read-only behavior.
- soak-critical tests.

---

### P1: Maintenance budget

Reserve explicit release capacity for:

- flaky test removal.
- CI runtime reduction.
- dead-code removal.
- unused feature removal.
- documentation pruning.
- obsolete tests.
- dependency cleanup.
- tracing cleanup.
- API deprecations.
- benchmark maintenance.

A mature project requires scheduled subtraction.

---

### P1: Production-shaped upgrade testing

Test rolling upgrades across supported adjacent releases.

Scenarios:

```text
old writer → new writer
old reader + new writer
new reader + old writer
backup old → restore new
```

Where unsupported, fail clearly rather than permitting ambiguous mixed-version operation.

---

### v0.53.x exit criteria

The v0.53 series is complete when:

- [ ] real AWS measurements exist.
- [ ] real GCS measurements exist.
- [ ] sustained multi-node soak has completed.
- [ ] no known severe correctness findings remain.
- [ ] core dependency advisories are substantially reduced.
- [ ] benchmark infrastructure executes rather than validates static reports.
- [ ] major performance bottlenecks have been identified using real evidence.
- [ ] the next optimization roadmap is based on observed profiles.

---

## Deferred work

The following work is intentionally outside the immediate roadmap.

---

### Tiered NVMe cache

Status:

**Deferred pending real-cloud profiling.**

Do not build a tiered L1, L2, and L3 cache merely because remote storage is assumed to be slow.

Real-cloud measurements may identify different bottlenecks:

- metadata scan amplification.
- manifest reads.
- insufficient indexes.
- serialization.
- object-store request count.
- page sizing.
- SlateDB configuration.
- concurrency.
- CPU.

An NVMe cache should be reconsidered only if profiling demonstrates substantial benefit.

---

### Native DuckDB extension

Continue tracking upstream feasibility.

Do not make this a primary roadmap item unless:

- the relevant DuckDB extension APIs stabilize.
- integration materially improves the user experience.
- there is concrete user demand.

The PG-wire path should remain excellent independently.

---

### New language bindings

Do not add bindings primarily for completeness.

A binding should require:

- real consumer.
- maintained CI.
- package distribution owner.
- compatibility policy.

Existing bindings should be reviewed under the same standard.

Unsupported bindings should be clearly experimental.

---

### New engines and clients

Do not add "supported" integrations without executable ongoing coverage.

A compatibility claim requires:

- version range.
- real execution.
- CI ownership.
- failure semantics.
- documentation.

---

### General-purpose fact store

The underlying architecture may eventually support broader use cases.

That direction should not distract from making the DuckLake catalog product small and excellent.

No generalized fact-store API belongs in this roadmap.

---

### PG-wire module decomposition

Status:

**Deferred until a feature or fix needs it.**

Large modules alone do not justify a semantic rewrite. Split a module when the active change cannot be tested or reviewed cleanly without doing so.

---

### 1.0

1.0 is intentionally deferred.

There is no requirement to promote RockLake to 1.0 at the end of this roadmap.

The project should remain pre-1.0 until maintainers decide the interface, operations model, and compatibility commitments are sufficiently stable.

No artificial deadline should drive that decision.

---

## Project policies

`ROADMAP.md` records release order, priorities, and exit criteria. Durable rules have one owner:

- compatibility claims live in [`docs/compatibility.md`](docs/compatibility.md).
- release rules live in [`docs/contributing/release-process.md`](docs/contributing/release-process.md).
- dependency rules live in [`docs/contributing/code-style.md`](docs/contributing/code-style.md#dependency-policy).
- benchmark rules live in [`docs/performance/benchmarks.md`](docs/performance/benchmarks.md).
- product boundaries live in [`docs/concepts/bounded-sql.md`](docs/concepts/bounded-sql.md) and [`docs/design-decisions/what-rocklake-is-not.md`](docs/design-decisions/what-rocklake-is-not.md).

The v0.48 truthfulness audit must reconcile these documents with the current code before treating their claims as current.

---

## Long-term success criteria

This roadmap succeeds if RockLake emerges from v0.53.x with the following properties.

### Product

- one obvious setup path.
- concise documentation.
- accurate compatibility claims.
- low configuration burden.

### Correctness

- failure certification remains continuously green.
- no known silent wrong-result paths.
- backups and restores are routinely verified.

### Security

- safe bind defaults.
- secure password authentication.
- no accidental public unauthenticated endpoints.
- governed dependency vulnerabilities.
- traceable release artifacts.

### Operations

- useful preflight diagnostics.
- stable JSON output.
- explicit backup and restore.
- understandable metrics.
- bounded shutdown and failure behavior.

### Scale

- pagination.
- streaming.
- backpressure.
- bounded memory.
- real-cloud performance evidence.

### Maintenance

- smaller CLI implementation.
- smaller active documentation set.
- fewer stale aliases.
- manageable executor structure.
- regular dependency modernization.
- explicit technical-debt budget.

---

## Immediate work queue

The first work after adopting this roadmap should be performed in this order.

### P0 immediate

1. Bind the default listener to loopback.
2. Remove raw SQL from normal logs.
3. Remove or secure the secondary DataFusion listener.
4. Replace legacy CLI dispatch.
5. Fix snapshot sentinel APIs.
6. Audit README and documentation claims.
7. Execute the primary quickstart in CI.

### P1 immediate

8. Remove completed implementation reports from normal navigation.
9. Consolidate duplicate CLI commands.
10. Protect `main`.
11. Fix release version and tag sequencing.
12. Build the reusable certification workflow.
13. Add `SECURITY.md`.
14. Review ignored dependency advisories.

### Then

15. Default password authentication to SCRAM.
16. Build `rocklake doctor`.
17. Improve startup output.
18. Add a typed configuration file.
19. Formalize backup and restore.
20. Implement pagination and streaming.
21. Run real-cloud certification.

---

## Final direction

RockLake has spent much of its development history proving that an object-store-backed DuckLake catalog can be correct.

The next phase should prove that it can also be **boring to use**.

That means:

- fewer interfaces.
- fewer claims.
- fewer ways to misconfigure the system.
- stronger defaults.
- better diagnostics.
- clearer releases.
- bounded resource behavior.
- evidence instead of projections.

The project should resist pressure to make the roadmap look larger than necessary.

The most valuable improvements from this point are likely to be the ones that make RockLake appear simpler than the machinery underneath it.

> **The post-v0.47.17 roadmap is therefore a roadmap of subtraction, hardening, usability, bounded scale, and evidence, not feature accumulation.**
