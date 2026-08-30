# RockLake Roadmap

**Status:** Active  
**Current baseline:** v0.47.17  
**Roadmap horizon:** v0.48.0–v0.52.x  
**1.0:** Deferred intentionally  
**Primary objective:** Simplification, hardening, operational usability, bounded scale, and real-world validation

---

## 1. Purpose

RockLake has completed its primary correctness-hardening phase through v0.47.17.

The project now has mature foundations for:

- atomic catalog commits;
- writer fencing and conflict handling;
- snapshot-correct reads;
- metadata isolation;
- checkpoint and retention safety;
- catalog export/import fidelity;
- read-only operation;
- path correctness;
- DataFusion scan correctness;
- fault injection;
- backend lifecycle testing;
- DuckLake value-level conformance;
- production failure certification.

The next development phase should not optimize for feature count.

The goal is to turn the existing core into a product that is:

- smaller;
- easier to understand;
- easier to install;
- safer by default;
- easier to operate;
- easier to maintain;
- more transparent about what is supported;
- bounded under large workloads;
- validated against real object-storage environments.

The guiding rule for this roadmap is:

> **Reduce first. Harden second. Simplify operation third. Scale only where measurements justify it.**

---

# 2. Roadmap Principles

## 2.1 Correctness remains non-negotiable

No usability, performance, or compatibility change may weaken the correctness guarantees established through v0.47.17.

Future releases must preserve:

- atomic logical commits;
- retry-safe failure behavior;
- writer fencing;
- rollback integrity;
- snapshot isolation;
- historical-read correctness;
- table and metadata isolation;
- fail-closed reader behavior;
- recovery invariants;
- export/import fidelity.

The v0.47.17 production-failure certification suite remains a permanent regression gate.

---

## 2.2 Prefer removal to expansion

Before adding a new:

- CLI command;
- configuration option;
- network listener;
- crate;
- integration;
- cache;
- background service;
- storage abstraction;
- language binding;

the project should first determine whether an existing surface can be simplified or removed.

A new feature must justify its long-term testing and maintenance cost.

---

## 2.3 Pre-1.0 compatibility is selective

RockLake remains pre-1.0.

This period should be used to clean up interfaces that would become expensive to preserve later.

Compatibility should be maintained when:

- real users depend on the interface;
- the behavior is explicitly documented as supported;
- removal would create significant migration cost;
- the maintenance burden is low.

Historical interfaces should not survive solely because they once existed.

---

## 2.4 One canonical path

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

## 2.5 Safe defaults

Safe behavior should require less configuration than unsafe behavior.

Examples:

- loopback binding by default;
- SCRAM for password authentication;
- explicit opt-in to public network exposure;
- no unauthenticated secondary listeners;
- secrets sourced from environment/files rather than process arguments;
- destructive operations use plan/apply;
- reader mode carries no mutation capability where practical.

---

## 2.6 Documentation is a tested interface

Documentation is part of the product.

A documented command that does not work is a regression.

A documented security capability that is not implemented is a defect.

A documented compatibility claim without executing coverage is not a supported claim.

Primary examples and quickstarts should therefore run in CI.

---

## 2.7 Measurement before optimization

Performance work should proceed in this order:

1. reproduce a workload;
2. measure it;
3. profile it;
4. identify the dominant bottleneck;
5. implement the smallest appropriate change;
6. measure again.

Complex caching or storage tiers should not be built speculatively.

---

# 3. Priority Definitions

Every roadmap item is assigned a priority.

## P0 — Release Blocking

A P0 item must be complete before the release is considered finished.

P0 items generally address:

- correctness;
- security;
- unsafe defaults;
- misleading product behavior;
- release integrity;
- core usability.

---

## P1 — Required

P1 work is expected in the release and should only move when there is a concrete reason.

P1 items generally address:

- maintainability;
- operator experience;
- testing depth;
- observability;
- important cleanup.

---

## P2 — Opportunistic

P2 work is desirable but may be deferred without changing the central purpose of the release.

P2 work should not delay P0/P1 completion.

---

# 4. Release Overview

| Release | Theme | Primary outcome |
|---|---|---|
| **v0.48.0** | Surface Reduction & Product Truthfulness | Smaller, coherent product surface |
| **v0.49.0** | Secure-by-Default Runtime & Release Integrity | Safe runtime and trusted release chain |
| **v0.50.0** | Operational UX & Deployment Simplicity | Excellent first-run and operator experience |
| **v0.51.0** | Bounded Scale, Streaming & Observability | Predictable behavior on large catalogs |
| **v0.52.x** | Real-Cloud Validation & Maintenance | Evidence-driven production confidence |

---

# 5. v0.48.0 — Surface Reduction & Product Truthfulness

## 5.1 Objective

v0.48.0 is a cleanup release.

The objective is to reduce duplicated interfaces, eliminate stale documentation, remove pre-1.0 compatibility baggage, and establish a simpler public surface before additional scalability features are introduced.

The success of v0.48.0 should be measured partly by deletion.

---

## 5.2 P0 — Replace the dual CLI implementation

### Problem

The CLI currently has both:

- typed Clap parsing; and
- legacy/manual argument parsing.

Successfully parsed Clap structures may be converted back into synthetic argument arrays before command execution.

This creates:

- duplicate validation;
- duplicated flag definitions;
- inconsistent error behavior;
- unnecessary code;
- compatibility complexity;
- increased testing burden.

### Requirements

- Remove fallback legacy argument parsing.
- Remove typed-command-to-synthetic-argv conversion.
- Dispatch directly from typed Clap command structures.
- Each command must have a typed configuration object.
- Command handlers must accept typed configuration rather than raw argument arrays.
- Unknown arguments must fail through Clap.
- Supported legacy names may temporarily exist only as explicit aliases.

### Target architecture

```text
argv
  ↓
Clap
  ↓
Typed command
  ↓
Typed handler
  ↓
Catalog/domain API
```

### Acceptance criteria

- [ ] Exactly one top-level CLI parser exists.
- [ ] No production command reconstructs synthetic `argv`.
- [ ] No command handler reparses raw arguments.
- [ ] `rocklake --help` and every subcommand help page are generated by Clap.
- [ ] Invalid arguments have deterministic exit codes.
- [ ] CLI conformance tests cover every public flag.
- [ ] Legacy aliases retained for compatibility are explicitly listed and marked deprecated.

---

## 5.3 P0 — Eliminate snapshot sentinel ambiguity

### Problem

Numeric snapshot IDs must not have hidden meanings such as `0 == latest`.

A snapshot number should always refer to a snapshot.

### Requirements

Introduce an explicit snapshot selector.

Preferred model:

```rust
pub enum SnapshotRef {
    Latest,
    At(SnapshotId),
}
```

Equivalent separate APIs are acceptable where more ergonomic.

### Required scope

Review:

- `rocklake-client`;
- FFI;
- DataFusion interfaces;
- CLI snapshot arguments;
- reader helper APIs;
- bindings.

### Acceptance criteria

- [ ] Public Rust APIs do not use `0` as the recommended latest-snapshot sentinel.
- [ ] Documentation distinguishes `Latest` from an explicit snapshot.
- [ ] Snapshot `0`, when accepted internally, has exactly one documented semantic.
- [ ] Tests verify both latest and explicit historical reads.
- [ ] FFI/bindings expose unambiguous behavior.

---

## 5.4 P0 — Repository documentation truthfulness audit

### Requirements

Audit every primary product claim against current implementation.

At minimum verify:

- current version;
- install instructions;
- binary names;
- Docker image availability;
- container base/runtime assumptions;
- environment variables;
- authentication modes;
- TLS behavior;
- mTLS behavior;
- certificate reload behavior;
- supported object stores;
- supported DuckDB versions;
- supported DuckLake versions;
- DataFusion behavior;
- read-replica behavior;
- backup/restore semantics;
- benchmark environment;
- language bindings.

Every capability must be labeled as one of:

- Stable;
- Supported;
- Experimental;
- Internal;
- Planned.

Unsupported claims must be removed rather than preserved aspirationally.

### Acceptance criteria

- [ ] README describes the current release.
- [ ] README examples use current supported installation paths.
- [ ] No primary documentation describes unimplemented CLI flags.
- [ ] No primary documentation claims mTLS unless mTLS is implemented and tested.
- [ ] No primary documentation claims certificate reload unless it is implemented and tested.
- [ ] Compatibility documentation matches executed tests.
- [ ] Docker documentation matches the actual shipped image.
- [ ] Documentation build runs with strict link/reference validation.

---

## 5.5 P0 — Execute the primary quickstart in CI

### Required scenario

CI must execute an end-to-end user workflow using the public binary.

Minimum sequence:

1. start RockLake;
2. connect DuckDB;
3. load DuckLake;
4. attach catalog;
5. create schema;
6. create table;
7. insert rows;
8. query rows;
9. inspect snapshots;
10. historical read;
11. run a diagnostic operation;
12. terminate RockLake gracefully.

### Acceptance criteria

- [ ] The public local quickstart maps directly to an executable CI script.
- [ ] Commands are copied from or generated from the same canonical source as documentation.
- [ ] CI fails if documentation uses an invalid command.
- [ ] Expected values—not merely process exit codes—are checked.
- [ ] The quickstart is included in release-blocking checks.

---

## 5.6 P1 — Reduce operator CLI surface

Review every top-level command.

Current capabilities should be grouped conceptually into:

### Server

```text
rocklake serve
rocklake doctor
```

### Data protection

```text
rocklake backup ...
rocklake restore ...
rocklake checkpoint ...
```

### Maintenance

```text
rocklake gc ...
rocklake excise ...
rocklake repair ...
```

### Inspection

```text
rocklake inspect ...
rocklake verify ...
```

### Development/internal

```text
rocklake dev corpus ...
rocklake dev compatibility ...
```

Exact naming may differ, but developer-only tooling should not compete with normal operational commands.

### Candidates for consolidation

- `export`;
- `export-catalog`;
- `diagnose`;
- selected `inspect` functionality;
- migration subcommands;
- corpus tooling.

### Acceptance criteria

- [ ] Every top-level command has a documented operator persona/use case.
- [ ] Duplicate export concepts are eliminated.
- [ ] Developer/conformance operations are separated from ordinary operator workflows.
- [ ] Destructive commands use a consistent `plan/apply` model.
- [ ] Machine-consumable commands support structured output where appropriate.

---

## 5.7 P1 — Replace the historical roadmap with a live roadmap

The live roadmap should contain:

- current baseline;
- Now;
- Next;
- Later;
- explicit non-goals;
- release acceptance criteria.

Historical release history belongs in `CHANGELOG.md`.

Old implementation assessments and completed project plans should be removed from normal repository navigation.

### Recommended root

```text
README.md
CHANGELOG.md
ROADMAP.md
CONTRIBUTING.md
SECURITY.md
LICENSE
```

Additional root documents require a durable reason to exist.

### Acceptance criteria

- [ ] `ROADMAP.md` contains only active/future work and brief historical context.
- [ ] Completed one-off implementation reports are removed from the repository root.
- [ ] Superseded planning documents are archived or removed.
- [ ] Documentation navigation does not expose historical implementation archaeology to normal users by default.

---

## 5.8 P1 — Decompose PG-wire implementation by semantic responsibility

Refactoring targets include oversized:

- executor modules;
- handler modules;
- command implementation modules.

Preferred boundaries:

- transactions;
- snapshots;
- schemas;
- tables;
- columns;
- file metadata;
- statistics;
- inlined data;
- views/macros;
- extension schemas;
- compatibility/system queries.

### Constraints

This is a structural refactor.

It must not change catalog semantics.

### Acceptance criteria

- [ ] No new large cross-domain dispatcher is introduced.
- [ ] Core feature families have clear module ownership.
- [ ] Conformance suite remains unchanged or stronger.
- [ ] Public SQL surface manifest remains complete.
- [ ] Semantic modules can be tested independently.

---

## 5.9 P2 — Remove stale compatibility aliases

Identify flags and command forms retained only for historical reasons.

For each alias:

- document real use;
- retain temporarily with warning; or
- remove.

Pre-1.0 aliases should not become permanent without justification.

---

## 5.10 v0.48.0 Release Gate

v0.48.0 is complete when:

- [ ] one CLI parser exists;
- [ ] snapshot selection is explicit;
- [ ] primary documentation matches actual implementation;
- [ ] local quickstart executes in CI;
- [ ] duplicate operator commands are reduced;
- [ ] repository documentation is materially smaller;
- [ ] legacy interfaces without demonstrated need are removed;
- [ ] v0.47.17 correctness certification remains green.

---

# 6. v0.49.0 — Secure-by-Default Runtime & Release Integrity

## 6.1 Objective

Make safe deployment the default and connect source control, CI, certification, tagging, and published artifacts into one enforceable trust chain.

---

## 6.2 P0 — Bind to loopback by default

Default:

```text
127.0.0.1:5432
```

Public exposure requires explicit:

```text
--bind 0.0.0.0:5432
```

### Acceptance criteria

- [ ] default configuration is inaccessible from remote network interfaces;
- [ ] README reflects the safe default;
- [ ] container deployments explicitly configure public/container binding;
- [ ] network tests verify default and explicit behavior.

---

## 6.3 P0 — SCRAM as default password authentication

### Requirements

When username/password authentication is enabled:

- SCRAM-SHA-256 is the preferred/default protocol;
- plaintext password authentication is not the default;
- insecure compatibility modes must be explicit;
- unsafe auth-without-TLS configurations produce strong warnings or fail unless intentionally overridden.

### Acceptance criteria

- [ ] normal authenticated server startup uses SCRAM;
- [ ] SCRAM behavior is network-tested;
- [ ] DuckDB supported client versions connect successfully;
- [ ] compatibility behavior is documented;
- [ ] cleartext password transport cannot occur silently.

---

## 6.4 P0 — Remove or secure the DataFusion secondary listener

Preferred solution:

**remove the second network listener.**

If retained, it must inherit:

- bind policy;
- TLS;
- authentication;
- read/write mode;
- session limits;
- tracing;
- security tests.

### Acceptance criteria

Either:

- [ ] secondary listener has been removed;

or:

- [ ] no configuration can accidentally create an unauthenticated public secondary endpoint;
- [ ] security configuration inheritance is tested;
- [ ] documentation explains why the listener exists.

---

## 6.5 P0 — Remove unconditional raw SQL logging

All direct SQL printing must be removed.

Structured tracing may include:

- operation kind;
- query fingerprint;
- duration;
- affected catalog object;
- trace ID.

Raw SQL must require explicit debug configuration and should support redaction.

### Acceptance criteria

- [ ] normal stdout/stderr contains no raw SQL statement text;
- [ ] secrets/literals are not emitted by default;
- [ ] logs are generated via tracing infrastructure;
- [ ] query correlation remains possible.

---

## 6.6 P0 — Protect the default branch

Repository controls should require:

- pull request workflow;
- release-critical status checks;
- no force pushes;
- restricted direct push;
- controlled automation exceptions.

### Acceptance criteria

- [ ] main branch protection enabled;
- [ ] correctness/release gates configured as required checks;
- [ ] failed certification prevents merge where designated;
- [ ] release automation does not bypass source review except for explicitly controlled actions.

---

## 6.7 P0 — Replace post-tag source mutation

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

### Acceptance criteria

- [ ] release workflow never pushes a version bump to main after a tag event;
- [ ] binary `--version` equals release tag;
- [ ] Cargo workspace version equals release tag;
- [ ] published source tag contains the exact version;
- [ ] release artifacts are built from the tagged SHA.

---

## 6.8 P0 — One release certification workflow

Create a reusable certification workflow.

Full certification should cover:

- fmt;
- clippy;
- workspace tests;
- v0.47.17 production failure suite;
- DuckLake value conformance;
- public surface manifest;
- LocalFS;
- MinIO;
- GCS emulator;
- Azure emulator;
- Windows;
- security tests;
- docs smoke;
- Miri;
- sanitizers where applicable;
- compatibility manifest.

External infrastructure required for certification must either:

- run successfully; or
- block certification.

It must not silently become optional.

### Acceptance criteria

- [ ] a single certification status represents the complete release matrix;
- [ ] releases require certification for the exact SHA being tagged;
- [ ] build-only jobs cannot satisfy execution gates;
- [ ] certification report is generated from job evidence rather than manually asserted.

---

## 6.9 P1 — Secret handling policy

Prefer:

```text
ROCKLAKE_AUTH_PASSWORD
ROCKLAKE_AUTH_PASSWORD_FILE
ROCKLAKE_ENCRYPTION_KEY_FILE
```

Exact names may differ.

Command-line secret arguments should be removed, deprecated, or clearly classified as unsafe development conveniences.

### Acceptance criteria

- [ ] production documentation does not recommend secret CLI arguments;
- [ ] secrets can be supplied from file/environment;
- [ ] startup errors never echo secret values;
- [ ] diagnostic output redacts secrets.

---

## 6.10 P1 — Dependency advisory lifecycle

Every ignored advisory must include:

- advisory ID;
- affected dependency;
- reason for temporary exception;
- upstream tracking reference;
- mitigation;
- review/expiration date.

### Acceptance criteria

- [ ] no advisory ignore exists without justification;
- [ ] dependency upgrades remove resolved exceptions;
- [ ] new advisories fail CI unless explicitly reviewed;
- [ ] security review is part of release preparation.

---

## 6.11 P1 — Supply-chain provenance

Add where practical:

- SBOM;
- GitHub artifact attestations;
- signed release tags;
- signed container images;
- build metadata;
- provenance.

Critical release workflow actions should be pinned more strictly over time.

### Acceptance criteria

- [ ] release artifacts can be traced to source SHA;
- [ ] checksums are published;
- [ ] SBOM is attached to releases or otherwise distributed;
- [ ] provenance/attestation exists for primary release artifacts.

---

## 6.12 P1 — Add `SECURITY.md`

Include:

- supported versions;
- vulnerability reporting path;
- expected response process;
- disclosure policy;
- dependency security policy.

---

## 6.13 v0.49.0 Release Gate

- [ ] loopback is the default bind address;
- [ ] password auth defaults to SCRAM;
- [ ] no accidental unauthenticated secondary listener exists;
- [ ] raw SQL is not logged by default;
- [ ] secrets have production-safe input paths;
- [ ] main is protected;
- [ ] releases build exact tagged/certified source;
- [ ] full certification is release-enforced;
- [ ] dependency exceptions have lifecycle ownership.

---

# 7. v0.50.0 — Operational UX & Deployment Simplicity

## 7.1 Objective

Make RockLake usable without requiring knowledge of SlateDB internals, catalog key structure, writer epochs, object-store implementation details, or historical project architecture.

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

## 7.2 P0 — Introduce `rocklake doctor`

Example:

```bash
rocklake doctor s3://my-bucket/catalog
```

or an equivalent canonical syntax.

### Checks

At minimum:

- URI validity;
- credentials;
- object-store connectivity;
- catalog prefix existence;
- read permission;
- write permission where required;
- list permission where required;
- catalog format;
- migration state;
- snapshot state;
- reader/writer eligibility;
- encryption configuration;
- DuckLake compatibility;
- known unsafe runtime configuration;
- basic storage latency.

### Output

Human-readable by default.

Machine-readable:

```text
--output json
```

### Exit behavior

- `0`: ready;
- non-zero: actionable failure.

### Acceptance criteria

- [ ] fresh local catalog passes appropriate preflight;
- [ ] valid cloud catalog passes without mutation unless explicitly requested;
- [ ] permission failures identify the missing capability;
- [ ] format/migration incompatibility is clearly reported;
- [ ] JSON schema is stable for the release series.

---

## 7.3 P0 — Improve server startup UX

Startup output should communicate:

- version;
- catalog URI;
- serving mode;
- supported DuckLake version;
- listen address;
- TLS state;
- auth state;
- metrics state;
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

### Acceptance criteria

- [ ] successful startup produces concise actionable output;
- [ ] unsafe configurations produce visible warnings;
- [ ] machine logs remain available through tracing;
- [ ] startup messages do not expose secrets.

---

## 7.4 P0 — Zero-friction local development

Preferred command:

```bash
rocklake serve ./lake
```

or equivalent.

It should:

- create the local catalog if appropriate;
- bind safely;
- use development-appropriate defaults;
- clearly state security status;
- output DuckDB connection instructions.

### Acceptance criteria

- [ ] new user can create and query a local catalog without cloud credentials;
- [ ] no additional required configuration for the basic case;
- [ ] the path is tested on Linux, macOS, and Windows where supported.

---

## 7.5 P1 — Typed configuration file

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

### Requirements

- strict validation;
- unknown keys rejected or explicitly warned;
- secrets may reference files/environment;
- generated example configuration;
- effective configuration inspection with redaction.

Possible command:

```bash
rocklake config check
```

### Acceptance criteria

- [ ] equivalent CLI/config values produce identical behavior;
- [ ] invalid configuration fails before catalog mutation;
- [ ] effective configuration can be inspected safely;
- [ ] configuration schema is documented from source where practical.

---

## 7.6 P0 — Make backup and restore first-class concepts

Low-level export/import may remain internally, but operators should see:

```text
rocklake backup create
rocklake backup inspect

rocklake restore plan
rocklake restore apply
```

### Backup requirements

- snapshot-consistent;
- versioned metadata;
- integrity metadata;
- source catalog identity;
- creation time;
- snapshot identifier;
- checksum where practical.

### Restore requirements

- validate before mutation;
- plan before apply;
- atomic publication;
- reconstruct counters/indexes;
- verify post-restore invariants;
- refuse unsafe overwrite without explicit action.

### Acceptance criteria

- [ ] backup → new catalog restore → next write is tested end-to-end;
- [ ] interrupted restore cannot expose partial catalog state;
- [ ] restore plan reports exactly what will change;
- [ ] successful restore automatically runs verification;
- [ ] docs distinguish backup, checkpoint, export, and migration.

---

## 7.7 P1 — Standardize operational output

Operator commands should support:

```text
--output human
--output json
```

where useful.

Potential commands:

- doctor;
- inspect;
- verify;
- backup;
- restore;
- gc plan;
- excise plan;
- repair plan.

JSON output should avoid human-format scraping.

---

## 7.8 P1 — Uniform plan/apply semantics

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

### Acceptance criteria

- [ ] plan mode makes no persistent changes;
- [ ] apply validates that assumptions have not materially changed;
- [ ] JSON plan can be archived;
- [ ] destructive actions are explicit.

---

## 7.9 P0 — Make Docker either real or absent

If RockLake claims an official container image, it must be a first-class release artifact.

### Required container support

- GHCR publication;
- version tag;
- immutable digest;
- tested startup;
- non-root execution;
- current CA bundle;
- correct environment handling;
- health check;
- multi-architecture images where practical;
- SBOM;
- signing/attestation.

The container should rely on RockLake's own environment parsing rather than shell-variable expansion inside JSON-form `CMD`.

If this standard is not met, official-container claims should be removed until it is.

### Acceptance criteria

- [ ] documented `docker run` command works verbatim;
- [ ] image tag matches binary version;
- [ ] image is created by release workflow;
- [ ] image startup is tested before publication;
- [ ] health check exercises real RockLake readiness;
- [ ] container docs are version-current.

---

## 7.10 P2 — Installation ergonomics

Consider after the core distribution story is stable:

- shell installer;
- Homebrew;
- cargo install documentation;
- package-manager integrations.

Do not add package channels that cannot be continuously maintained.

---

## 7.11 v0.50.0 Release Gate

A first-time user must be able to:

1. install RockLake;
2. run `doctor`;
3. start a local or cloud catalog;
4. copy the provided DuckDB attach command;
5. create/query data;
6. diagnose a bad configuration;
7. create a backup;
8. understand restore procedure;

without reading architecture/internal documentation.

---

# 8. v0.51.0 — Bounded Scale, Streaming & Observability

## 8.1 Objective

Ensure large catalogs do not cause unbounded memory use or unnecessarily delay first results.

Optimize behavior before introducing speculative caching.

---

## 8.2 P0 — Paginated data-file listing

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

### Continuation token requirements

Tokens should be:

- opaque;
- validated;
- snapshot-aware;
- independent of public knowledge of internal key encoding;
- rejected if incompatible with request context.

### Acceptance criteria

- [ ] 100k+ file listing can be traversed without one `Vec` containing every row;
- [ ] page traversal returns exactly-once logical coverage for a stable snapshot;
- [ ] invalid tokens fail cleanly;
- [ ] historical snapshot pagination is correct;
- [ ] page-size limit is enforced.

---

## 8.3 P0 — Async streaming API

Provide streaming for high-cardinality operations.

Conceptual:

```rust
stream_data_files(...)
```

### Requirements

- bounded channel/buffer;
- cancellation safety;
- backpressure;
- error propagation;
- snapshot consistency.

### Acceptance criteria

- [ ] consumer may process files incrementally;
- [ ] producer does not unboundedly outrun consumer;
- [ ] cancellation releases resources;
- [ ] mid-stream storage failure propagates as error rather than truncated success.

---

## 8.4 P0 — PG-wire incremental result delivery

Large metadata responses should stream where possible.

### Metrics

Measure:

- time to first row;
- total response time;
- rows/sec;
- bytes/sec;
- peak buffered rows;
- peak RSS.

### Acceptance criteria

- [ ] large scans do not require full response materialization;
- [ ] slow clients apply backpressure;
- [ ] disconnected clients cancel remaining work;
- [ ] resource use remains bounded by documented limits.

---

## 8.5 P0 — Explicit resource limits

Introduce or consolidate limits for:

- active sessions;
- active scans;
- stream queue depth;
- maximum page size;
- buffered rows;
- relevant response memory;
- operational concurrency.

### Acceptance criteria

- [ ] every potentially unbounded user-controlled collection has a limit or streaming behavior;
- [ ] limits are observable;
- [ ] exhaustion produces explicit errors;
- [ ] defaults are safe for modest deployments.

---

## 8.6 P1 — Observability redesign

Prefer a small useful metric set over many low-value counters.

### Core metrics

#### Request

- request duration histogram;
- SQL classification latency;
- response rows;
- time to first row.

#### Catalog

- snapshot read latency;
- commit latency;
- conflicts;
- current snapshot;
- reader refresh lag.

#### Object store

- operations by type;
- bytes read;
- bytes written;
- latency;
- retries;
- errors.

#### Process

- RSS;
- active sessions;
- active scans;
- queue depth;
- stream backpressure;
- task/bridge pressure where relevant.

### Acceptance criteria

- [ ] standard dashboard can identify CPU, memory, storage latency, and queue bottlenecks;
- [ ] histograms are true histogram instruments;
- [ ] metric names/labels have documented cardinality constraints;
- [ ] no metric embeds uncontrolled table/query values in labels.

---

## 8.7 P1 — End-to-end trace correlation

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

### Acceptance criteria

- [ ] one trace ID can correlate a slow user request with catalog/storage work;
- [ ] errors record the relevant trace ID;
- [ ] tracing does not include sensitive SQL values by default;
- [ ] tracing overhead is measured.

---

## 8.8 P1 — Slow-operation reporting

Provide configurable logging for operations exceeding thresholds.

Examples:

- slow PG query;
- slow snapshot open;
- slow file scan;
- slow object-store request;
- slow commit.

Use operation identifiers/fingerprints rather than raw sensitive payloads.

---

## 8.9 P1 — Large-catalog benchmark suite

Minimum scenarios:

- 10k files;
- 100k files;
- 1M files where practical;
- wide schemas;
- many tables;
- historical snapshots;
- paginated reads;
- streaming reads;
- concurrent readers.

Measure:

- p50/p95/p99/p999;
- time to first row;
- peak RSS;
- object-store operations;
- bytes transferred.

---

## 8.10 v0.51.0 Release Gate

- [ ] high-cardinality listings support pagination;
- [ ] large reads support bounded streaming;
- [ ] PG-wire large responses are bounded;
- [ ] cancellation/backpressure is tested;
- [ ] memory/resource limits are explicit;
- [ ] operational metrics identify primary bottlenecks;
- [ ] large-scale benchmark includes memory and first-row measurements.

---

# 9. v0.52.x — Real-Cloud Validation & Maintenance

## 9.1 Objective

Validate RockLake under realistic cloud conditions and use those results to determine the next architectural priorities.

v0.52 should be a release series rather than one feature bundle.

Example:

```text
v0.52.0 — AWS baseline
v0.52.1 — GCS baseline
v0.52.2 — multi-node soak
v0.52.3 — dependency/storage upgrade
...
```

Exact sequencing should follow engineering needs.

---

## 9.2 P0 — Real AWS S3 benchmark

Use current production dependencies and a documented environment.

### Minimum topology

- 1 writer;
- 1 reader;
- 4 readers;
- 16 readers.

### Workloads

- catalog open;
- latest snapshot refresh;
- create tables;
- add files;
- list files;
- historical reads;
- backup;
- verification;
- writer replacement.

### Scale points

At minimum:

- small catalog;
- 10k files;
- 100k files;
- larger scale where cost permits.

### Report

Record:

- region;
- availability zone topology;
- EC2 instance;
- S3 class;
- RockLake SHA/version;
- SlateDB version;
- request counts;
- bytes;
- p50/p95/p99/p999;
- RSS;
- cold-start latency;
- estimated cost.

### Acceptance criteria

- [ ] raw benchmark procedure is committed;
- [ ] results are reproducible;
- [ ] no projected/local values are labeled AWS measurements;
- [ ] reader/writer correctness invariants are checked during load.

---

## 9.3 P0 — Real GCS benchmark

Run comparable scenarios on GCS.

The purpose is not to force identical latency between clouds.

The purpose is to verify:

- correctness;
- lifecycle behavior;
- error behavior;
- operational viability;
- cost/performance characteristics.

---

## 9.4 P0 — Multi-node soak

Run sustained workloads against real object storage.

Target duration:

**24 hours** for formal soak certification where feasible.

### Workload

Continuously perform:

- commits;
- reads;
- historical reads;
- reader refresh;
- writer restart;
- reader restart;
- checkpoint creation;
- verification;
- backup;
- GC where safe.

Inject:

- process kill;
- network delay;
- object-store throttling;
- transient errors;
- writer replacement;
- reader rolling restart;
- credential refresh/expiration scenarios where feasible.

### Acceptance criteria

- [ ] no invariant violations;
- [ ] no silent wrong results;
- [ ] no unbounded RSS trend;
- [ ] all committed snapshots remain readable within retention policy;
- [ ] reader convergence remains bounded;
- [ ] writer takeover behaves correctly;
- [ ] expected transient failures recover.

---

## 9.5 P1 — Benchmark execution, not JSON validation

Benchmark files may remain as historical artifacts.

However, release/performance gates should execute benchmark code for important baselines.

### Required benchmark metadata

Every published result must contain:

- version;
- commit SHA;
- date;
- Rust version;
- SlateDB version;
- object_store version;
- machine;
- backend;
- dataset;
- workload;
- repetitions;
- raw results;
- summary.

### Acceptance criteria

- [ ] CI or designated benchmark infrastructure runs actual benchmark workloads;
- [ ] committed JSON alone cannot satisfy a performance certification gate;
- [ ] results identify projections explicitly;
- [ ] stale baselines are retired.

---

## 9.6 P0 — Dependency modernization

Use v0.52.x to review core dependencies.

Priority:

- SlateDB;
- object_store;
- DataFusion;
- pgwire;
- sqlparser;
- Rust MSRV;
- crypto/TLS dependencies.

Goals:

- remove ignored advisories;
- reduce duplicated dependency versions;
- retire compatibility shims;
- validate performance after upgrades.

Each significant storage upgrade must rerun:

- production failure certification;
- backend matrix;
- import/export;
- read-only behavior;
- soak-critical tests.

---

## 9.7 P1 — Maintenance budget

Reserve explicit release capacity for:

- flaky test removal;
- CI runtime reduction;
- dead-code removal;
- unused feature removal;
- documentation pruning;
- obsolete tests;
- dependency cleanup;
- tracing cleanup;
- API deprecations;
- benchmark maintenance.

A mature project requires scheduled subtraction.

---

## 9.8 P1 — Production-shaped upgrade testing

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

## 9.9 v0.52.x Exit Criteria

The v0.52 series is considered complete when:

- [ ] real AWS measurements exist;
- [ ] real GCS measurements exist;
- [ ] sustained multi-node soak has completed;
- [ ] no known severe correctness findings remain;
- [ ] core dependency advisories are substantially reduced;
- [ ] benchmark infrastructure executes rather than validates static reports;
- [ ] major performance bottlenecks have been identified using real evidence;
- [ ] the next optimization roadmap is based on observed profiles.

---

# 10. Deferred Work

The following work is intentionally outside the immediate roadmap.

---

## 10.1 Tiered NVMe cache

Status:

**Deferred pending real-cloud profiling.**

Do not build a tiered L1/L2/L3 cache merely because remote storage is assumed to be slow.

Real-cloud measurements may identify different bottlenecks:

- metadata scan amplification;
- manifest reads;
- insufficient indexes;
- serialization;
- object-store request count;
- page sizing;
- SlateDB configuration;
- concurrency;
- CPU.

An NVMe cache should be reconsidered only if profiling demonstrates substantial benefit.

---

## 10.2 Native DuckDB extension

Continue tracking upstream feasibility.

Do not make this a primary roadmap item unless:

- the relevant DuckDB extension APIs stabilize;
- integration materially improves the user experience;
- there is concrete user demand.

The PG-wire path should remain excellent independently.

---

## 10.3 New language bindings

Do not add bindings primarily for completeness.

A binding should require:

- real consumer;
- maintained CI;
- package distribution owner;
- compatibility policy.

Existing bindings should be reviewed under the same standard.

Unsupported bindings should be clearly experimental.

---

## 10.4 New engines and clients

Do not add "supported" integrations without executable ongoing coverage.

A compatibility claim requires:

- version range;
- real execution;
- CI ownership;
- failure semantics;
- documentation.

---

## 10.5 General-purpose fact store

The underlying architecture may eventually support broader use cases.

That direction should not distract from making the DuckLake catalog product small and excellent.

No generalized fact-store API belongs in this roadmap.

---

## 10.6 1.0

1.0 is intentionally deferred.

There is no requirement to promote RockLake to 1.0 at the end of this roadmap.

The project should remain pre-1.0 until maintainers decide the interface, operations model, and compatibility commitments are sufficiently stable.

No artificial deadline should drive that decision.

---

# 11. Support Levels

Starting in v0.48, public capabilities should be classified explicitly.

## Stable

Meaning:

- release-gated;
- continuously tested;
- production-intended;
- compatibility changes considered carefully.

---

## Supported

Meaning:

- regularly tested;
- documented;
- expected to work;
- may evolve before 1.0.

---

## Experimental

Meaning:

- clearly labeled;
- may change without compatibility guarantees;
- may be removed;
- not part of production certification.

---

## Internal

Meaning:

- implementation detail;
- no public compatibility guarantee;
- should not appear in user-facing guidance.

---

# 12. Definition of Supported

A capability may only be documented as **Supported** or **Stable** if all applicable conditions are met:

- [ ] implementation exists;
- [ ] primary path is tested;
- [ ] failure behavior is tested;
- [ ] documentation matches implementation;
- [ ] supported versions are identified;
- [ ] CI runs continuously or on the release certification path;
- [ ] an owner exists for maintenance.

A feature existing in source code does not automatically make it supported.

---

# 13. Release Certification Policy

Every formal release must evaluate the following categories.

## 13.1 Correctness

- [ ] production failpoint suite passes;
- [ ] full invariant verification passes;
- [ ] overlapping writers pass;
- [ ] rollback passes;
- [ ] historical reads pass;
- [ ] backup/restore tests pass where applicable.

---

## 13.2 DuckLake compatibility

- [ ] supported DuckDB version tested;
- [ ] supported DuckLake version tested;
- [ ] exact value-level checks pass;
- [ ] public metadata surface manifest passes;
- [ ] unsupported SQL fails explicitly.

---

## 13.3 Backends

For each backend marked supported:

- [ ] lifecycle test executes;
- [ ] nested prefix behavior tested;
- [ ] read/write permissions tested;
- [ ] transient failure paths covered where practical.

Emulator testing certifies deterministic behavior.

Real-cloud testing certifies real-cloud behavior.

The two must not be conflated.

---

## 13.4 Security

- [ ] dependency policy passes;
- [ ] auth tests pass;
- [ ] TLS tests pass;
- [ ] default network behavior verified;
- [ ] no secret leakage regression;
- [ ] no raw-SQL logging regression;
- [ ] security advisory exceptions reviewed.

---

## 13.5 Documentation

- [ ] strict documentation build;
- [ ] quickstart executes;
- [ ] CLI examples execute;
- [ ] version references are current;
- [ ] supported-feature claims match certification.

---

## 13.6 Platform

For every published binary:

- [ ] builds successfully;
- [ ] launches;
- [ ] reports correct version;
- [ ] performs a minimal catalog operation where practical.

---

## 13.7 Release integrity

- [ ] source version committed before tag;
- [ ] certification completed on exact SHA;
- [ ] tag points at certified SHA;
- [ ] artifacts built from tag;
- [ ] checksums generated;
- [ ] provenance generated where supported.

---

# 14. Documentation Strategy

The documentation site should optimize for users before contributors.

Recommended navigation:

```text
Getting Started
Deployment
Operations
Integrations
Reference
Internals
Contributing
```

Avoid exposing every historical design decision in the primary navigation.

---

## 14.1 Getting Started

Should answer:

- What is RockLake?
- Should I use it?
- How do I install it?
- How do I start it?
- How do I connect DuckDB?
- What do I do next?

---

## 14.2 Deployment

Only document deployments that are actually supported.

Examples:

- binary;
- Docker;
- AWS;
- GCS;
- Azure;
- Kubernetes, only if maintained.

---

## 14.3 Operations

Focus on tasks:

- diagnostics;
- backup;
- restore;
- upgrades;
- verification;
- garbage collection;
- excision;
- monitoring;
- security;
- troubleshooting.

---

## 14.4 Internals

Deep material belongs here:

- MVCC;
- key layout;
- fencing;
- transactions;
- SlateDB;
- failure certification;
- SQL dispatcher internals.

Internals should not be prerequisites for first-time use.

---

# 15. Product UX Targets

## 15.1 Five-minute local target

A user with RockLake and DuckDB installed should be able to reach a useful query using approximately:

```bash
rocklake serve ./lake
```

then:

```sql
ATTACH 'host=127.0.0.1 port=5432' AS lake (TYPE ducklake);
```

and proceed normally.

---

## 15.2 Cloud preflight target

A user should be able to diagnose a cloud deployment before running a server:

```bash
rocklake doctor s3://bucket/catalog
```

The output should identify:

- missing credentials;
- missing permissions;
- incompatible catalog;
- unsupported configuration;
- unsafe runtime choices.

---

## 15.3 Operator automation target

Operational tooling should have stable structured output.

Users should not need to scrape decorative console text from:

- doctor;
- verify;
- backup;
- restore;
- inspect;
- maintenance plans.

---

# 16. Architecture Direction

## 16.1 Read-only capability should become structural

Long term, prefer an API where read-only processes cannot obtain mutation methods.

Possible designs:

```rust
Catalog<ReadOnly>
Catalog<ReadWrite>
```

or:

```text
ReaderCatalog
WriterCatalog
```

The SQL access-mode guard remains valuable defense in depth, but the underlying capability model should eventually make illegal states difficult to represent.

This work may land incrementally after v0.48 and should not block higher-priority cleanup unless a concrete vulnerability requires it.

---

## 16.2 Keep bounded SQL bounded

RockLake should continue supporting the finite SQL vocabulary necessary for its clients.

It should not gradually become a general PostgreSQL implementation.

For each newly supported SQL shape:

- identify the client that emits it;
- add corpus evidence;
- add semantic tests;
- reject unsupported variants explicitly.

---

## 16.3 Avoid invisible fallbacks

Prefer:

```text
error
```

over:

```text
best-effort empty result
```

for:

- malformed metadata;
- unsupported formats;
- object-store failures;
- snapshot errors;
- unknown SQL mutations;
- corrupted backup state.

Silent success is more dangerous than visible incompatibility.

---

# 17. Performance Policy

Performance claims must identify whether they are:

- measured;
- modeled;
- projected.

Projected numbers must never be presented as production validation.

---

## 17.1 Required performance dimensions

Future benchmark reports should cover more than latency.

At minimum where applicable:

- p50;
- p95;
- p99;
- p999;
- time to first row;
- throughput;
- RSS;
- CPU;
- object-store operations;
- bytes read;
- bytes written;
- cold start;
- estimated cost.

---

## 17.2 Regression policy

Performance regression gates should:

- execute current code;
- use documented datasets;
- tolerate expected environment noise;
- compare against a meaningful recent baseline;
- avoid treating stale committed numbers as proof.

---

# 18. Dependency Policy

Core dependencies form part of RockLake's effective architecture.

Priority dependencies include:

- SlateDB;
- object_store;
- DataFusion;
- pgwire;
- sqlparser;
- Tokio;
- rustls;
- cryptographic dependencies.

For each:

- track supported version;
- monitor advisories;
- periodically test upgrade paths;
- avoid remaining indefinitely on vulnerable/transitionally unsupported versions.

---

# 19. Work That Should Trigger Deletion Review

Before adding any of the following, require a surface-area review:

- new top-level CLI command;
- new crate;
- new network port;
- new external dependency;
- new environment variable;
- new object-store abstraction;
- new binding;
- new compatibility target.

The review must ask:

1. Can an existing interface solve this?
2. Can something be removed first?
3. Who will test it?
4. Who will maintain it?
5. What happens if it becomes stale?

---

# 20. Long-Term Success Criteria

This roadmap succeeds if RockLake emerges from v0.52.x with the following properties.

## Product

- one obvious setup path;
- concise documentation;
- accurate compatibility claims;
- low configuration burden.

## Correctness

- failure certification remains continuously green;
- no known silent wrong-result paths;
- backups and restores are routinely verified.

## Security

- safe bind defaults;
- secure password authentication;
- no accidental public unauthenticated endpoints;
- governed dependency vulnerabilities;
- traceable release artifacts.

## Operations

- useful preflight diagnostics;
- stable JSON output;
- explicit backup/restore;
- understandable metrics;
- bounded shutdown and failure behavior.

## Scale

- pagination;
- streaming;
- backpressure;
- bounded memory;
- real-cloud performance evidence.

## Maintenance

- smaller CLI implementation;
- smaller active documentation set;
- fewer stale aliases;
- manageable executor structure;
- regular dependency modernization;
- explicit technical-debt budget.

---

# 21. Immediate Work Queue

The first work after adopting this roadmap should be performed in this order.

## P0 Immediate

1. Replace legacy CLI dispatch.
2. Fix snapshot sentinel APIs.
3. Perform README/docs truthfulness sweep.
4. Add executable quickstart CI.
5. Remove or correct unsupported Docker claims.
6. Remove unsupported TLS/mTLS claims.
7. Remove raw SQL printing.
8. Decide fate of secondary DataFusion listener.

## P1 Immediate

9. Shrink/archive historical planning documentation.
10. Consolidate duplicate CLI commands.
11. Protect `main`.
12. Redesign release version/tag sequencing.
13. Build reusable full certification workflow.
14. Establish `SECURITY.md`.
15. Review ignored dependency advisories.

## Then

16. Build `rocklake doctor`.
17. Improve startup UX.
18. Introduce safe configuration file.
19. Formalize backup/restore UX.
20. Implement pagination and streaming.
21. Run real-cloud certification.

---

# 22. Final Direction

RockLake has spent much of its development history proving that an object-store-backed DuckLake catalog can be correct.

The next phase should prove that it can also be **boring to use**.

That means:

- fewer interfaces;
- fewer claims;
- fewer ways to misconfigure the system;
- stronger defaults;
- better diagnostics;
- clearer releases;
- bounded resource behavior;
- evidence instead of projections.

The project should resist pressure to make the roadmap look larger than necessary.

The most valuable improvements from this point are likely to be the ones that make RockLake appear simpler than the machinery underneath it.

> **The post-v0.47.17 roadmap is therefore a roadmap of subtraction, hardening, usability, bounded scale, and evidence—not feature accumulation.**