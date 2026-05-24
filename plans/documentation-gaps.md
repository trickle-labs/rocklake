# Documentation Gap Analysis

Comparison of the current `docs/` directory against `plans/documentation-1.md`.

**Date:** 2025-05-24

---

## Executive Summary

The documentation site is structurally complete: all 12 planned sections exist, the MkDocs
Material infrastructure is fully configured, the GitHub Actions CI/CD workflow is deployed,
and every planned page has meaningful content (no stubs). However, significant gaps remain
in three areas: (1) the Concepts section uses a different topic decomposition than planned,
omitting several key pages; (2) the Deployment section is missing all cloud-provider-specific
guides and credential isolation; (3) most pages are 40–130 lines — well below the "anti-anemia"
standard of lengthy, essay-style paragraphs that the plan requires.

| Category | Planned pages | Existing pages | Gap |
|----------|--------------|----------------|-----|
| Getting Started | 5 | 5 | **0** — complete |
| Concepts | 9 | 8 | **5 missing** (different decomposition) |
| Architecture | 9 | 9 | **2 missing** (different naming covers 7) |
| Deployment | 11 | 11 | **6 missing** (different topic choices) |
| Operations | 11 | 11 | **3 missing** (different naming covers 8) |
| Integration | 6 | 6 | **0** — complete |
| Design Decisions | 8 | 8 | **0** — complete |
| Performance | 5 | 5 | **0** — complete |
| Internals | 8 | 8 | **0** — complete |
| Contributing | 5 | 5 | **0** — complete |
| Reference | 6 | 6 | **0** — complete |
| Roadmap | 2 | 2 | **0** — complete |

---

## 1. Missing Pages

### 1.1 Concepts — Missing from Plan

The plan defines 9 concept pages; the current site has 8 with different topics. Pages
that exist in the plan but have no direct equivalent:

| Planned page | Status | Notes |
|--------------|--------|-------|
| `concepts/lakehouse-primer.md` | **Missing** | No standalone lakehouse explainer exists |
| `concepts/ducklake.md` | **Missing** | DuckLake format not explained in its own page |
| `concepts/slatedb.md` | **Missing** | SlateDB engine deserves dedicated treatment; `object-store-durability.md` covers part of this |
| `concepts/time-travel.md` | **Missing** | `snapshots.md` covers mechanics but lacks the full tutorial-style treatment the plan requires |
| `concepts/reader-scaleout.md` | **Missing** | Reader scale-out is mentioned in `single-writer-many-readers.md` but not separated |
| `concepts/writer-fencing.md` | **Missing** | Writer fencing protocol not covered in standalone page |
| `concepts/fact-store-vision.md` | **Missing** | Forward-looking vision page does not exist |
| `concepts/catalog-immutability.md` | **Present** as `concepts/immutability.md` | Name differs; content aligns well |

Current pages that are **not in the plan** but exist:

| Current page | Disposition |
|--------------|-------------|
| `concepts/bounded-sql.md` | Duplicates `design-decisions/bounded-sql.md` content; plan puts this only in Design Decisions |
| `concepts/catalog-vs-data.md` | Unique — not in plan but valuable; consider keeping |
| `concepts/key-value-mapping.md` | Overlaps with `architecture/key-layout.md`; plan does not have a concept-level key-mapping page |
| `concepts/object-store-durability.md` | Partially covers planned `concepts/slatedb.md` |

### 1.2 Architecture — Missing from Plan

| Planned page | Status | Notes |
|--------------|--------|-------|
| `architecture/system-design.md` | **Present** as `architecture/overview.md` | Different name, content maps well |
| `architecture/crate-map.md` | **Present** as `architecture/crate-structure.md` | Name differs |
| `architecture/pgwire-protocol.md` | **Present** as `architecture/pg-wire-protocol.md` | Hyphenation differs |
| `architecture/counter-allocation.md` | **Missing** | ID allocation protocol not documented |
| `architecture/data-flow.md` | **Missing** | End-to-end read/write path walkthrough not documented |

Extra page not in plan: `architecture/mvcc-implementation.md` — covers MVCC at storage level.
This content is partially covered by `concepts/mvcc.md` in the plan's structure.

### 1.3 Deployment — Missing from Plan

| Planned page | Status | Notes |
|--------------|--------|-------|
| `deployment/local-dev.md` | **Missing** | Plan wants a dedicated local-filesystem guide |
| `deployment/aws-s3.md` | **Missing** | No AWS S3 production guide |
| `deployment/aws-s3-express.md` | **Missing** | S3 Express One Zone not covered |
| `deployment/gcs.md` | **Missing** | Google Cloud Storage not covered |
| `deployment/azure.md` | **Missing** | Azure Blob Storage not covered |
| `deployment/minio.md` | **Missing** | MinIO self-hosted not covered |
| `deployment/credential-isolation.md` | **Missing** | IAM separation (catalog vs data plane) not documented |
| `deployment/tls-and-auth.md` | **Present** as `deployment/tls.md` | Missing the authentication half |

Extra pages not in plan (but valuable):

| Current page | Disposition |
|--------------|-------------|
| `deployment/binary.md` | Partially maps to planned `local-dev.md`; good standalone content |
| `deployment/fly-io.md` | Not in plan; covers a specific PaaS target |
| `deployment/high-availability.md` | Not in plan as standalone; concepts folded into Kubernetes guide |
| `deployment/multi-region.md` | Not in plan; forward-looking content |
| `deployment/networking.md` | Not in plan; useful operational content |
| `deployment/configuration.md` | Plan places this under Operations |

### 1.4 Operations — Missing from Plan

| Planned page | Status | Notes |
|--------------|--------|-------|
| `operations/cli-reference.md` | **Missing** | No comprehensive CLI command reference |
| `operations/configuration.md` | **Exists** in `deployment/configuration.md` | Plan puts it in Operations |
| `operations/checkpoints.md` | **Missing** | SlateDB checkpoints not separately documented |
| `operations/encryption.md` | **Missing** | At-rest encryption with block transformers not documented |
| `operations/gc-and-retention.md` | **Present** as `operations/garbage-collection.md` | Naming differs |
| `operations/export-import.md` | **Partially present** as `operations/export.md` + `operations/backup-restore.md` | Split across two files |
| `operations/repair.md` | **Present** as `operations/verify-repair.md` | Name differs |
| `operations/upgrading.md` | **Present** as `operations/upgrades.md` | Name differs; content is thin (28 lines) |

Extra pages not in plan:

| Current page | Notes |
|--------------|-------|
| `operations/health-checks.md` | Plan folds this into Monitoring or Kubernetes |
| `operations/inspect.md` | Plan folds this into CLI Reference |
| `operations/logging.md` | Plan folds this into Monitoring |

---

## 2. MkDocs Configuration Gaps

| Planned feature | Status | Impact |
|-----------------|--------|--------|
| `social` plugin | **Missing** from `mkdocs.yml` | No Open Graph social cards for link previews |
| `enable_creation_date: true` | **Set to `false`** | "Created" timestamps not shown on pages |
| `extra_javascript` for MathJax/KaTeX | **Missing** | `pymdownx.arithmatex` configured but no JS loaded |
| Custom `why-this-matters` admonition in CSS | **Unknown** | Not verified in `extra.css` |

The `requirements-docs.txt` is fully aligned with the plan (all packages present and pinned).

The GitHub Actions workflow matches the plan exactly.

---

## 3. Content Depth Assessment ("Anti-Anemia" Standard)

The plan mandates that pages be "lengthy, interesting, informative, useful, engaging, and
written in longer paragraphs throughout," with narrative sections targeting 5–10 sentence
paragraphs. Current page lengths:

| Range | Count | Plan expectation |
|-------|-------|-----------------|
| < 30 lines | 6 | Index pages — acceptable for some, but plan wants substantive introductions |
| 30–60 lines | 30 | **Below standard** — concept/design pages should be 2–3× longer |
| 60–100 lines | 40 | **Borderline** — some meet the bar, most could be expanded |
| 100–160 lines | 17 | **Acceptable** for reference; still thin for concept/architecture essays |
| > 160 lines | 0 | Plan expects concept pages in the 200–400 line range |

**Assessment:** No page in the current site exceeds 155 lines. The plan's description of
each concept page implies 2,000–4,000 words of flowing prose. The existing pages average
~800–1,500 words. Most pages need to be expanded 2–3× to meet the anti-anemia standard.

Pages most urgently needing expansion to match plan depth:

1. `concepts/immutability.md` (45 lines) — plan describes a full essay on benefits, costs,
   interaction with GC, comparison with PostgreSQL's MVCC, concrete reader/writer scenarios
2. `architecture/overview.md` (105 lines) — plan's `system-design.md` calls for full sequence
   diagrams of read/write paths, concurrency model, and crate boundaries
3. `design-decisions/bounded-sql.md` (33 lines) — plan expects a thorough taxonomy of
   supported shapes, AST-matching rationale, security implications
4. `design-decisions/single-writer.md` (39 lines) — plan expects multi-page comparison with
   OCC, Raft, multi-writer, including fencing protocol walkthrough
5. `operations/upgrades.md` (28 lines) — plan expects full compatibility matrix, migration
   workflow, three-step export/reinit/import process

---

## 4. Structural Discrepancies

### 4.1 Concepts Section — Different Decomposition

The plan decomposes concepts around the *system's components and properties*:
- Lakehouse Primer → DuckLake → SlateDB → Immutability → MVCC → Time Travel → Reader Scale-Out → Writer Fencing → Fact Store Vision

The current site decomposes around *technical mechanisms*:
- Bounded SQL → Catalog vs Data → Immutability → Key-Value Mapping → MVCC → Object Store Durability → Single Writer → Snapshots

The plan's approach is more pedagogical (builds from first principles for newcomers); the
current approach is more technical-reference-oriented (each page covers one mechanism).

**Recommendation:** Restructure toward the plan's decomposition. Several current pages can
be merged or reorganized:
- `object-store-durability.md` → becomes `concepts/slatedb.md`
- `single-writer-many-readers.md` → splits into `concepts/reader-scaleout.md` + `concepts/writer-fencing.md`
- `snapshots.md` → becomes `concepts/time-travel.md`
- `bounded-sql.md` → move to Design Decisions only (avoid duplication)
- `key-value-mapping.md` → fold into `architecture/key-layout.md`

### 4.2 Deployment Section — Missing Cloud Guides

The plan's most critical gap is **cloud-provider-specific deployment guides**. The plan
envisions self-contained guides for AWS S3, S3 Express, GCS, Azure, and MinIO — each with
full IAM policies, environment variable setup, bucket configuration, and verification steps.
None of these exist. The current `deployment/` section focuses on *how to run the binary*
rather than *how to deploy against a specific cloud*.

### 4.3 Navigation Structure

The plan's nav structure vs. actual nav differs in several places:
- Plan: `Configuration` under Operations | Actual: under Deployment
- Plan: single `tls-and-auth.md` | Actual: `tls.md` (no auth content)
- Plan: no `binary.md`, `fly-io.md`, `networking.md`, `multi-region.md` | Actual: present
- Plan: no `health-checks.md`, `inspect.md`, `logging.md` | Actual: present as separate pages

---

## 5. Quality Standard Gaps

### 5.1 Working Examples

The plan requires every concept page to have at least one DuckDB SQL example showing the
concept in action, and every deployment guide to have copy-paste commands with expected
output. Spot-checking indicates most pages include code blocks, but many lack:
- Expected output after commands
- Verification steps ("you should see...")
- Error examples for common misconfigurations

### 5.2 Mermaid Diagrams

The plan calls for system design sequence diagrams (read path, write path) and a crate
dependency graph. The existing `architecture/overview.md` includes a component diagram and
`architecture/crate-structure.md` has a dependency graph. Missing:
- Read-path sequence diagram (DuckDB → pgwire → sql → catalog → SlateDB → response)
- Write-path sequence diagram (BEGIN → accumulate → COMMIT → DbTransaction → flush)
- Data flow page with timing annotations from phase-0 baseline

### 5.3 Cross-Linking

The plan emphasizes that each page should link to related deeper content. Current pages
tend to be self-contained with minimal cross-references. A systematic cross-linking pass
is needed.

### 5.4 Glossary Completeness

The plan specifies key glossary entries (`dl_snapshot_id`, `catalog-data fact`,
`infrastructure state`, `retain-from`, `excision`, `writer fencing`, `kv_snapshot`).
The current `reference/glossary.md` exists but should be verified against this list.

---

## 6. Priority Remediation Plan

### P0 — Critical Missing Content

These gaps represent information a user cannot find anywhere in the current docs:

1. **Cloud deployment guides** (`aws-s3.md`, `gcs.md`, `azure.md`, `minio.md`) — blocks
   production adoption
2. **Credential isolation** (`credential-isolation.md`) — security-critical for production
3. **CLI reference** (`cli-reference.md`) — operators need this daily
4. **Counter allocation** (`counter-allocation.md`) — essential for contributors
5. **Encryption** (`encryption.md`) — required for compliance deployments

### P1 — Missing Concept Pages

These gaps mean the documentation fails to explain the system from first principles:

6. **Lakehouse primer** (`lakehouse-primer.md`) — newcomers lack the on-ramp
7. **DuckLake format** (`ducklake.md`) — the format SlateDuck implements is unexplained
8. **SlateDB engine** (`slatedb.md`) — the storage engine has no dedicated explanation
9. **Fact store vision** (`fact-store-vision.md`) — forward-looking motivation is absent
10. **Time travel** (`time-travel.md`) — exists partially in `snapshots.md`, needs dedicated page
11. **Reader scale-out** (`reader-scaleout.md`) — merged into single-writer page currently
12. **Writer fencing** (`writer-fencing.md`) — protocol walkthrough not documented

### P2 — Content Depth

Every existing page needs expansion to meet the anti-anemia standard. Highest priority:

13. All Design Decisions pages (33–65 lines → target 150–250 lines)
14. All Concepts pages (45–97 lines → target 150–300 lines)
15. Architecture pages (79–127 lines → target 200–400 lines)
16. `operations/upgrades.md` (28 lines → target 100+ lines)

### P3 — Structural Alignment

17. Restructure Concepts section to match plan's pedagogical flow
18. Move `deployment/configuration.md` to Operations
19. Add authentication content to `deployment/tls.md` (→ `tls-and-auth.md`)
20. Resolve `concepts/bounded-sql.md` duplication with `design-decisions/bounded-sql.md`
21. Add `social` plugin to mkdocs.yml
22. Enable `creation_date` in git-revision-date-localized plugin
23. Add data-flow and counter-allocation architecture pages
24. Add S3 Express One Zone deployment guide

---

## 7. Files That Exist But Are Not in Plan

These pages were added outside the plan's scope and should be evaluated for retention:

| File | Recommendation |
|------|---------------|
| `deployment/binary.md` | **Keep** — useful; subsumes part of planned `local-dev.md` |
| `deployment/fly-io.md` | **Keep** — additional deployment target |
| `deployment/high-availability.md` | **Keep** — expand per plan's HA discussion in Kubernetes |
| `deployment/multi-region.md` | **Keep** — forward-looking |
| `deployment/networking.md` | **Keep** — practical operational value |
| `operations/health-checks.md` | **Keep** — reference plan's monitoring section |
| `operations/inspect.md` | **Fold** into future `cli-reference.md` |
| `operations/logging.md` | **Fold** into `monitoring.md` |
| `concepts/bounded-sql.md` | **Remove or redirect** to `design-decisions/bounded-sql.md` |
| `concepts/catalog-vs-data.md` | **Keep** — valuable unique content |
| `concepts/key-value-mapping.md` | **Keep** or fold into `architecture/key-layout.md` |
| `architecture/mvcc-implementation.md` | **Keep** — bridges concepts and internals |
| `docs/phase-0/` (7 files) | **Keep** — research artifacts, not in nav |

---

## 8. Summary Metrics

| Metric | Plan target | Current state | Gap |
|--------|-------------|---------------|-----|
| Total planned pages | 84 | 93 (different set) | 16 missing, 25 extra |
| Pages > 150 lines | ~50 (concepts, arch, deployment) | 4 | **~46 pages need major expansion** |
| Cloud deployment guides | 5 | 0 | **5 missing** |
| Sequence diagrams | 2+ | 0 | **2 missing** |
| Working examples with output | All pages | ~60% | ~40% need examples added |
| Social plugin / OG cards | Yes | No | Not configured |
| CLI reference | Complete | Does not exist | **Critical gap** |
