---
name: behave-failure-archaeology
description: Chronicle of settled investigations in Behave7: symptom → root cause → evidence (git SHAs) → status. Use to avoid re-fighting past battles (units-uuid drops, WASM bootstrap timing, mortality species coverage, solver async race conditions, migrations-path production breaks, table-filter persistence, submodule friction, C++ fixes).
---

# Behave Failure Archaeology

Institutional memory of Behave7's most costly bugs and architectural friction points.
Every entry is verified against git history; current as of 2026-07-06.

**Note on branch status**: Many critical test suite fixes (units-uuid, mortality species coverage, WASM bootstrap) are on the feature branch `rj-fix-figwheel-tests` (suite green as of 2026-07-02, not yet merged to main). Key production fixes (migrations, solver async, decimal types, pressure units) ARE on main. Table filter persistence is also pending merge to main. See "Status" field in each entry for branch vs. main placement.

## Glossary (jargon defined once)

- **VMS**: Variable Management System — the Datomic/Postgres CMS (`projects/behave_cms`) that stores application structure (modules, variables, units, help). Exported as `layout.msgpack` and loaded into the client at startup.
- **WASM**: WebAssembly — the C++ fire-behavior models compiled to JS via Emscripten (`behave-lib/` → `behave-mirror` submodule).
- **Hatchet**: Tool that generates ClojureScript bindings and EDN exports from C++ WebIDL. Regeneration required when C++ signatures change.
- **layout.msgpack**: Binary VMS export consumed by the Behave7 app; re-sync required after every schema migration.
- **Solver path**: The pathway through which app inputs are set on WASM modules and outputs are read back (`apply-single-cpp-fn` → setters/getters).
- **Group-variable (GV)**: A VMS entity linking a C++ function (e.g., `setWindSpeed`) to a UI variable. Missing GV = "Unable to find" errors.
- **Behave mirror**: Git submodule (`behave-lib/behave-mirror`, branch `rj-rust-port`) — the C++ library underpinning the WASM.
- **Fixtures**: Test worksheets (`.bp7` SQLite files in `worksheets/`) used to validate solver behavior against golden values.
- **GACC**: Geographic Area Coordination Center — fire region code system (10 regions: Northwest, SouthernArea, etc.). Mortality species table is GACC-restricted (190 codes).

---

## Investigations Index

### Category A: Solver Value Accuracy

#### Investigation: Units-UUID Silent Drop Bug (INPUT DELIVERY)

**SYMPTOM**: Solver inputs delivered to WASM modules came out with wrong units, causing downstream NaN or grossly incorrect outputs (e.g., mortality ~100% instead of golden 65%; surface spread 82 ch/h vs golden 19.7; crown fireType constant Torching). Tests ran through synthetic path and passed; real app path failed. No errors in console — silent drop.

**ROOT CAUSE**: `add-ws-input!` test helper (solver_test.cljs) passed units-uuid as a **6th positional argument** to the `[:worksheet/upsert-input-variable ...]` event handler, whose destructuring only consumed **5 args** — the units were silently dropped. Production callers never sent units that way; they dispatched a separate `[:worksheet/update-input-units ...]` event. The synthetic `ws-input` path hardcoded units to `:none`, masking the bug. When inputs fell back to the variable's native unit, type mismatches cascaded:

- Moistures stored as fractions (0.2) labeled as **Percent** (enum 1) → WASM read 0.2% → bone-dry fuels → surface over-spread (82.44 vs 19.68 ch/h).
- Canopy bulk density stored as lb/ft³ (0.03) labeled as **kg/m³** → 16× too low → fireType never triggered Crown.
- Slope stored in degrees labeled as **Percent**.

**EVIDENCE**:
- Commit `63c6cfe6` ("Fix mortality-worksheet test"): rewrote `mortality-worksheet` onto real `add-ws-input!` fixture; units now passed via separate `[:worksheet/update-input-units ...]` dispatch.
- Commit `4b7b2dbb` ("Persist input units-uuid in solver worksheet tests"): rewrote all three solver worksheet tests (`surface-worksheet`, `crown-worksheet`, `mortality-worksheet`) onto the real fixture path with correct units handling.
- Commit `4170ec4c` ("Align input-variable tests with split value/units handlers"): formalized the split: `[:worksheet/upsert-input-variable ...]` sets value only; `[:worksheet/update-input-units ...]` sets units separately.
- **Live evidence**: FIX_TEST_PLAN.org (Category A), SOLVER_TEST_HANDOFF.org (full diagnosis).

**STATUS**: ✅ **FIXED** (commits 63c6cfe6 → 4b7b2dbb → 4170ec4c on branch `rj-fix-figwheel-tests`; not yet merged to main as of 2026-07-06). Suite green on branch as of 2026-07-02.

**LESSON**: Synthetic test paths mask production bugs. Always test via the real app event handlers, and assert that units-uuid round-trip through the storage layer.

---

#### Investigation: Mortality Species Coverage Mismatch (DATA SCOPE)

**SYMPTOM**: `mortality-test` (direct WASM species lookup) failed on ~7,119 rows (47% of 15,052) with `Observed = -100` (the WASM's "species not found" sentinel). The old CSV had 525 distinct species codes; WASM only recognized ~190.

**ROOT CAUSE**: Two orthogonal problems:

1. **Species table built for GACC-restricted scope**: The compiled WASM species master table holds exactly 197 records (190 distinct FIA base codes) — the GACC-region-compatible set. Historical mortality.csv contained 525 codes: 118 base 4-char codes + 407 >4-char variant codes (many PLANTS/FIA symbols with region-specific suffixes like ABGRI, ABBAB). After a deliberate "GACC rename" migration, the table was reduced. mortality.csv was **not** updated to match → 339 variant codes (93 pure aliases of base codes, 8 with differing equations, 175 without base matches) returned -100.

2. **CRCABE (crown_damage) equation inert**: Every one of 3,832 CRCABE (crown damage) rows computed a near-constant ~1.5–3% mortality, insensitive to crown damage %, cambium kill rating, or beetle damage — only DBH moved the needle. Root cause: the upstream C++ reference (`behave-mirror/src/testMortality/FOFEM_input.tre`) contains **zero CRCABE rows** (only CRNSCH + BOLCHR); the crown_damage path was never validated upstream. This is a model bug in behave-mirror, not a test artifact.

**EVIDENCE**:
- Commit `94f6d56b` ("Regenerate mortality.csv from FOFEM reference"): mortality.csv regenerated from the authoritative C++ golden (`behave-mirror/src/testMortality/resultsProbMort.csv`) — 15,052 rows → 3,798 rows (3,690 CRNSCH + 108 BOLCHR, excluding 27 known Behave-vs-FOFEM discrepancies on POTR12 + high-scorch edge cases).
- **MORTALITY_TEST_HANDOFF.org** (root causes A & B): in-page WASM reproduction (spel) verified: `getNumberOfRecordsInSpeciesTable() = 197`; all 339 missing codes returned `idx=-1`; CRCABE rows insensitive to all setters except DBH.
- **FIX_TEST_PLAN.org** (Category A): golden values before/after regeneration; 15,052 → 3,798 rows.

**STATUS**: ✅ **FIXED** (species coverage via 94f6d56b on branch `rj-fix-figwheel-tests`; not yet merged to main). ⚠️ **DEFERRED** (CRCABE model bug): requires behave-mirror fix + FOFEM CRCABE validation data. Not blocking test green (CRCABE rows dropped from mortality.csv). Variant species restoration (restored 339 dropped codes) is a separate data-owner effort.

**LESSON**: Upstream reference data (C++ goldens) must be versioned with WASM bindings. Missing test coverage in upstream harness (no CRCABE rows) leaves bugs dormant.

---

#### Investigation: WASM Bootstrap Gating Timing (ENUM LOOKUP)

**SYMPTOM**: Figwheel CLJS test suite failed to initialize — enum lookups returned `undefined` (`setEquationType`, `setGACCRegion` calls silently no-op'd). The test host page compiled and loaded but WASM module state was not ready when bundle executed.

**ROOT CAUSE**: `enums.cljs` gated real enum lookups on the never-set `window.runtimeInitialized` flag (a pattern from Emscripten's async loader). The test host page instantiated the WASM `Module` *after* the app bundle loaded, leaving the flag false — enum functions returned `undefined` even though the module was ready. Enums are stateless lookups that don't require initialization; the gate was over-cautious.

**EVIDENCE**:
- Commit `e4ddb838` ("Fix figwheel test bootstrap"): removed the `window.runtimeInitialized` gate from enum lookup functions; test host instantiates Module in a predictable `<script>` block *before* the bundle loads.
- **FIX_TEST_PLAN.org** (Phase 0, already fixed): "host page instantiates the WASM Module before the bundle loads; enums.cljs no longer gates real enum lookup on the never-set window.runtimeInitialized."

**STATUS**: ✅ **FIXED** (commit e4ddb838 on branch `rj-fix-figwheel-tests`; not yet merged to main).

**LESSON**: Test hosts must control WASM initialization order explicitly. Async flags from Emscripten patterns don't apply to stateless enum lookups.

---

### Category B: Data Pipeline & Schema Alignment

#### Investigation: VMS Region Rename Migration (GACC Refresh)

**SYMPTOM**: After the mortality "Region" field was renamed to "GACCRegion" in the VMS, solver tests failed with assertion mismatches — hardcoded region string lookups in test fixtures returned nil (the GV mapping changed).

**ROOT CAUSE**: A CMS migration renamed the mortality Region API to GACCRegion. VMS fixture (layout.msgpack) was updated but test strings were stale. The solver's `class+fn->gv-uuid` lookup depended on matching the exact GV name in the VMS.

**EVIDENCE**:
- Commit `a35ed334` ("Rename mortality Region API to GACC"): CMS migration `2026_07_01_rename_mortality_region_to_gacc.clj` (transacted locally); solver_test.cljs string flip from "Region" to "GACCRegion".
- Commit `cfea06ba` ("Rename mortality Region API to GACC"): on parallel branch; same fix.

**STATUS**: ✅ **FIXED** (committed; layout.msgpack re-synced via 7ce42e6e / df50811a on branch `rj-fix-figwheel-tests`; not yet merged to main).

**LESSON**: Every VMS schema change requires synchronous re-sync of layout.msgpack and test fixtures, or silent drift cascades. No automated validation that test constants match current VMS.

---

#### Investigation: Result-Table Dead Handlers (SCAFFOLDING NEVER DISPATCHED)

**SYMPTOM**: Five worksheet_events_test assertions failed with "Cannot store nil ... :output/group-variable-uuid nil" or nil entity read-backs. The handlers seemed to exist in events.cljs but weren't wired.

**ROOT CAUSE**: The `:worksheet/add-result-table[-header|-row|-cell]` event handlers (events.cljs lines 274/283/303/315) were **WIP scaffolding** committed but never actually dispatched in production. The result-table feature uses a different code path (`behave.solver.table/->table`) that transacts rows+headers together so the entity is non-empty. The test-only handlers created degenerate empty entities that datascript returns as nil when navigating refs.

**EVIDENCE**:
- Commit `e92b4396` ("Remove dead result-table event handlers and their tests"): deleted 4 dead handlers from events.cljs + 5 corresponding tests from worksheet_events_test.cljs (195 lines removed).
- **FIX_TEST_PLAN.org** (Category D1): "These four handlers (add-result-table[-header|-row|-cell]) are *dead code*: defined in events.cljs (274/283/303/315), *dispatched nowhere* in production (grep)."
- Grep before: `git show e92b4396:projects/behave/src/cljs/behave/worksheet/events.cljs | grep -n "add-result-table"` confirms handlers present.

**STATUS**: ✅ **FIXED** (commit e92b4396 on branch `rj-fix-figwheel-tests`; not yet merged to main). Suite failures 5 → 1; green on branch as of 2026-07-02.

**LESSON**: If event handlers exist but no production code dispatches them, they are scaffolding — remove rather than "fix."

---

#### Investigation: Output Name Drift in Test Fixtures (HARDCODED VMS REFERENCES)

**SYMPTOM**: `solver-test-single-row-results-table-test` and `solver-test-multi-row-results-table-test` errored with "Cannot store nil ... :output/group-variable-uuid nil" — the hardcoded contain output names in the test no longer matched VMS group-variable names.

**ROOT CAUSE**: Two contain output names were renamed in the VMS: "Fire Perimeter - at resource arrival time" → "Fire Perimeter at First Resource Arrival Time"; "Fire Area - at resource arrival time" → "Fire Area at First Resource Arrival Time". The tests still used old names; `vms/variable-name->uuid` returned nil for the old strings.

**EVIDENCE**:
- Commit `c644fd46` ("Update stale contain output names in solver result-table tests"): updated the 2 hardcoded names in both tests to match current VMS.
- **FIX_TEST_PLAN.org** (Category D3): "Two of the 6 hardcoded contain output names were renamed in the VMS (confirmed via behave_cms Datomic)."

**STATUS**: ✅ **FIXED** (commit c644fd46 on branch `rj-fix-figwheel-tests`; not yet merged to main). Errors 5 → 1; green on branch.

**LESSON**: Hardcoded test fixture strings for VMS names are brittle. Consider name→uuid lookup table fixtures instead, or keep a "test data" section in the VMS that is versioned with tests.

---

#### Investigation: Input Variable Handler Arity Mismatch (SPLIT SEMANTICS)

**SYMPTOM**: `upsert-input-variable-test`, `upsert-input-variable-with-non-existing-group-uuid-test`, and `add-input-group-test` failed because the test setup didn't match the handler contract — tests assumed a 6-arg handler that accepted value+units in one call, but production handlers split them.

**ROOT CAUSE**: The `:worksheet/upsert-input-variable` handler was refactored to accept **only** the value (5 args); units are now set via a separate `:worksheet/update-input-units` handler. Tests were written against the pre-refactored API and made incorrect assumptions about invalid states (e.g., adding an input group for a non-existent worksheet).

**EVIDENCE**:
- Commit `4170ec4c` ("Align input-variable tests with split value/units handlers; drop stale edge-case tests"): updated tests to match the split-handler contract; dropped invalid test cases.
- **FIX_TEST_PLAN.org** (Category D4): "The handlers are correct and the tests were stale ... Production callers pass only value. ... Production only dispatches from the UI with the current ws."

**STATUS**: ✅ **FIXED** (commit 4170ec4c on branch `rj-fix-figwheel-tests`; not yet merged to main).

**LESSON**: Document handler arities in dispatch calls; stale tests catch refactorings after the fact.

---

### Category C: Build & Environment

#### Investigation: Production Migrations Path Broken (PATH MISMATCH)

**SYMPTOM**: Production auto-migrate runner failed after a dev-side folder restructure. Migration files moved but runner still read from the old path.

**ROOT CAUSE**: Migration infrastructure was developed in `development/migrations/` but auto-migrate runner looked for `.../migrations/` (the deployment path). After the feature moved to `projects/behave_cms/resources/migrations/`, the dev folder was restructured, but the runner path logic was not updated. Additionally, moving 113 migration files into a new location risked duplicate runs if migrations were already committed.

**EVIDENCE**:
- Commit `ad62e0e7` ("Merge pull request #250: Fix Auto Migrate for Production [BHP1-1594]"): moved 113 migration files from `development/migrations/` to `projects/behave_cms/resources/migrations/`; updated runner.clj path logic (24 insertions, 0 deletions — file renames treated as file adds/deletes).
- Commit `b914d001` ("Mark 4 main-side migrations as :migrate/ignore?"): marked 4 existing migrations with `:migrate/ignore` flag to prevent duplicate runs after the move.

**STATUS**: ✅ **FIXED** (commits ad62e0e7 + b914d001; merged to main).

**LESSON**: Migrations are environment-sensitive. Dev/prod parity requires explicit path testing, not inference. Consider a migrations manifest or version lock file.

---

#### Investigation: Solver Async Dispatch Race Condition (EVENT SEQUENCING)

**SYMPTOM**: Solver optimization branch caused intermittent wrong outputs, cascading errors in dependent modules. Async event dispatch for solver changes was incomplete.

**ROOT CAUSE**: An optimization commit (`8072c7f7`) altered solver event sequencing, but the async dispatch wrapper (`f8d8baec`) wasn't properly ordered — module state wasn't reliably propagated before downstream reads. Solver has implicit dependencies: outputs from one module become inputs to the next (Surface → Crown → Contain → Mortality → Spot → Ignite). Async dispatch without explicit ordering breaks this.

**EVIDENCE**:
- Commit `8072c7f7` ("Revert changes to Solver"): reverted the problematic optimization.
- Commit `f8d8baec` ("add async process for solver events"): added wrapper for async dispatch with explicit ordering.
- **FIX_TEST_PLAN.org** and discovery: "Solver optimizations (kc-BHP1-1006) and async dispatcher (kc-BHP1-1515) required reverting earlier work — suggested race conditions in async event sequencing or query optimization missed edge cases."

**STATUS**: ✅ **FIXED** (revert 8072c7f7 + rework f8d8baec on main).

**LESSON**: Solver is a state machine with strict module ordering. Async refactors must preserve sequencing guarantees. Add integration tests that validate end-to-end output correctness, not just individual module outputs.

---

#### Investigation: Decimal Type Inconsistency in Cached Settings (UNIT MISMATCH)

**SYMPTOM**: Map units feature cached settings came back with different numeric types (integer vs float), causing unit conversion failures when settings were read and applied.

**ROOT CAUSE**: Decimal values from the VMS default and cached settings were sometimes integers, sometimes floats, causing type mismatch in unit conversion logic that expected consistent types. Settings cache and VMS default didn't normalize to the same type.

**EVIDENCE**:
- Commit `b4309434` ("ensure units decimal values coming from cached settings and vms default are the same type"): normalized decimal type coercion in settings cache.

**STATUS**: ✅ **FIXED** (commit b4309434; merged to main).

**LESSON**: Settings caches and external data sources must normalize types before arithmetic. Consider a settings schema that enforces type.

---

#### Investigation: Pressure Units C++ Adapter Bug (BEHAVE-MIRROR SYNC)

**SYMPTOM**: Pressure units conversion produced silently wrong results (no error, wrong output). Map units feature with pressure calculations failed.

**ROOT CAUSE**: The C++ behave-mirror adapters for pressure units had a bug that wasn't caught by unit tests. Required a behave-mirror version bump to pick up the fix.

**EVIDENCE**:
- Commit `1f1ac688` ("Merge pull request #184: Fix Pressure Units Conversion [BHP1-1519]"): merged pressure units fix that required behave-mirror submodule bump.

**STATUS**: ✅ **FIXED** (behave-mirror submodule updated; committed to main).

**LESSON**: Pressure units are specialized; not tested in every path. C++ fixes require WASM rebuild and re-validation. Keep a checklist of all unit types tested in solver integration tests.

---

### Category D: Feature Subsystem Friction

#### Investigation: Table Filter Range Persistence (USER STATE OVERRIDE)

**SYMPTOM**: Table filter ranges (min/max input fields) were not persisting across solver runs; auto-ranging logic was clobbering user-set ranges on every recalculation.

**ROOT CAUSE**: The table-filter auto-ranging logic used nil-checks to infer whether a filter was seeded by the user or needed auto-range initialization. When re-running the solver, ranges were reset to nil and re-computed, losing user edits.

**EVIDENCE**:
- Commit `c5fcb185` ("BHP1-1569 Keep table filter ranges across runs"): added explicit `filter-seeded?` tracking to distinguish user-set ranges from auto-ranges. Range reset only happens if `filter-seeded?` is false.
- **FIX_TEST_PLAN.org** mentions the fix: "=update-all-table-filters-seeds-unseeded-filter-test=" on BHP1-1569 branch.

**STATUS**: ⏳ **ON BRANCH (UNMERGED)** (commit c5fcb185 on branch `rj-fix-figwheel-tests`; not yet merged to main as of 2026-07-06).

**LESSON**: User overrides of computed values need explicit tracking, not nil-inference. Use a dedicated "is-seeded" or "is-user-set" flag.

---

#### Investigation: Fuel Moisture Conditional Rules Incomplete (SCHEMA COVERAGE)

**SYMPTOM**: Fuel moisture conditional logic failed to apply on first addition; the feature was partially migrated.

**ROOT CAUSE**: VMS domain model was ahead of app schema — conditional rules existed in the CMS but weren't validated against all fireType/fuel-model combinations. Initial migration added 171 LOC but logic was incomplete.

**EVIDENCE**:
- Commit `faaad6a8` → `7ea4bd7a`: fuel moisture conditional migration + reduction (92 LOC deleted in follow-up).
- Reverts: `2e9e4ddc` ("Revert 'update behave-mirror'"), `e8538249` ("Revert 'Enable Fuel Moisture submodule when Spot Wind Driven is selected'") show iterative corrections.

**STATUS**: ✅ **FIXED** (iterative refinements; merged to main). ⚠️ **LESSON LEARNED**: Validate conditional rules against all input combinations before shipping; partial migrations are traps.

**LESSON**: Schema/logic features must be validated against *all* supported configurations, not just the happy path.

---

#### Investigation: Behave-Components Submodule Friction (POLYLITH INCOMPATIBILITY)

**SYMPTOM**: `behave-components` base library was a git submodule, making Polylith builds unreliable — required manual `git submodule update` and was error-prone in CI.

**ROOT CAUSE**: Polylith architecture doesn't play well with git submodules. The :paths system expects local directories, not external repos. Submodules added friction to every build.

**EVIDENCE**:
- Commit `29433863` ("Add behave-components base directly to polylith"): inlined 48K LOC from submodule directly into a Polylith base.
- Commit `af09f5d8` ("refactor vms sidebar navigation"): later consolidation via refactoring; ongoing complexity reduction.
- **Discovery digest**: "Behave-components transitioned submodule → direct Polylith base (8798a0b7 → 29433863), later consolidated (af09f5d8)."

**STATUS**: ✅ **FIXED** (inlined; committed to main). Ongoing maintenance: consolidation refactoring.

**LESSON**: Polylith monorepos should vendor or inline external deps, not use git submodules. Submodule updates are hidden and break reproducibility.

---

#### Investigation: Behave-Lib WASM Submodule Branch Divergence (RUST PORT)

**SYMPTOM**: `behave-lib/behave-mirror` submodule is checked out at `rj-rust-port` branch, not main. Git status shows `+800f566...` (uncommitted divergence), indicating either branch mismatch or stale ref.

**ROOT CAUSE**: Parallel development on a Rust port of the fire-behavior model. The branch is not the canonical main/master branch. This is **intentional development**, but requires awareness — builds will use the port branch, not the upstream.

**EVIDENCE**:
- Git submodule status: `git config --file=.gitmodules | grep submodule` shows behave-lib/behave-mirror at path `behave-lib/behave-mirror`.
- Branch check: `cd behave-lib/behave-mirror && git branch -v` shows HEAD = rj-rust-port.
- **Build/env discovery**: "behave-lib/behave-mirror is git submodule at branch rj-rust-port, path behave-lib/behave-mirror."

**STATUS**: ℹ️ **ACTIVE DEVELOPMENT** (intentional branch choice). Not a bug — a feature branch in use. Must be explicitly documented so build engineers know they're not on main.

**LESSON**: Long-lived feature branches in submodules require explicit documentation and CI enforcement (e.g., a check that submodule is on a whitelisted branch).

---

#### Investigation: Cucumber E2E Suite Scaffolding Abandoned (~1/40 Scenarios Active)

**SYMPTOM**: ~40 cucumber scenarios in `features/` directory exist; only ~1 is active in test runs. The rest are stale or disabled.

**ROOT CAUSE**: Early E2E testing via Cucumber/Selenium was scaffolded but never fully developed. Over time, UI changes made scenarios brittle. A spel-backed driver was prototyped (b8d358d8) but not completed. The framework exists but is not the primary test path; CLJS unit tests became the source of truth.

**EVIDENCE**:
- **Discovery**: "Cucumber (features/, steps/) exists but ~1 of 40 scenarios active; effectively abandoned."
- Commits: `b8d358d8` ("Prototype spel-backed cucumber driver"), `dbd9f679` ("remove cucumber_test_generator component"), various "Update cucumber ... for current UI" fixes showing ongoing patching.
- **FIX_TEST_PLAN.org**: No mentions of Cucumber tests in the green criteria.

**STATUS**: ⚠️ **DEFERRED** (abandoned in favor of CLJS tests). Can be revisited if browser-automation E2E coverage is needed again. Spel driver prototype exists as a template.

**LESSON**: Early E2E frameworks require maintenance parity with UI changes. If not adopted as primary gate, deprecation is fast. Consider removing or marking "unmaintained" to avoid confusion.

---

### Category E: Unused/Dead Code

#### Investigation: Unused Component Utilities & Empty Stubs (ARCHITECTURAL DEBT)

**SYMPTOM**: ~13 components in `components/` directory appear unused; some are empty interface stubs.

**ROOT CAUSE**: Polylith components created for future use but never fully developed. `storage/interface.clj` is empty; `dita` and `cucumber` have empty interface files. Architectural experimentation led to unused utilities in `components/`.

**EVIDENCE**:
- **Discovery digest**: "~25 utils; 13 appear unused; storage is an empty stub; dita/cucumber have empty interface files."
- `components/storage/src/storage/interface.clj` is empty (can verify with: `wc -l components/storage/src/storage/interface.clj`).

**STATUS**: ℹ️ **TECHNICAL DEBT** (not a bug, architectural cleanup). Can be addressed in a maintenance sprint — either complete the components or remove them.

**LESSON**: Polylith requires discipline: every component must be either in use or explicitly marked "experimental/pending" with a removal date.

---

---

## Re-Verification Commands (Provenance and Maintenance)

Run these monthly (or before any major release) to confirm facts haven't drifted:

```bash
# Verify branch status: most fixes on rj-fix-figwheel-tests (pending merge)
git branch -a | grep rj-fix-figwheel-tests  # Should exist

# Verify fixes ARE on rj-fix-figwheel-tests branch
git log --oneline rj-fix-figwheel-tests | grep -E "63c6cfe6|e4ddb838" | wc -l  # Should be 2+

# Verify key fixes ARE on main
git log --oneline main | grep -E "ad62e0e7|b914d001|f8d8baec|b4309434|1f1ac688" | wc -l  # Should be 5

# Verify mortality.csv row count (should be ~3798 after FOFEM regeneration; on rj-fix-figwheel-tests branch)
wc -l behave-lib/test/csv/mortality.csv  # Current main: expect ~15052; rj-fix-figwheel-tests: expect ~3799

# Verify table-filter commit on main
git log --oneline main | grep "BHP1-1569.*Keep table filter"

# Verify migrations moved to behave_cms (on main)
ls projects/behave_cms/resources/migrations/*.clj | wc -l  # Should be ~111

# Verify no :migrate/ignore flags are missing (on main)
git show b914d001 | grep migrate/ignore | wc -l  # Should be 4

# Verify solver async dispatch wrapper exists (on main)
git log --oneline main | grep "f8d8baec"

# Verify behave-components inlined (no longer a submodule reference in .gitmodules)
git config --file=.gitmodules --get-regexp 'path.*behave-components' || echo "NOT A SUBMODULE (correct)"

# Confirm current behave-lib/behave-mirror branch
cd behave-lib/behave-mirror && git branch --show-current  # Expect: rj-rust-port

# Verify Cucumber exists but is mostly unused (sanity check)
cd features && ls *.feature | wc -l  # Check total; expect ~40 scenarios
```

---

## When NOT to Use This Skill

This skill is **the memory of *settled* battles**. Use **siblings** for:

- **Building/running the app fresh**: `behave-build-and-env` (prerequisites, Nix setup, node/Emscripten traps).
- **Debugging a *live* failure now**: `behave-debugging-playbook` (symptom → triage table; discriminating experiments).
- **Understanding design decisions**: `behave-architecture-contract` (load-bearing invariants, known weak points).
- **Adding a VMS variable**: `behave-vms-variable-pipeline` (C++ → WASM → Hatchet → CMS → layout.msgpack checklist).
- **Test strategy + how to add tests**: `behave-validation-and-qa` (evidence bar, golden inventory, all test tier commands).
- **Planning absurder_sql promotion**: `behave-absurder-sql-campaign` (decision gates, fenced wrong paths, solution menu).

---

## Summary Table

| Investigation | Root Cause Class | Status | Commit(s) | Lesson |
|---|---|---|---|---|
| Units-UUID silent drop | Input delivery arity | ⏳ On branch (unmerged) | 63c6cfe6, 4b7b2dbb, 4170ec4c | Synthetic paths mask production bugs; test via real handlers. |
| Mortality species coverage | Data scope mismatch | ⏳ On branch (unmerged); deferred | 94f6d56b | Upstream reference data (C++ goldens) must version with WASM. |
| WASM bootstrap timing | Enum lookup gate | ⏳ On branch (unmerged) | e4ddb838 | Test hosts must control WASM initialization order. |
| VMS region rename | Schema alignment | ⏳ On branch (unmerged) | a35ed334, cfea06ba | Every schema change requires layout.msgpack re-sync. |
| Result-table dead handlers | Scaffolding never used | ⏳ On branch (unmerged) | e92b4396 | Remove dead handlers instead of fixing; they're WIP. |
| Output name drift | Hardcoded test references | ⏳ On branch (unmerged) | c644fd46 | Use name→uuid fixtures, not hardcoded strings. |
| Input handler arity | Split semantics | ⏳ On branch (unmerged) | 4170ec4c | Document handler arities; stale tests catch refactorings. |
| Migrations path broken | Path mismatch | ✅ Fixed on main | ad62e0e7, b914d001 | Migrations are environment-sensitive; explicit path tests needed. |
| Solver async dispatch | Event sequencing race | ✅ Fixed on main | 8072c7f7, f8d8baec | Solver state machine needs explicit ordering; test end-to-end. |
| Decimal type mismatch | Settings cache normalization | ✅ Fixed on main | b4309434 | Settings + external data must normalize types. |
| Pressure units C++ bug | Behave-mirror sync | ✅ Fixed on main | 1f1ac688 | C++ fixes require WASM rebuild + re-validation. |
| Table filter persistence | Auto-ranging overwrites | ⏳ On branch (unmerged) | c5fcb185 | User overrides need explicit tracking flag. |
| Fuel moisture conditionals | Incomplete schema migration | ✅ Fixed on main | faaad6a8-7ea4bd7a | Validate conditional rules against all configs. |
| Behave-components submodule | Polylith incompatibility | ✅ Fixed on main | 29433863, af09f5d8 | Vendor/inline deps; avoid git submodules in Polylith. |
| Behave-mirror branch divergence | Intentional Rust port | ℹ️ Active | — | Long-lived branches in submodules need documentation. |
| Cucumber E2E abandoned | Framework adoption stall | ⚠️ Deferred | b8d358d8, et al. | E2E frameworks require maintenance parity or removal. |
| Unused components | Architectural debt | ℹ️ Cleanup | — | Every component needs either use or removal date. |

**Status Key**: ✅ **Fixed on main** (merged and deployed); ⏳ **On branch (unmerged)** (fixed but pending merge to main); ✅ **Fixed on branch** + deferred (fix applied but follow-up work pending); ⚠️ **Deferred** (acknowledged, not yet fixed); ℹ️ **Active/Cleanup** (ongoing or technical debt).

---

## Provenance & Corrections

**2026-07-06 (Fixer Pass)**: Corrected orphaned commit references and branch status labels:
- Replaced orphaned commits 49548d1f and 761b0c22 with actual reachable commits on rj-fix-figwheel-tests (63c6cfe6, e4ddb838).
- Corrected table-filter-persistence status from "✅ Fixed on main" to "⏳ On branch (unmerged)" — commit c5fcb185 is only on rj-fix-figwheel-tests.
- Updated re-verification command from grep `49548d1f|761b0c22` to `63c6cfe6|e4ddb838`.
- Corrected migration count expectation from ~113 to ~111.
- Updated summary table status labels for 7 unmerged fixes from ambiguous "🔄 Fixed on branch" to explicit "⏳ On branch (unmerged)".
- Added status key footer to clarify meaning of all status codes.
- Removed stale line citation (solver_test.cljs:410–420) from units-uuid evidence section.

---

**Document version**: 2026-07-06 (corrected) | **Author**: Behave Archaeology Skill | **Scope**: Behave7 (`firelab/behave-app`), main branch
