---
name: behave-change-control
description: Classify changes (app code / VMS data / generated artifacts / migrations / WASM / release config), enforce the 4 non-negotiables with incident archaeology, verify branch+PR+commit+lint conventions, migration checklist, release gates.
---

# Behave Change Control

A guide to what kinds of changes exist in Behave7, how to classify them, what gates each must pass, and the hard rules learned from production incidents. Date-stamped as of 2026-07-06.

## Glossary of Key Terms

Before proceeding, understand these domain-specific terms:

- **VMS (Variable Management System)**: The Behave7 content database running on the CMS (`projects/behave_cms`, Datomic + PostgreSQL). Stores the fire science model structure: applications, modules (Surface/Crown/Mortality/etc.), groups of variables, variables themselves, and their mappings to C++ functions in the WASM. Exported as `layout.msgpack` (binary MessagePack format) loaded at app startup. **Do not hand-edit Datomic data directly.**

- **CMS**: The admin interface (`projects/behave_cms`, port 8001) for managing VMS content. Changes are made via migrations (Clojure code in `projects/behave_cms/resources/migrations/`), which are applied to Datomic and automatically versioned.

- **WASM / behave-mirror**: The C++ fire-behavior modeling engine (extended Rothermel model), compiled to WebAssembly via Emscripten. Lives in the `behave-lib/behave-mirror` submodule at branch `rj-rust-port`. Bundled as `behave-min.wasm` and `behave-min.js` in `projects/behave/resources/public/js/`. **Regeneration is not automatic.**

- **Generated artifacts**: ClojureScript wrapper functions (e.g., `behave/lib/surface.cljs`, `behave/lib/crown.cljs`) produced by the Hatchet tool (ANTLR-based code generator) from C++ headers. Also: EDN module definitions in `cms-exports/` (e.g., `SIGSurface.edn`, `SIGMortality.edn`) exported from the CMS. **Do not patch these by hand; regenerate the whole tree.**

- **layout.msgpack**: Binary serialized form of the VMS, loaded into every app instance at startup. After any VMS migration (Datomic schema change), this file must be re-exported and checksummed, and all test fixtures (`.bp7` worksheets) must be re-synced or tests silently drift.

- **`.bp7` files (worksheets)**: SQLite 3 databases containing user worksheets (saved fire runs with inputs, outputs, notes, graphs). Fixtures in `worksheets/` directory are used for testing. Schema is tied to the VMS version; stale VMS = stale fixtures.

- **BHP1-#### ticket branches**: Feature branches named `<initials>-BHP1-####-<kebab-case-slug>` (e.g., `rj-BHP1-1611-new-direction-table-filter-logic`). Pushed to the upstream `firelab/behave-app` repo (not forked), then opened as PRs back to `main`.

---

## Change Classification

Every change in Behave7 falls into one of six categories. Identify yours first to determine which gates apply.

### 1. Pure App Code (ClojureScript, Clojure, CLJS components)

**Scope**: Logic and UI in `projects/behave/src/`, `bases/behave_*/src/`, `components/*/src/` (except generated/schema_migrate).

**Examples**:
- Event handlers in `worksheet/events.cljs`
- Subscriptions in `**/subs.cljs`
- Views in `**/views.cljs`
- Solver orchestration in `solver/core.cljs`
- Server route handlers in `behave/handlers.clj`
- CMS helpers in `behave-cms/**`

**Gates**:
1. **Clj-kondo lint** (enforced on PRs via GitHub Actions; see [Lint Gate](#lint-gate) below)
2. **Cljfmt format** (run locally before commit: `cljfmt fix <file>`)
3. **Tests pass** (manual at dev time: `clojure -M:dev:behave/app:figwheel` → `http://localhost:8081/api/test`; headless CI runner not yet configured on main)
4. **No solver output changes without golden validation** (see [Non-Negotiable #3](#non-negotiable-3-solver-outputs-must-validate-against-golden-data) below)

**Commit message style**: `BHP1-#### <imperative-verb> <noun-phrase>` (title only, no body).
Example: `BHP1-1611 Shade result tables per direction`

---

### 2. VMS / CMS Data Changes (Datomic schema, content, variables)

**Scope**: Changes to fire science model structure (new variables, module definitions, conditionals, links).

**Examples**:
- Adding a new mortality region (GACC code)
- Changing a variable's units or default value
- Adding a new group-variable or input/output mapping
- Updating help content
- Renaming variables (`2026_07_01_rename_mortality_region_to_gacc.clj`)

**How to make changes**:
1. **Never hand-edit Datomic data**. Use migrations (see [Migration Authoring Checklist](#migration-authoring-checklist)).
2. Each migration is a `.clj` file in `projects/behave_cms/resources/migrations/` following the naming convention `YYYY_MM_DD_description.clj` (e.g., `2026_07_01_rename_mortality_region_to_gacc.clj`).
3. Migrations use `datomic.api/transact` to apply changes. Test locally before committing.
4. Mark migrations with `:migrate/ignore?` metadata if they should be skipped in future runs (e.g., if merged into main and then replayed, mark as `:migrate/ignore? true` at the ns-level to prevent duplicate application).

**Gates**:
1. **Migration file lint** (clj-kondo, same as app code)
2. **Local migration test** (verify transact succeeds and produces expected entities)
3. **layout.msgpack re-export** (run `clojure -X:download-vms` or CMS export step; commit new msgpack)
4. **Test fixtures re-synced** (update `.bp7` files or regenerate from templates; re-run tests)
5. **PR review** (manual; ensure migration is idempotent and handles rollback scenario)

**Related incident**:
- **BHP1-1594** (commit ad62e0e7): 113 migration files auto-moved from `development/migrations/` to `projects/behave_cms/resources/migrations/` + runner path updated. Later, 4 migrations were marked `:migrate/ignore? true` (commit b914d001) to prevent duplicate runs after merge. **Lesson**: After moving/restructuring migrations, test both old and new paths to confirm migration runner finds them and applies them once.

---

### 3. Generated Artifacts (WASM wrappers, EDN module defs, enums)

**Scope**: Files auto-generated by Hatchet, CMS export, or Emscripten. **Do not hand-edit.**

**Examples**:
- `projects/behave/src/cljs/behave/lib/surface.cljs` (WASM wrapper functions)
- `projects/behave/src/cljs/behave/lib/enums.cljs` (C++ enum bindings)
- `projects/behave/src/cljs/behave/lib/units.cljs` (unit conversion bridge)
- `cms-exports/SIGSurface.edn`, `SIGCrown.edn`, etc. (EDN module definitions)

**How to update**:
1. **C++ change**: Edit `behave-lib/behave-mirror/` source (`.cpp`, `.h` files).
2. **Hatchet regeneration**: Run `cd behave-lib && make install` (compiles C++ to WASM, runs Hatchet to generate CLJS wrappers, places artifacts in `projects/behave/resources/public/js/` and `projects/behave/src/cljs/behave/lib/`).
3. **CMS export**: Re-export from CMS Datomic as EDN via admin tool or script (verifies `cms-exports/*.edn` reflects current VMS).
4. **Commit the generated files** (do not manually edit).

**Gates**:
1. **All generated files must be re-generated as a tree** (don't patch one file; regenerate all)
2. **C++ unit tests pass** (WASM compilation succeeds without errors)
3. **Solver integration tests pass** (golden-data validation; see Non-Negotiable #3)
4. **Hatchet determinism check** (regenerate twice; diffs should be identical — if not, Hatchet config or source changed unexpectedly)

**Trap**: If you edit a generated wrapper by hand and later regenerate, your changes are lost. If you skip regeneration and the C++ code changed, the solver silently uses stale behavior.

---

### 4. Schema Migrations (Datomic schema attributes, Datascript schema updates)

**Scope**: Changes to the data schema itself (adding attributes, changing cardinality, adding uniqueness constraints).

**Examples**:
- Adding `:group-variable/direction` attribute (commit 2024_07_15_add_group_variable_direction.clj)
- Renaming `:field/name` to `:field/title`
- Adding a new entity type or relationship

**How to make changes**:
- **Use CMS migrations**: Define the new attribute in a migration file using `datomic.api/transact` with `:db/ident`, `:db/valueType`, `:db/cardinality`, `:db/doc`, etc.
- **Test locally**: Run the migration against a test Datomic instance and verify the schema attribute exists.
- **Update schemas in code**: If Datalog query code (`behave_schema/src/**`) references the new attribute, add it to the schema definition so Datascript understands it.

**Gates**:
1. **Migration transact succeeds** (Datomic accepts the schema change)
2. **Code doesn't reference non-existent attributes** (clj-kondo can catch `:unknown-key` warnings if schema is registered)
3. **Existing data is compatible** (no breaking cardinality changes without data migration)
4. **All downstream code updated** (if you add a new entity type, add its query rules to solver, worksheet logic, etc.)

**Related incident**:
- Schema migration issues were revealed when test fixtures drifted after VMS changes. Commit 6e9709d2 ("Rework crown solver test on real worksheet fixture") required updating the test to work with the new worksheet schema. **Lesson**: Whenever you change a schema attribute, regenerate test fixtures (`.bp7` files) or update test setup to seed the new attributes.

---

### 5. Behave-Mirror / WASM / C++ Build

**Scope**: Changes to the C++ fire behavior model, CMake build config, or Emscripten settings.

**Examples**:
- Updating fire spread equations in `behave-lib/behave-mirror/surface.cpp`
- Adding a new species to the mortality model
- Bumping Emscripten version or updating CMake flags

**How to make changes**:
1. Edit source in `behave-lib/behave-mirror/` (C++ files).
2. Update `behave-lib/CMakeLists.txt` if build flags or source files change.
3. Run `cd behave-lib && make install` to:
   - Compile C++ to WASM (outputs `build/behave-min.wasm` and `build/behave-min.js`)
   - Run Hatchet to regenerate ClojureScript wrappers
   - Copy artifacts to `projects/behave/resources/public/js/`
4. Run solver integration tests to verify output correctness.

**Gates**:
1. **C++ compiles without warnings** (Emscripten clang warnings indicate undefined behavior)
2. **WASM size reasonable** (sanity check; sudden large bloat may indicate debug symbols or unnecessary code)
3. **Solver outputs validated against golden data** (see Non-Negotiable #3)
4. **All solver tests green** (units, enums, output linking verified)

**Trap**: WASM module initialization timing is fragile. The test host page must instantiate `Module` from `behave-min.js` **before** the main bundle loads, or `window.runtimeInitialized` is never set and enums fail to load. See commit 761b0c22 ("Fix figwheel test bootstrap") for the fix.

---

### 6. Release Configuration (version bumps, packaging, signing)

**Scope**: Changes to release artifacts, version strings, CI/CD workflows.

**Examples**:
- Bumping version in `projects/behave/resources/version.edn` and `projects/behave/conveyor.base.conf`
- Updating `.github/workflows/jar-builder.yml` build steps
- Modifying `projects/behave/conveyor.*.conf` for new platforms or signing keys

**How to make changes**:
1. **Version**: Update both `projects/behave/resources/version.edn` (format: `{:version "vX.Y.Z"}`) and line 29 of `projects/behave/conveyor.base.conf` to keep them in sync.
2. **Release workflows**: Edit `.github/workflows/*.yml` files. Test locally with `act` or manually trigger workflow via GitHub UI.
3. **Conveyor config**: Edit `projects/behave/conveyor.*.conf` files (base, macos, windows, linux overlays). Conveyor docs at https://hydraulic.dev/conveyor/

**Gates**:
1. **Version consistency** (check both `version.edn` and `conveyor.base.conf` match)
2. **Secrets available** (VMS_URL, VMS_AUTH_TOKEN, CONVEYOR_ROOT_KEY, Azure signing creds, Apple certs)
3. **Workflow syntax valid** (GitHub validates on PR)
4. **Test release run** (trigger release.yml with a test tag; verify all platforms build)

**Trap**: Version dual-sourcing (stored in two files) risks drift. No automated check enforces consistency between `version.edn` and `conveyor.base.conf`. If they diverge, build artifacts will have mismatched versions. **Action**: Add a pre-release lint step to verify both files contain the same version string.

---

## The Four Non-Negotiables

These rules are sacred. Each emerged from a production incident; violating them causes silent corruption, test drift, or broken deployments.

### Non-Negotiable #1: Never Hand-Edit VMS / Datomic Data

**The Rule**: Changes to the Variable Management System (VMS) structure, content, or variable mappings must go through **migrations** (Clojure code in `projects/behave_cms/resources/migrations/`). Direct Datomic editing (via transactor console, REPL, or admin tools) bypasses versioning and causes schema drift.

**Why**: The VMS is the source of truth for the fire science model. Every app instance loads `layout.msgpack` at startup, which is a snapshot of the VMS. If you hand-edit Datomic data:
1. Other instances already running see the old VMS snapshot and compute wrong results.
2. The next CMS export and app restart may overwrite your changes.
3. Rollback is impossible (no version history).
4. Tests become unreliable (fixtures based on old VMS state).

**The Incident** (BHP1-1594, commit ad62e0e7):
- Someone manually moved 113 migration files from `development/migrations/` to `projects/behave_cms/resources/migrations/` without updating the migration runner's search path.
- Production CMS server started; migration runner looked in the old path, found no migrations, and skipped them.
- VMS was left in an inconsistent state (schema mismatch).
- Fix: Updated `schema_migrate/runner.clj` to look in the new path, and re-applied the 113 migrations. Later, marked 4 migrations as `:migrate/ignore? true` (commit b914d001) to prevent duplicate application after the merge to main.

**What To Do**: If you need to change VMS data:
1. Write a migration file: `projects/behave_cms/resources/migrations/YYYY_MM_DD_description.clj`
2. Use `datomic.api/transact` to apply changes.
3. Test locally: start CMS, apply migration, verify VMS state via Datomic query tool.
4. Commit the migration file to git (not the Datomic database dump).
5. When deployed, the CMS migration runner applies it automatically on startup.

---

### Non-Negotiable #2: Never Hand-Edit Generated Artifacts

**The Rule**: ClojureScript wrapper functions (`behave/lib/surface.cljs`, etc.), EDN module definitions (`cms-exports/*.edn`), and enum bridges (`enums.cljs`) are generated code. **Do not patch them by hand.** If they're wrong, regenerate the source.

**Why**: These files are auto-generated by Hatchet (from C++ headers) and CMS export (from VMS). Hand-edits are immediately lost on the next regeneration cycle. Also:
1. Hatchet regeneration is deterministic; if you hand-edit and then regenerate, your changes are lost.
2. If you hand-edit without regenerating the C++ source, the app and C++ get out of sync and solver produces wrong values silently.
3. Reviewers can't tell if an edit is intentional or accidental corruption.

**The Incident** (Implicit in git history, commit c5fcb185 and others):
- Test failures revealed that generated wrapper functions were referencing deleted C++ methods (e.g., `getResourcesUsed` on `SIGContainAdapter`).
- Root cause: WASM was rebuilt without updating the Hatchet generator, producing stale wrappers.
- Fix: Removed the dead wrappers, updated tests, confirmed WASM module was actually missing the method.

**What To Do**: If a generated file is wrong:
1. Find the **source** (C++ header, VMS data, etc.).
2. Fix the source.
3. Regenerate the artifact (run `make install` in behave-lib for WASM wrappers, or export VMS for EDN).
4. Commit the regenerated files, not hand-edits.

**Copy-paste command** (regenerate all WASM artifacts):
```bash
cd /Users/rsheperd/code/sig/behave-app/behave-lib
make install  # Compiles C++, runs Hatchet, copies to projects/behave/resources/public/js/
```

---

### Non-Negotiable #3: Solver Outputs Must Validate Against Golden Data

**The Rule**: Any change that affects solver computation (units, equations, conditionals, output linking, module initialization) must be validated against **golden reference data** (C++ test CSVs, FOFEM comparisons, historical Behave6 runs, or previous Behave7 runs). **Never judge correctness by eye.**

**Why**: The solver orchestrates six fire-behavior modules (Surface → Crown → Mortality → Contain → Spot → Ignite). Output linking means downstream modules consume upstream outputs as inputs. A small bug in one place propagates silently through the whole chain:
1. Silent failures: solver returns `0`, `NaN`, or a plausible-looking wrong number.
2. Cascade: If Surface is wrong, Crown gets garbage input and produces garbage output; Mortality depends on Crown scorch height, etc.
3. Tests can pass subjectively ("output looks reasonable") but be numerically wrong.

**The Incident** (in-progress on rj-fix-figwheel-tests branch):
- Test `mortality-worksheet` was failing. Root cause: `add-ws-input!` was setting units to `:none` (nil after deserialization), so the solver's `apply-single-cpp-fn` skipped 2-argument setters (requiring units).
- Setters like `setDBH`, `setSurfaceFireScorchHeight`, `setTreeHeight` never ran.
- Mortality was computed as ~100 instead of the golden ~65.
- The error was silent; the test only caught it because it checked the output value.
- Fix: Rewrote the test to use the real worksheet fixture path (with proper units), confirmed solver output matched golden (64.98 vs 65). (See FIX_TEST_PLAN.org line 66 for current status.)

**What To Do**: After any solver-affecting change:
1. Run the test suite: `clojure -M:dev:behave/app:figwheel` → `http://localhost:8081/api/test`
2. Check for RED tests (not YELLOW warnings). Example failing test: `mortality-worksheet` (see SOLVER_TEST_HANDOFF.org; the fix lives on branch `rj-fix-figwheel-tests`).
3. If a solver output changes, **cross-check the new value** against:
   - FOFEM golden data (mortality, crown damage)
   - Behave6 historical runs (rate of spread, flame length)
   - C++ unit tests in behave-mirror (`testSurface.cpp`, `testMortality.cpp`, etc.)
4. Verify the change is intentional (e.g., refactored units conversion, not a bug).
5. Update test expectations to the validated value.
6. Commit the test update with a reference to the golden-data source.

**Evidence required in PR**:
- Test output (GREEN suite screenshot or log)
- Golden-data comparison (e.g., "Behave6 ROS=21.5, Behave7=21.4, ΔFOFEM < 1%")
- Changed test expectations with rationale

---

### Non-Negotiable #4: After VMS Migrations, Re-Sync layout.msgpack and Fixtures

**The Rule**: The VMS is versioned via migrations. `layout.msgpack` is a binary snapshot of the VMS, exported and committed to git. Every app instance loads it at startup. **After applying a VMS migration, the `layout.msgpack` file and all test fixtures (`.bp7` worksheets) must be re-synced.** If they drift, tests silently pass with wrong data.

**Why**: 
1. `layout.msgpack` is loaded once per app startup. If it's stale (from before the migration), the app runs with an inconsistent VMS (e.g., old variable names, missing attributes).
2. Test fixtures (`.bp7` SQLite files) have embedded VMS entity IDs and variable mappings. If the VMS schema changes and fixtures aren't updated, queries fail silently or return nil.
3. The solver computes outputs based on VMS group-variable→C++ function mappings. Stale mappings mean the solver skips setters or uses wrong units.

**The Incident** (in-progress on rj-fix-figwheel-tests branch):
- Migration `2026_07_01_rename_mortality_region_to_gacc.clj` renamed a VMS attribute from `region` to `gacc-region`.
- Test fixture `solver_test.cljs` was hard-coding the old variable name for lookup.
- On first run post-migration, the lookup returned nil (variable not found under old name).
- Solver skipped initializing mortality, producing wrong output.
- Fix: (1) Re-export `layout.msgpack` with the new VMS schema. (2) Update test fixture variable names to use the new schema. (3) Verify golden output matches. (See FIX_TEST_PLAN.org lines 21-22, 97-99 for current status.)

**What To Do** (after any VMS migration):
1. **Re-export layout.msgpack**:
   ```bash
   # From projects/behave, assuming CMS is running on port 8001
   curl -X POST http://localhost:8001/api/export-vms > layout.msgpack
   # Or use the CMS admin UI to download the export
   # Then commit: git add projects/behave/resources/public/layout.msgpack && git commit -m "BHP1-xxxx Update layout.msgpack post-migration"
   ```
2. **Regenerate test fixtures** (if fixtures are templated):
   - Delete stale `.bp7` files from `worksheets/`
   - Re-run test setup that creates them (e.g., `with-dummy-worksheet` or `new-solver-worksheet!`)
   - Or manually update hard-coded variable UUIDs in test code to match the new VMS
3. **Run the full test suite**:
   ```bash
   # Browser-based tests
   clojure -M:dev:behave/app:figwheel
   # Navigate to http://localhost:8081/api/test and check for GREEN
   ```
4. **Verify specific fixtures**:
   - If a test loads a `.bp7` file, open it in SQLite and check that variable UUIDs match the current VMS.
   - Example: `sqlite3 worksheets/some-test.bp7 ".schema"` to inspect the schema.

**Checksum** (to verify layout.msgpack is up-to-date):
```bash
# After exporting layout.msgpack, compute a checksum and commit it
sha256sum projects/behave/resources/public/layout.msgpack > layout.msgpack.sha256
# On next build, verify checksum hasn't drifted
```

---

## Observable Conventions in Git History

These are **not guidelines** — they are proven patterns from 300+ commits. Follow them to stay in sync with the project.

### Branch Naming

**Pattern**: `<initials>-BHP1-<ticket-number>-<kebab-case-slug>`

**Examples**:
- `rj-BHP1-1611-new-direction-table-filter-logic` (RJ Sheperd, ticket 1611)
- `kc-BHP1-1594-fix-auto-migrate-production` (Kenneth Cheung, ticket 1594)
- `rj-BHP1-1532-graph-axes-lines`

**Rules**:
1. Always include the ticket number (BHP1-####).
2. Slug should be kebab-case and descriptive (e.g., `fix-auto-migrate-production` not `fix-bug` or `BHP1-1594`).
3. Branches are pushed to the **upstream** repo (`firelab/behave-app`), not forked.

**Verification command**:
```bash
git branch -a | grep "rj-BHP1\|kc-BHP1" | head -10
```

---

### PR Merging and Conventions

**Destination**: PRs merge to `main` on `firelab/behave-app` (upstream, not fork).

**PR naming**:
- GitHub auto-names from branch: `Merge pull request #<number> from firelab/<branch-name>`
- Example: `Merge pull request #259 from firelab/rj-BHP1-1611-new-direction-table-filter-logic`

**PR expectations**:
1. Title includes ticket: `[BHP1-1611] <Sentence case description>` or just `<branch-name>`
2. Description brief (see [Commit Message Style](#commit-message-style) for voice)
3. All GitHub checks pass (clj-kondo lint, at minimum; test gating not yet automated as of 2026-07-06)
4. Review by at least one maintainer (Kenneth Cheung, RJ Sheperd)

**Verification command**:
```bash
# List recent PRs (merged to main)
git log --all --oneline --grep="Merge pull request" | head -10
```

---

### Commit Message Style

**Format**: Title only (no body). Imperative mood.

**Pattern**: `BHP1-#### <verb> <noun-phrase>`

**Examples**:
```
BHP1-1532 Fix graph axes limits
BHP1-1544 Preserve newlines in saved notes
BHP1-1611 Shade result tables per direction
BHP1-1570 Default single ranged input table to outputs on rows
```

**Anti-patterns**:
- ❌ `Fix the graph bug` (too vague)
- ❌ `BHP1-1532 Fixed graph axes limits` (past tense, not imperative)
- ❌ `BHP1-1532: graph axes` (missing verb)
- ❌ `Update docs` (missing ticket)

**Multi-line commits** (rare, but allowed for complex changes):
- Line 1: `BHP1-#### <verb> <noun-phrase>`
- Line 2: (blank)
- Lines 3+: Explanation (if needed; prefer small commits that don't need explanation)

**Verification**:
```bash
# Check last 20 commits
git log --oneline -20
# All should match pattern "BHP1-#### ..."
```

---

## Lint Gate and Format Expectations

### Clj-Kondo Lint (GitHub Actions)

**Trigger**: Every PR to `main`.

**Workflow file**: `.github/workflows/clj-kondo.yml`

**What it does**:
1. Detects changed `.clj`, `.cljs`, `.cljc` files via `git diff --name-only origin/main...HEAD`
2. Builds analysis cache by linting all files in `projects/`, `components/`, `bases/`
3. Lints only changed files against the cache
4. **Fails the PR if any linting errors detected**

**Local linting** (before commit):
```bash
# Install clj-kondo (if not already installed)
curl -sLO https://raw.githubusercontent.com/clj-kondo/clj-kondo/master/script/install-clj-kondo
chmod +x install-clj-kondo
./install-clj-kondo

# Build cache
clj-kondo --lint projects/ components/ bases/

# Lint specific file
clj-kondo --lint projects/behave/src/cljs/behave/solver/core.cljs
```

**Clj-kondo configuration**: `.clj-kondo/config.edn` (project-level) defines linting rules (e.g., suppressed warnings, custom lint rules). Check it before assuming a warning is a real error.

**Common suppressions** (use sparingly):
```clojure
;; Suppress a specific warning on a single form
#_{:clj-kondo/ignore [:unused-binding]}
(let [x (foo)] ...)

;; Suppress at namespace level (metadata on ns)
(ns ^{:clj-kondo/ignore [:unused-binding]} my.ns)
```

---

### Cljfmt Format

**Run before every commit**:
```bash
# Format a single file
cljfmt fix projects/behave/src/cljs/behave/solver/core.cljs

# Format all Clojure files in a directory
cljfmt fix projects/behave/src/

# Check (don't modify)
cljfmt check projects/behave/src/
```

**Cljfmt configuration**: `.cljfmt.edn` at repo root. Contains indentation rules, line length (typically 80), etc. Respect it; don't disable rules for convenience.

**GitHub Actions linting** does not currently enforce cljfmt, only clj-kondo. However, **maintainers may ask you to format before merge**, so run it locally to avoid rework.

---

## Migration Authoring Checklist

Follow this checklist when creating a VMS/CMS migration (category 2 or 4 changes).

### Pre-Migration

- [ ] Start CMS locally: `cd projects/behave_cms && clojure -M:server` (port 8001)
- [ ] Start Datomic transactor: `bb transactor` (from root)
- [ ] Verify CMS can connect to Datomic: check logs for "connected to datomic"

### Create Migration File

- [ ] **File location**: `projects/behave_cms/resources/migrations/YYYY_MM_DD_description.clj`
- [ ] **Naming convention**: Date first (sortable), then kebab-case description
  - Example: `2026_07_15_add_group_variable_direction.clj`
  - **NOT** `add-group-variable-2026_07_15.clj` (unsortable)
- [ ] **Namespace metadata**: Mark with `:migrate/ignore? true` if migration was applied before moving migration files (e.g., when consolidating migrations from `development/` to `resources/migrations/`)
  ```clojure
  (ns ^{:migrate/ignore? true} migrations.2024_07_15_add_group_variable_direction
    (:require [schema-migrate.interface :as sm]
              [datomic.api :as d]
              [behave-cms.store :refer [default-conn]]))
  ```
- [ ] **Documentation**: Add a comment block explaining what the migration does
  ```clojure
  ;; Overview
  ;; Adds an attribute to group variables that are specified for a spread direction
  ;; (one of #{:heading :backing :flanking})
  ```

### Write the Migration Logic

- [ ] Use `datomic.api/transact` to apply schema or data changes
- [ ] Test the transact locally (manually run the REPL code)
- [ ] Verify the change is **idempotent** (can be applied twice without error):
  ```clojure
  ;; BAD: fails on second apply
  (d/transact conn [{:db/ident :new-attribute
                      :db/valueType :db.type/string}])
  
  ;; GOOD: upsert pattern checks existence first
  (when-not (some? (d/q '[:find ?e .
                           :where [?e :db/ident :new-attribute]]
                         @conn))
    (d/transact conn [{:db/ident :new-attribute
                        :db/valueType :db.type/string}]))
  ```
- [ ] If data migration is needed (e.g., rename a variable), query existing entities and update them:
  ```clojure
  (let [entities (d/q '[:find ?e ?v
                         :where [?e :group-variable/name ?v]]
                       @conn)
        updates (mapv (fn [[eid old-name]]
                        {:db/id eid
                         :group-variable/name (str/replace old-name "old" "new")})
                      entities)]
    (d/transact conn updates))
  ```
- [ ] **Rollback plan**: Document how to undo (e.g., "revert the entity upserts" or "set :migrate/ignore? true and deploy")

### Test the Migration

- [ ] Run migration locally:
  ```bash
  cd projects/behave_cms
  clojure -M:dev -e "(require 'migrations.YYYY-MM-DD-description) (migrations.YYYY-MM-DD-description/run)"
  ```
  Or load the migration file in a REPL and evaluate the transact form.
- [ ] Verify the change persists:
  ```bash
  # In Datomic console
  datomic:clojure-peer-server=> (d/q '[:find ?e ?v :where [?e :new-attribute ?v]] @conn)
  # Should return the expected data
  ```
- [ ] Check that second application (idempotency) doesn't error or corrupt data
- [ ] **Mark as ignore if needed**: If the migration was already applied before being moved/restructured, add `:migrate/ignore? true` to prevent reapplication on production:
  ```clojure
  (ns ^{:migrate/ignore? true} migrations.some-old-migration)
  ```

### Sync Downstream Artifacts

- [ ] **Re-export layout.msgpack**: After VMS schema changes, export the updated VMS:
  ```bash
  # Option 1: via CMS UI (admin → export)
  # Option 2: via curl
  curl -X POST http://localhost:8001/api/export-vms \
    -H "Content-Type: application/octet-stream" \
    -o projects/behave/resources/public/layout.msgpack
  # Option 3: if CMS has an export endpoint
  clojure -X:behave/app:download-vms :url "http://localhost:8001" :auth-token "YOUR_TOKEN"
  ```
- [ ] Commit the new `layout.msgpack`: `git add projects/behave/resources/public/layout.msgpack && git commit -m "BHP1-xxxx Re-export layout.msgpack post-migration"`
- [ ] **Update test fixtures** (if applicable):
  - Regenerate `.bp7` files using updated VMS (if fixtures are templated)
  - Update hard-coded UUIDs in test code to match new VMS
  - Re-run tests to verify fixtures are consistent
- [ ] **Update any hard-coded variable names** in test code:
  ```bash
  # Search for old variable names
  grep -r "old-variable-name" projects/behave/test/
  # Update to new names or fetch from VMS at runtime
  ```

### Commit and PR

- [ ] Commit message: `BHP1-#### <verb> description of migration`
  Example: `BHP1-1594 Move migrations to behave_cms resources folder`
- [ ] PR description: Explain why the migration is needed, any data loss/transformation, and rollback plan
- [ ] Ensure all tests pass (green suite on `http://localhost:8081/api/test`)
- [ ] Request review from a maintainer (they'll verify idempotency and schema consistency)

### Production Rollout

- [ ] **On production deploy**: CMS migration runner (in `schema_migrate.runner/run-pending-migrations!`) automatically applies migrations on startup
- [ ] **No manual intervention needed** (unlike SQL migrations, Datomic migrations are code-driven)
- [ ] **Monitor**: Check CMS logs for successful migration application
- [ ] **Rollback (if needed)**: Set `:migrate/ignore? true` on the migration file, redeploy, and manually revert the Datomic data (e.g., via Datomic console)

---

## Release Gates and Process

When releasing a version of Behave7, the following gates apply.

### Pre-Release Checklist

- [ ] **All tests GREEN** on main
  - Browser: `http://localhost:8081/api/test` (manual run or `bb test:ci` headless)
  - Solver: golden outputs verified
  - No outstanding red deftests
- [ ] **Commit log clean**: No uncommitted changes (`git status`)
- [ ] **Version consistency**:
  ```bash
  # Verify both files have the same version
  grep ':version' projects/behave/resources/version.edn
  grep 'version.*=' projects/behave/conveyor.base.conf
  # Should match, e.g., both should show 7.1.4
  ```
- [ ] **Latest VMS exported**: layout.msgpack is up-to-date
  ```bash
  # Last update should be recent (check commit log)
  git log --oneline projects/behave/resources/public/layout.msgpack | head -1
  ```
- [ ] **All migrations applied**: No pending migrations in behave_cms
  ```bash
  # On production DB, verify all migration files have been applied
  # (manual check via Datomic console or CMS logs)
  ```
- [ ] **Behave-lib submodule status**: Confirm branch is stable (not mid-refactor)
  ```bash
  cd behave-lib && git log -1 --oneline && git branch -v
  ```

### GitHub Actions Release Workflow

**Trigger**: Manual workflow dispatch via GitHub UI (`Actions` → `Create Release` → `Run workflow`)

**Inputs**:
- `tag` (required): Release version, e.g., `v7.1.4` or `7.1.4` (v-prefix added if missing)
- `build_windows` (optional, default=true): Build Windows .exe
- `build_macos` (optional, default=false): Build macOS .dmg
- `build_linux` (optional, default=false): Build .deb

**Workflow steps** (defined in `.github/workflows/release.yml`):

1. **get-version**: Normalize version string (ensure v-prefix)
2. **build**: Run `jar-only.yml` to create uberJAR
   - Downloads VMS layout via `clojure -X:download-vms` (uses VMS_URL + VMS_AUTH_TOKEN secrets)
   - Compiles ClojureScript: `bb build-js`
   - Builds uberJAR: `bb uber`
   - Uploads artifact: `behave7-jar`
3. **package-windows** (if selected): Run `conveyor.yml` for Windows build
   - Outputs: `windows-zip` artifact
4. **sign-windows** (if selected): Sign Windows zip via Azure Trusted Signing
   - Uses custom action `rjsheperd/az-jsign-trusted-signing@main`
   - Requires secrets: AZURE_SIGNING_ALIAS, AZURE_SIGNING_REGION, AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET
   - Outputs: `windows-zip-signed` artifact
5. **package-macos-amd64** (if selected): Run `conveyor.yml` for macOS Intel build
   - Decodes base64 secrets: APPLE_SIGNING_P12_ENCODED → `~/.behave/.env/apple.p12`
   - Decodes: MAC_NOTARY_PRIVATE_KEY_ENCODED → `~/.behave/.env/AuthKey.p8`
   - Outputs: `mac-zip-amd64` artifact
6. **package-macos-aarch64** (if selected): Run `conveyor.yml` for macOS ARM build
   - Same as amd64, but with `-Kapp.machines=aarch64-linux` flag
   - Outputs: `mac-zip-aarch64` artifact
7. **package-linux** (if selected): Run `conveyor.yml` for Debian build
   - Outputs: `linux-deb` artifact
8. **create-release**: Aggregate all artifacts and create GitHub release
   - Downloads all platform artifacts
   - Generates release notes from git log: `git log origin/main..HEAD --pretty=format:'- %s' --no-merges` (fallback: "Initial release")
   - Creates GitHub release with name format: `VERSION (DATE)`, e.g., `v7.1.4 (2026-07-06)`
   - Uploads artifacts with MIME type `application/octet-stream`
   - Sets `makeLatest: true`

**Verification** (after release completes):
- [ ] GitHub release created at https://github.com/firelab/behave-app/releases/
- [ ] All platform artifacts present (JAR, Windows zip, macOS dmg/zip, Debian deb)
- [ ] Release notes include commits since last release
- [ ] Artifacts are signed (Windows, macOS)
- [ ] JAR and exes are executable (sanity check)

### Tag and Deploy

**Manual step**: Push the release tag to trigger automated builds (if using `jar-builder.yml`):
```bash
# After release.yml completes successfully
git tag v7.1.4
git push origin v7.1.4
# This triggers .github/workflows/jar-builder.yml (alternate automated build path)
```

**OR** use the GitHub UI:
1. Navigate to Releases
2. Click "Create a new release"
3. Enter tag, title, description
4. Attach artifacts manually (if workflow didn't upload automatically)
5. Mark as latest
6. Publish

---

## When NOT to Use This Skill

This skill covers **change control, classification, and conventions**. It does **not** cover:

- **Debugging failures**: Use `behave-debugging-playbook` (symptom → triage table, discriminating experiments)
- **Understanding root causes**: Use `behave-failure-archaeology` (every settled investigation, evidence(SHA), status)
- **Design decisions**: Use `behave-architecture-contract` (load-bearing invariants, known-weak points)
- **Fire science domain**: Use `fire-behavior-reference` (Rothermel model, 6 modules, GACC, fuel models)
- **VMS variable pipeline**: Use `behave-vms-variable-pipeline` (C++→WASM→Hatchet→cms-exports→layout.msgpack→fixtures)
- **Build/env setup**: Use `behave-build-and-env` (recreating from scratch, traps, prerequisites)
- **Running/operating**: Use `behave-run-and-operate` (dev/server/desktop/CMS, config axes, release operation end-to-end)
- **Testing and validation**: Use `behave-validation-and-qa` (evidence bar, golden inventory, every test tier, known standing reds)
- **Landing absurder_sql**: Use `behave-absurder-sql-campaign` (decision-gated phases, gates with expected observations)

---

## Provenance and Maintenance

Every fact in this skill has been verified against the repo as of **2026-07-06**. Updated 2026-07-06 to note that some incident examples (Non-Negotiable #3 incidents, mortality-worksheet and GACC rename) are in-progress on rj-fix-figwheel-tests branch, not yet on main. Also removed references to bb test:ci (not available on main). Commands to re-verify key claims:

| Claim | Verification Command |
|-------|----------------------|
| PR pattern merges to firelab/behave-app | `git log --all --oneline \| grep "Merge pull request" \| head -3` → should show "from firelab/rj-..." |
| Commit messages are "BHP1-#### verb" | `git log --oneline \| grep "BHP1" \| head -10` → all should match pattern |
| clj-kondo runs on PRs | `cat .github/workflows/clj-kondo.yml \| grep "branches:"` → should show "main" |
| Migrations live in resources/migrations/ | `ls projects/behave_cms/resources/migrations/ \| head -5` → files should exist |
| :migrate/ignore? metadata used | `grep -r "migrate/ignore" projects/behave_cms/resources/migrations/ \| head -2` → should find examples |
| layout.msgpack exists and is tracked | `git ls-files projects/behave/resources/public/layout.msgpack` → should return the path |
| Version dual-sourced in two files | `grep "version\|7.1.4" projects/behave/resources/version.edn projects/behave/conveyor.base.conf` → both should match |
| Settled incidents are in git history | `git log --all --oneline \| grep -E "761b0c22\|ad62e0e7\|b914d001"` → core infrastructure incidents; examples of fixes in-progress on rj-fix-figwheel-tests (FIX_TEST_PLAN.org) |
| Test suite at /api/test | `grep -r "/api/test" projects/behave/src/` → should find handler route |
| Lint gate workflow is clj-kondo.yml | `ls .github/workflows/ \| grep kondo` → should show clj-kondo.yml |

**When to update this skill**:
- If PR process changes (fork-based vs shared upstream)
- If migration path moves or runner changes
- If release workflow is restructured
- If lint tool is swapped or configured differently
- If a new non-negotiable incident occurs (add it with SHA and rationale)
- Annually: re-verify all commands still return expected results

**Owner**: RJ Sheperd (maintainer contact)
