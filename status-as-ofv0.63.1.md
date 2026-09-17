# RockLake status as of v0.63.1 and the plan for v0.64

Assessment date: 2026-09-17. Release baseline: `v0.63.1`, commit `2ff27071a048d5f66506ea8336a1c3464184b9fa`. Source reviewed: `ff6b0bdb49abf102f6b244070e134540a22f9964`, which includes subsequent maintenance on `main`.

Make v0.64 a correctness, resource-control, and usability release. The most valuable work is to make independent readers safe, make backups trustworthy, enforce router limits before consuming resources, and replace nominal performance checks with measurements of the candidate code.

v1.0 is postponed indefinitely. Live AWS S3, GCS, and Azure testing is also postponed, with no replacement deadline. Neither is a prerequisite for this plan. All proposed validation uses local files, in-memory fault injection, local processes, or optional containers. Emulator results must retain their emulator label.

## What the review establishes

The project has useful foundations: MVCC rows and secondary indexes, explicit writer epochs, snapshot-bound catalog reads, paged and streaming metadata APIs, a versioned export format, fault-injection infrastructure, protocol conformance tests, and a multi-catalog router. Reuse these. A new storage engine, query engine, orchestration service, or cache framework would add work before addressing the current defects.

The completion labels in the roadmap are ahead of the evidence in several areas:

| Area | Current implementation and evidence | Consequence for v0.64 |
|---|---|---|
| Independent readers | Both read-only opening paths use ordinary SlateDB `Db` handles. Opening a reader fenced a live writer in a local reproduction. | Fix before claiming reader-process scale-out. |
| Backup and recovery | Export uses a storage snapshot, but backup inventory, restore-point validation, and checkpoint retention have gaps. | Test actual restore outcomes and referenced objects. |
| Router isolation | First-open coordination, capacity admission, descriptor reload, and quota reload have concrete gaps. | Bound and test the lifecycle of catalog handles. |
| Performance evidence | The v0.52 directory contains a schema and README, but no raw result reports. v0.61 adds profile constants. | Treat the advertised envelopes as unverified in this checkout. |
| Performance CI | Jobs compare committed results with committed thresholds. The checker accepts missing measurements. | A passing job currently does not demonstrate candidate performance. |
| Administrative jobs | Durable records exist. Scheduling queues records, while resume requeues them. The CLI execution wrapper does not consume checkpoints or poll cancellation. | Separate recorded job state from executable recovery behavior. |
| Bindings | CI execution has improved after the tag, but fixtures are mostly empty and native-handle lifecycle problems remain. | Fix lifecycle safety, then add populated interoperability checks. |

These findings come from source and test review, not a production incident investigation. Source-confirmed defects below have direct implementation evidence. Items marked for reproduction or measurement need their proposed check before choosing a larger implementation.

### Work already on main

Do not schedule these fixes again. Include them in the next release notes and rerun their checks on the eventual candidate:

- Catalog and SQL executor simplification, removal of unused code, and Rust client ownership simplification in `da11f17` and `28ae747`.
- SQL client test-server readiness repair in `a42e7fa`.
- Coverage failure enforcement and removal of swallowed Python and Node test failures in `9bbce9d`.
- Rust binding compilation repairs, Python and Node close-error propagation, Node test discovery, and use of the local NAPI CLI in `807f51e`, `1ad7740`, and `802a385`.
- Emulator and sanitizer workflow repairs in `ff6b0bd`.

The current [changelog](CHANGELOG.md) has an empty Unreleased section despite these changes. Recording them is part of milestone M0, not evidence that v0.64 has already shipped.

## Milestones and scope

The milestones are ordered by dependencies, without calendar promises. Size describes a work packet: small is a focused change, medium crosses a few components, and large changes a shared runtime contract. It is not a time estimate.

| Milestone | Required tasks | Main owners in the code | Dependency | Exit condition |
|---|---|---|---|---|
| M0: Establish an honest baseline | T01–T02 | Documentation, evidence scripts, CI | None | Release direction is current and invalid performance reports fail. |
| M1: Make reads and recovery safe | T03–T07 | Catalog, shared read APIs, CLI | None | Independent readers preserve writer availability; backup and retention regressions pass. |
| M2: Make concurrency predictable | T08–T12 | Router, PG-wire lifecycle, FFI, Go | Reader-backed routes depend on M1 | Opens, reloads, cancellation, parsing, and native handle use obey their contracts. |
| M3: Remove measured scaling obstacles | T13–T15 | Cleanup, verification, PG-wire, CLI jobs | M1–M2 | Cleanup avoids repeated full scans; buffer and job behavior are demonstrable. |
| M4: Make operator output actionable | T16 | CLI, cache and capacity reporting | M0 and safe inspection from M1 | Estimates are distinguishable from observations and recommendations name real controls. |
| M5: Certify the local release | T17–T18 | Evidence runner, testkit, release workflow | M0–M4 | Candidate-specific local evidence, upgrade checks, and artifact quickstart pass. |

T01–T18 are the proposed v0.64 scope. T14 and T15 allow a smaller documented contract where a full implementation is not justified. T19–T23 are optional follow-on work, ordered after the correctness fixes. Do not exchange a required safety fix for an optional feature.

If the reader migration is larger than expected, cut a maintenance release with completed fixes and retain the unresolved limitation. Do not advance the reader support claim merely to meet a version number.

## M0: Establish an honest baseline

### T01: Replace the obsolete release direction and reconcile support claims

Size: small. Impact: quality and operator expectations.

[ROADMAP.md](ROADMAP.md) assigns v0.80.0 to AWS evidence and keeps v1.0 release candidates postponed indefinitely. [Beta support](docs/operations/beta-support.md) prohibits new features until that process finishes. It also says LocalFS and MinIO evidence is complete, while [COMPATIBILITY.md](COMPATIBILITY.md#object-store-evidence) still marks their scale certification pending. The [v0.52 evidence directory](benchmarks/evidence/v0.52.0/) contains no raw runs, despite its README saying it does.

- [ ] Make this v0.64 sequence the active roadmap. Mark v1.0, field observation, independent production review, and real-cloud certification as unscheduled future work.
- [ ] Replace the blanket beta feature freeze with the bounded v0.64 scope. Preserve the existing compatibility and deprecation rules for changes to public commands and schemas.
- [ ] Reconcile README, compatibility, beta support, readiness assessment, and capacity documentation. Distinguish implemented functionality, executed local tests, emulator tests, and measured scale.
- [ ] Locate the raw reports behind existing capacity and cloud-performance claims. If they cannot be supplied, label the numbers as historical estimates or withdraw the claims. Do not manufacture replacement evidence.
- [ ] Populate Unreleased with the post-tag changes listed above.

Acceptance: the documents agree that cloud testing and v1.0 are unscheduled. Every retained performance claim points to a reproducible result with source identity and environment. `mkdocs build --strict` and compatibility-manifest validation pass.

### T02: Make the performance check reject absent or invalid evidence

Size: medium. Impact: quality and performance regression detection.

The [checker](scripts/check_benchmark_regression.py#L20) returns success without thresholds and skips missing metrics. It looks for the suffixed name before resolving minimum thresholds, so a `throughput_min` threshold skips a result stored as `throughput`. All three cases were reproduced. [CI](.github/workflows/ci.yml#L395) and [release certification](.github/workflows/release-certification.yml#L50) validate stored values rather than fresh candidate measurements.

- [ ] Give the checker separate candidate-result and baseline inputs. Reject missing required metrics, empty reports, nonfinite values, incompatible units, and incompatible measurement identities.
- [ ] Resolve minimum-threshold names before looking up results. Test one latency maximum and one throughput minimum.
- [ ] Before using `rocklake-evidence` in this gate, bring forward T17's minimum runner repairs: candidate identity, correctness-failure handling, and accurate accounting for every metric the gate compares. Leave the larger workloads in M5.
- [ ] Run a small current-code benchmark in CI. Compare fresh output with a reviewed baseline and archive the output. Keep full scale runs manual.
- [ ] Make baseline updates explicit and reviewable. A normal check must never rewrite its own thresholds.

Acceptance: a deliberately slowed candidate fails, as do empty results and missing metrics. A valid candidate passes. Add one small Python test file using the standard library, including the three reproduced failures.

## M1: Make reads and recovery safe

### T03: Open independent readers through SlateDB's reader API

Size: large. Impact: correctness, availability, and reader scalability. Highest priority.

[ReadOnlyCatalog::open](crates/rocklake-catalog/src/readonly.rs#L71) and [CatalogStore::open_without_epoch](crates/rocklake-catalog/src/store.rs#L332) both open a normal SlateDB `Db`. Skipping RockLake's epoch does not skip SlateDB's writer fencing. The [existing tests](crates/rocklake-catalog/tests/v047_readonly_tests.rs#L26) acknowledge this and share one underlying handle instead of opening independent readers.

The local reproduction committed from a writer, opened `ReadOnlyCatalog` independently, and attempted another write. The second commit returned `TransactionConflict("Closed error: detected newer DB client")`.

- [ ] Use the existing SlateDB 0.13 reader facilities, including `DbReader` and its read operations, for independent readers. Adapt shared catalog reads once, preserving encryption, snapshot selection, retention checks, and close behavior.
- [ ] Route the async client, sync client, FFI, read-only server, and router through the corrected path. Define refresh semantics for each public read-only API.
- [ ] Audit direct `Db::open` calls in [main.rs](crates/rocklake-pgwire/src/main.rs). Read-only commands such as capacity reporting and inspection must not seize storage-writer ownership. Commands that write job state require explicit ownership handling.
- [ ] Replace the misleading independent-reader tests with separately opened handles, then separate processes. Keep shared-handle tests under accurate names.

Acceptance: one writer continues committing while 1, 4, and 16 independent readers open, refresh, close, and reopen on the same local catalog. An exact snapshot remains stable while refresh observes newer durable commits. Repeat with encryption. Inspection commands do not fence the server. Document any native reader checkpoint writes and freshness lag instead of promising zero object-store writes.

### T04: Make backup inventory complete without imposing its cap on every backup

Size: medium. Impact: recovery correctness and catalog scale.

[collect_object_references](crates/rocklake-catalog/src/backup.rs#L241) scans only data files, ignores delete files, joins paths without honoring `path_is_relative`, and treats every HEAD error as a missing object. [encryption_key_ids](crates/rocklake-catalog/src/backup.rs#L310) builds the same capped inventory even when inventory was not requested. Consequently, a default backup fails above 100,000 visible data files.

- [ ] Include visible data files and delete files, including their encryption-key references. Reuse [resolve_object_path](crates/rocklake-core/src/path.rs) for canonical paths.
- [ ] Compare HEAD sizes with catalog sizes. Preserve missing, permission, and transient-error distinctions.
- [ ] Collect unique encryption IDs with a streaming scan or reuse an inventory already collected. Remove the inventory cap from the default metadata-backup path.
- [ ] Keep an explicit bound on optional inline inventories. Document it and reject before producing a misleading complete artifact. Add a streamed sidecar only if that inventory is required at larger sizes.

Acceptance: backup of 100,001 references succeeds with inventory disabled. Missing delete files, wrong sizes, rooted paths, and injected permission failures produce the expected result. A requested oversized inline inventory fails clearly. Test both encrypted and unencrypted references.

### T05: Verify the backup's claimed restore point and publish complete artifacts

Size: medium. Impact: recovery integrity.

[inspect_backup](crates/rocklake-catalog/src/backup.rs#L324) checks the NDJSON hash, byte count, and row count but never compares its export header with the outer manifest's snapshot. Restore planning trusts the outer snapshot. Export itself correctly uses [one DbSnapshot](crates/rocklake-catalog/src/export.rs#L430), while backup reads inventory, retention, and pins afterward from the live database. Backup creation also opens the output with `File::create`, which can truncate an existing backup.

- [ ] Parse the export header during inspection. Cross-check snapshot and format information against the backup manifest before restore planning or mutation.
- [ ] Derive inventory and snapshot-dependent manifest fields from the exported view, or preserve the same storage snapshot across their collection. Give operational metadata a clear observation boundary.
- [ ] Stage backup output and publish completion only after data, manifest, and required durability steps succeed. Introduce non-overwrite behavior for completed artifacts and make interrupted attempts identifiable.

Acceptance: altering only the outer snapshot fails inspection. A paused backup remains internally consistent while catalog state changes through an allowed concurrent path. Interrupted creation never appears complete, and retry does not overwrite a valid backup. Extend the tests already in `backup.rs`.

### T06: Preserve retention boundaries and checkpoint dependencies

Size: medium. Impact: recovery and deletion safety.

[Writer open](crates/rocklake-catalog/src/store.rs#L141) converts any retention-floor read failure to zero. [pin_snapshot](crates/rocklake-catalog/src/gc.rs#L166) and [pin_checkpoint](crates/rocklake-catalog/src/checkpoint.rs#L295) accept snapshots without checking committed bounds or retention. [hold_snapshot](crates/rocklake-catalog/src/lease.rs#L51) already performs those checks transactionally.

Separately, [checkpoint creation](crates/rocklake-catalog/src/checkpoint.rs#L46) copies catalog state without registering a retention pin. Physical cleanup can therefore remove an object that a restorable checkpoint still references. This is a source-supported recovery risk that needs the complete regression sequence below.

- [ ] Propagate retention read and decode errors. Treat a missing floor as zero only through the existing missing-key handling.
- [ ] Apply the existing transactional bounds checks to both pin APIs, including races with GC.
- [ ] Define and enforce the object-retention dependency of a restorable checkpoint. Protect its references until explicit release, or fail clearly when restoration cannot recover them. Do not silently change the metadata-only backup contract into a data-copy promise.
- [ ] Make cleanup consider every live or protected reference to a canonical object path before deleting it.

Acceptance: corrupt retention metadata prevents open. Expired and future pins fail. A protected checkpoint retains readable objects through retirement, attempted retention advancement, cleanup, and restore; protection may correctly block advancement. Released or unsupported checkpoints fail clearly before restoring metadata that references missing objects. Cover duplicate references and explicit checkpoint release. Protection cannot recover objects already deleted.

### T07: Retry transaction conflicts without hiding permanent failures

Size: small. Impact: availability and error diagnosis.

[Writer epoch acquisition](crates/rocklake-catalog/src/store.rs#L131) and [retention advancement](crates/rocklake-catalog/src/gc.rs#L151) retry every commit error. A permanent error can be misclassified as contention and retried until another operation fails.

- [ ] Retry only actual transaction conflicts. Propagate fencing, corruption, and storage failures with their original cause.
- [ ] Use typed SlateDB error information where available. Review sibling retry loops that classify errors by string matching.
- [ ] Bound contention retries through the operation's deadline or a documented retry budget, with cancellation and a yield or backoff.

Acceptance: injected permanent commit failure returns within a short test deadline without a retry storm. A genuine conflict can retry successfully. Cancellation releases the operation and its admission permits.

## M2: Make concurrency predictable

### T08: Make catalog opening cancellation-safe and reserve capacity first

Size: medium. Impact: availability and memory scalability.

[open_route](crates/rocklake-router/src/lib.rs#L1014) clones a `Notify` before creating its waiting future. The opener can notify in between. The lost-notification mechanism was reproduced with the cached Tokio library. Cancelling the owning opener also leaves the `opening` entry behind. Finally, the router checks `max_open_catalogs` only after opening storage, which may already have acquired a writer epoch.

- [ ] Give each in-progress open explicit completion and cleanup ownership. Waiters must observe completed state even if completion precedes their wait.
- [ ] Reserve capacity before storage I/O, counting both opening and cached handles. Release the reservation on failure, cancellation, and eviction.
- [ ] Publish a successfully opened handle before making another opener eligible. Close discarded handles and retain useful close errors.

Acceptance: deterministic tests pause around waiter registration, abort the owning opener, and inject open failure. Every waiter terminates within a deadline. With capacity one, concurrent requests for distinct catalogs never open more than one store, and rejected requests perform no writer acquisition.

### T09: Define reload behavior for cached descriptors and quotas

Size: medium. Impact: catalog isolation and predictable administration.

[reload](crates/rocklake-router/src/lib.rs#L916) replaces the route table, but cached handles remain keyed only by catalog ID. New routes can return the previous handle after a location or mode change. [CatalogQuotaManager](crates/rocklake-pgwire/src/lifecycle.rs#L42) creates semaphores once, so changing an existing numeric limit does not resize them.

- [ ] Reject storage-identity replacement under an existing catalog ID, or implement a documented drain and reopen transition. Keep alias-only reloads inexpensive.
- [ ] Define mode transitions for existing and new sessions. An in-progress open must not publish a stale-generation descriptor after reload.
- [ ] Apply quota increases and decreases to future admission without losing accounting for existing permits. If a limit requires restart, reject that reload explicitly and say so.

Acceptance: reload tests cover location, mode, alias, and quota changes with live sessions and an in-progress open. New sessions never reach the wrong store. For supported live quota changes, reducing a limit from two to one prevents additional admission until usage is within the new limit, and increasing it makes extra capacity available. Restart-only quota or descriptor changes fail explicitly and preserve the old configuration.

### T10: Reproduce and close cancellation gaps in transaction cleanup

Size: medium. Impact: transaction correctness. Reproduction required before assigning final severity.

[execute_sql_with_mode](crates/rocklake-pgwire/src/executor/mod.rs#L218) clears buffered transaction state after an error result. Cancellation branches in [the handler](crates/rocklake-pgwire/src/handler.rs#L1823) drop that future and return directly. Source shows that cleanup is bypassed; a network test must establish the resulting session behavior.

- [ ] Test BEGIN, a buffered mutation, a blocked subsequent operation, cancellation, and COMMIT on the same connection.
- [ ] Put abort cleanup at the owner of request cancellation so simple query, extended query, COPY, timeout, and disconnect follow the same transaction contract.
- [ ] Check cancellation before commit separately from cancellation after durable commit. A completed commit must not be described as rolled back.

Acceptance: no previously buffered mutation can leak through a later COMMIT after the transaction aborts. A new transaction works afterward, protocol transaction status is correct, and permits and metrics return to baseline.

### T11: Parse DELETE batches with the existing SQL parser

Size: small. Impact: SQL correctness and user ergonomics.

The [executor](crates/rocklake-pgwire/src/executor/mod.rs#L248) looks for `delete` and `ducklake` anywhere in SQL, then splits on every semicolon. Literals, quoted identifiers, and comments can trigger or break this path.

- [ ] Use the existing `sqlparser` dependency and classifier for statement boundaries. Remove the keyword-triggered raw split.
- [ ] Preserve ordered batch execution, parameters, transaction behavior, and useful SQLSTATE errors.

Acceptance: the corpus covers semicolons and trigger words inside strings, quoted identifiers, and comments, plus a failing statement midway through a transaction. The same SQL has the same meaning when sent alone or in a batch.

### T12: Give FFI and Go handles safe ownership and observable close failures

Size: medium. Impact: memory safety and long-running process stability.

The [FFI contract](crates/rocklake-ffi/src/lib.rs#L207) requires exclusive access to each pointer. [Go Catalog](bindings/go/rocklake.go#L57) does not synchronize reads or close. [rocklake_close](crates/rocklake-ffi/src/lib.rs#L464) discards close errors and leaks the outer allocation on every cycle. The [sanitizer workflow](.github/workflows/sanitizers.yml#L20) disables leak detection globally to accommodate that leak.

- [ ] Serialize all Go operations and close through shared handle ownership. Account for copied Go wrapper values so they cannot introduce independent locks around one pointer.
- [ ] Add an error-returning close operation and an explicit final destruction contract. Preserve the existing close ABI or version its replacement; do not free memory while still promising safe calls through that freed pointer.
- [ ] Move bindings to the owned lifecycle and propagate native close failures. Restore leak detection, using only narrowly justified suppressions if needed.

Acceptance: concurrent reads and read/close races pass Go race checks plus native sanitizer checks. Repeated open, close, and destroy cycles release allocations. Injected close failure reaches the caller. Double-close and use-after-close behavior match the documented ownership contract.

## M3: Remove scaling obstacles and verify resource bounds

### T13: Remove repeated catalog scans from cleanup and quadratic name checks

Size: medium. Impact: maintenance throughput and large-catalog latency.

[Scheduled deletion](crates/rocklake-catalog/src/cleanup.rs#L274) calls `file_retired_at` for every candidate. That helper scans data-file and delete-file rows each time, making work proportional to deletion candidates multiplied by catalog size. [verify_name_set](crates/rocklake-catalog/src/verify.rs#L510) compares every pair of names, including unrelated owners and names.

- [ ] Gather retirement and reference evidence once per cleanup operation using canonical object identity. Preserve the deletion protections from T06.
- [ ] Group or sort verification intervals by owner and name before checking overlaps. Preserve exact overlap detection and deterministic diagnostics.
- [ ] Measure full verification memory, which currently retains many row categories. Bound diagnostics and document the supported administrative size. Add paging or spill only if those measurements require it.

Acceptance: use a counting store to show approximately linear scan work as scheduled candidates grow from 1,000 to 10,000. Verify disjoint and overlapping histories across many names. Report elapsed time and peak RSS before and after on identical data; do not relax correctness checks to meet the budget.

### T14: Measure response buffering at the point that retains memory

Size: medium. Impact: predictable memory under slow clients. Measurement required.

The [response wrapper](crates/rocklake-pgwire/src/handler.rs#L379) obtains a row before acquiring a buffer permit and drops the permit before yielding it downstream. This does not account for downstream retention. It does not, by itself, prove unbounded memory because socket and pgwire backpressure may already provide a bound.

- [ ] Run a local PG-wire client that pauses reads while several other clients request large metadata values. Measure retained rows, bytes, active scans, and cancellation latency.
- [ ] Enforce limits at the actual buffering owner if the native bound is insufficient. If native backpressure already works, simplify the redundant accounting and document what each remaining limit controls.
- [ ] Distinguish total response size from in-flight buffered bytes in configuration, errors, and metrics.

Acceptance: slow consumers remain within the declared memory budget, a fast consumer still makes progress, and early disconnect releases resources. Preserve pull-based streaming; do not add another queue without a measured need.

### T15: Make job controls accurately describe executable behavior

Size: medium for contract repair; a complete background worker is outside required scope. Impact: operator ergonomics and recoverability.

[maintenance run](crates/rocklake-pgwire/src/main.rs#L316) creates pending records without executing their operations. [jobs resume](crates/rocklake-pgwire/src/main.rs#L230) only requeues state. [run_job](crates/rocklake-pgwire/src/main.rs#L613) runs a closure between start and completion, without passing a durable checkpoint or checking cancellation. Durable records alone do not make those operations resumable.

- [ ] Inventory job kinds against their actual execution, progress, cancellation, and restart behavior. Reject unsupported resume or scheduling operations instead of implying that work will execute.
- [ ] Document the supported foreground commands and use the host process supervisor for recurring work that does not need a database scheduler.
- [ ] Where cancellation is supported, check it at safe work boundaries and record the last completed unit. Do not mark every failure retryable unconditionally.
- [ ] If retaining queued scheduling, make due-claim and job creation atomic or recoverable so a crash between them cannot lose a scheduled run.

Acceptance: each advertised job command either performs its operation or returns an explicit unsupported result. A cancellation and restart exercise demonstrates the promised behavior for each enabled kind. No command reports successful resumption merely because a state field changed. A generic job-execution service is not required for v0.64.

## M4: Make operator output actionable

### T16: Separate observed cache and capacity data from estimates

Size: small to medium. Impact: diagnosis and tuning ergonomics.

[cache_utilization](crates/rocklake-catalog/src/cache.rs#L160) synthesizes hits, misses, evictions, and a hit ratio, including a fixed 0.92 when the estimated working set fits. The [CLI](crates/rocklake-pgwire/src/main.rs#L2523) calls it with a fixed 256 MiB budget and prints those fields as cache statistics. [Capacity profiles](crates/rocklake-catalog/src/capacity.rs#L196) are constants, not a measurement of the inspected workload.

- [ ] Label working-set calculations and capacity profiles as estimates with their assumptions and evidence status. Remove fabricated observed counters or replace them with actual SlateDB observations.
- [ ] Report unknown values as unknown. Do not present zero observations as measured inactivity.
- [ ] Remove recommendations for nonexistent serving flags. Only recommend a cache control if its value reaches the actual storage configuration.
- [ ] Correct `snapshot_history`, which currently repeats the latest snapshot ID, or rename it through the public-schema policy.

Acceptance: an untouched catalog never reports invented cache hits. A known read sequence changes real counters if exposed. Capacity output distinguishes input rates, projections, profile assumptions, and measured results. JSON changes update the public manifest and retain compatibility where promised.

## M5: Certify the local release

### T17: Produce candidate-specific local performance and concurrency evidence

Size: medium. Impact: measurable quality, performance, and scalability.

Extend [rocklake-evidence](tools/rocklake-evidence/src/main.rs), not a second benchmark framework. It already uses fresh processes and records digests, timing, RSS, and object-store activity. However, it records digest mismatches without failing the run, hardcodes `v0.52.0`, measures export into a `Vec`, and counts GET bytes using whole-object metadata size even for range requests. Those details must be corrected before relying on the results.

- [ ] Fail on correctness mismatches and absent required measurements. Record the candidate SHA, binary hash, actual release identity, dependencies, backend version, and configuration.
- [ ] Count transferred or consumed ranges accurately, distinguish operation types, and state what retries and multipart activity the counters observe. Measure export to a counting sink or file so the runner does not introduce full-output buffering.
- [ ] Repeat operations and retain raw samples. Use medians for comparison and enough samples for any published percentile. Record first-row latency, completion latency, peak RSS, and post-close resource use separately.
- [ ] Add a duration-based local mixed workload using the existing testkit. The current [manual soak job](.github/workflows/ci.yml#L474) runs 1,000 in-memory schema cycles; it is not evidence of a 24-hour multi-process workload.

Use this staged matrix so routine work stays affordable:

| Stage | Environment | Workload | Required evidence |
|---|---|---|---|
| Every pull request | LocalFS and deterministic fault injection | Small populated catalog, lifecycle regressions, short benchmark | Correctness, termination, fresh metrics, resource release |
| v0.64 candidate | LocalFS | 10k and 100k visible files, old versions, deletes, wide metadata | Page/stream/full-read digest equality, latency, RSS, I/O |
| v0.64 candidate | Local processes | One writer and 1, 4, then 16 independent readers | Continued writes, bounded freshness, no fencing |
| v0.64 candidate | Local processes | Multiple catalogs, one noisy catalog, open/evict churn | Per-catalog isolation, global open bound, quiet-catalog latency |
| v0.64 candidate | LocalFS | At least 30 minutes of reads, writes, cancellation, and maintenance | Rolling latency and resource samples, final verification, reopen |
| Optional extended run | LocalFS or local MinIO | 1M files, 1,000 registered catalogs, larger active set | Explicit machine budget and the actually achieved envelope |
| Optional backend regression | Pinned local emulators | Functional and recovery cases | Backend-specific results with no real-cloud claim |

Acceptance: store raw reports and a short interpretation for the exact candidate. Define latency and memory budgets after measuring the corrected baseline and before evaluating subsequent changes. Missing larger runs reduce the documented envelope; they do not block the smaller proven release. If GCS emulator coverage is undertaken, fix startup and execute the suite; compilation alone does not count as execution.

### T18: Test upgrades and the actual release artifact

Size: medium. Impact: installation and release reliability.

The [release workflow](.github/workflows/release.yml#L90) already builds and installs artifacts. Its [artifact smoke script](scripts/test_release_artifact.sh) checks version and doctor. Release certification runs migration tests, but that is different from creating state with a previous released binary and restoring it with the candidate.

- [ ] Extend artifact validation to start the downloaded candidate and run a populated DuckDB quickstart, including reconnect and persistence. Verify reported SHA and artifact digests.
- [ ] Use v0.63.1 to create a catalog and backup, then use the candidate to open, read, write, close, reopen, and restore. Cover registry state when the change touches it.
- [ ] Keep v0.60.0 fixtures while it remains the documented minimum direct-upgrade source. Test unknown-format rejection before mutation. Exercise an unsafe downgrade only where the format contract actually makes it unsafe.
- [ ] Preserve the existing failure-certification, protocol, security, platform, documentation, and compatibility checks. Record unsupported or unavailable checks explicitly.

Acceptance: the new release works from produced artifacts in a clean directory without rebuilding application code. Previous-release state survives the documented upgrade and restore paths. Reconcile release notes and support claims with the evidence attached to this candidate. A completed v1.0 observation window is not an acceptance condition.

## Optional additions after the required work

### T19: Expose existing paging and refresh capabilities consistently

Size: medium. Depends on T03 and T12. Impact: client ergonomics and bounded memory.

The Rust client already has `get_table` and `list_data_files_paged`; language bindings largely expose materialized file lists. Reuse those operations for column descriptions and bounded pages. Add explicit refresh where the binding cannot currently advance a read-only snapshot. Correct Node's [declarations](bindings/nodejs/index.d.ts), which omit the implemented `openReadonly` method.

Acceptance: all pages at one exact snapshot concatenate to the complete expected dataset with no duplicate or missing rows. Refresh advances only according to the documented contract. A TypeScript consumer compiles against the packaged declarations. Defer additional streaming wrappers unless pages are insufficient.

### T20: Test populated bindings and installation outside the checkout

Size: medium. Impact: integration quality.

Reuse one local fixture containing schemas, columns, data files, Unicode, and IDs above `2^53`. The [Python dictionary test](bindings/python/tests/test_catalog.py#L56) and [Node ID test](bindings/nodejs/test/test_catalog.js#L66) currently exercise empty results or zero values. Test actual values, snapshot selection, numeric boundaries, URI handling, and closed handles.

Unify native URI resolution with the existing client behavior. [Go](bindings/go/rocklake.go#L77) advertises `file://`, while FFI passes its input directly to a filesystem-prefix constructor. Reject embedded NUL before C-string conversion. Validate Python wheels and packed Node packages from a clean directory. For Go, provide one working source-build installation route before considering prebuilt libraries; current cgo paths refer to the repository's header and `target/debug` directories.

Acceptance: the same populated fixture yields matching values across bindings, and a consumer outside the checkout runs using only the documented artifacts or source-build procedure. Keep bindings Experimental until their own support requirements are met. Package publication is not required.

### T21: Correct projected empty DataFusion scans

Size: small. Impact: integration correctness.

[The empty-table branch](crates/rocklake-datafusion/src/catalog_provider.rs#L881) constructs `EmptyExec` with the full schema and ignores projection. Apply the requested projection before constructing the empty plan.

Acceptance: selecting one column, reordered columns, and `COUNT(*)` from an empty multi-column table produces correct plan and result schemas. Extend `v04716_scan_fidelity_tests.rs`. Keep unsupported delete and inlined-data cases explicit; implementing a broader data engine is outside this plan.

### T22: Measure and bound registry history growth

Size: medium. Impact: management scalability.

[RegistryState](crates/rocklake-router/src/registry.rs#L360) keeps request-deduplication records in its serialized state. Profile registration, node renewal, authorization changes, and alias updates at increasing catalog and request counts. Measure mutation bytes, latency, and retained history independently of open catalog count.

Acceptance: publish the growth curve and an operational limit. If history dominates, add bounded retention with a documented idempotency window or compact through existing storage facilities. Test duplicate requests across that boundary and crash recovery. Do not introduce a distributed registry or new database without this measurement.

### T23: Make one administrative operation truly resumable

Size: medium to large. Depends on T15. Impact: recovery from interrupted large operations.

If restart cost remains material after the bounded foreground contracts are repaired, start with export. Persist its snapshot, output identity, cursor, completed byte boundary, and digest state or a verifiable reconstruction strategy using the existing job ledger. Reject incompatible output or changed parameters on resume.

Acceptance: interrupt after a known chunk, resume without duplicate or missing rows, and obtain the same digest as an uninterrupted export. Cancellation leaves a clearly incomplete artifact. Expand to another operation only after its restart cost justifies the extra state machine.

## What stays out of scope

- v1.0 dates, release candidates, the 30-day field-observation gate, and an independent production certification program.
- Live cloud benchmarks, cross-region trials, provider cost claims without measured inputs, and backend graduation based only on an emulator.
- Distributed multi-writer catalogs, shared-keyspace tenancy, cross-catalog transactions, and a new remote management API.
- New language bindings, automatic physical deletion, a general SQL engine, or expanded DataFusion mutation support.
- A custom cache, prefetch system, native checkpoint replacement, or generalized background scheduler before the existing paths are correct and measurements show a need.
- A storage-format change merely to deliver this plan. If a required repair changes a persisted or public contract, give it an explicit version and compatibility test.

## Verification performed for this assessment

This review traced storage opening, catalog reads, backup and cleanup, router opening and reload, request admission and cancellation, SQL batching, binding ownership, evidence generation, and release workflows. It compared the release tag with current `main`. No production code was changed and no remote-cloud test was run.

The following repository checks passed:

```text
python3 scripts/validate_compatibility_manifest.py
python3 scripts/validate_capacity_contract.py
python3 scripts/check_benchmark_regression.py benchmarks/baseline.json
```

The first reported 34 manifest entries and zero drift warnings. The latter two validate checked-in inputs; they do not establish runtime capacity or candidate performance.

Additional local probes produced these results:

| Probe | Result | Scope of the conclusion |
|---|---|---|
| Writer commit, independent read-only open, second writer commit | Second commit failed with `Closed error: detected newer DB client` | Actual local reader/writer failure using cached libraries whose implicated source matches this checkout |
| `notify_waiters` before creation of `notified()` | Wait timed out | Confirms the synchronization mechanism used in the router's race window, not a full router reproduction |
| Regression checker with an empty report | Returned success | Confirmed checker defect |
| Regression checker with a missing required latency metric | Returned success | Confirmed checker defect |
| Regression checker with throughput 1 and minimum 100 | Skipped the suffixed metric and returned success | Confirmed minimum-threshold defect |

The full Rust workspace, DuckDB matrix, sanitizers, emulator suites, large-scale runs, and upgrade matrix were not rerun for this documentation assessment. Their execution belongs to the task acceptance checks and final candidate certification. Source-inspection findings must become regressions before their fixes are considered complete.
