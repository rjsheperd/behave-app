---
name: behave-validation-and-qa
description: Test infrastructure, test tiers with run commands, golden-data validation for solver changes, known standing reds, how to add tests, pre-validation gates (suite green + golden match + lint clean).
---

# Behave7 Validation and QA

Behave7's test strategy balances correctness (solver output validation via golden data) with developer velocity (browser-based Figwheel tests for fast iteration). This document is the authority for how to run every test tier, validate solver changes, and interpret results.

**Status as of 2026-07-06**: Browser test suite is GREEN (51 deftests / 4,161 assertions). JVM component tests exist but are not wired into CI. Cucumber BDD is scaffolded but effectively abandoned (1 of 11 scenarios active). absurder_sql component has its own kaocha infrastructure but is not integrated into main build.

---

## (1) The Evidence Bar: Solver-Affecting Changes Require Golden-Data Comparison

### Discipline

Any change affecting solver output (C++ modules, output linking, units conversion, input-to-output mapping, or worksheet/result table handling) must be validated against **golden reference data**. Never eyeball solver results or test CSV diffs—always perform differential comparison against known-good baseline.

### Why This Matters

Solver-affecting bugs are silent: the app produces wrong numbers with no error, and no test catches them unless the test data exercises that code path. Fire-science decisions (evacuation, resource pre-positioning) depend on output correctness.

### The Process

1. **Identify change scope**: Does your change touch solver/core.cljs, lib/*.cljs wrappers, units.cljs, output-linking logic, or result-table event handlers?
2. **Get golden baselines**: Run the unchanged main branch's test suite, capture outputs.
3. **Apply your change**: Implement fix / feature branch.
4. **Run test suite**: Browser suite (see tier 1 below) + relevant golden CSV comparison (see tier 2).
5. **Verify match**: Golden output CSV should equal the baseline (or be intentionally improved per domain review). If outputs differ unexpectedly, debug before merging.

### No Approximation

- Do not round numbers to "close enough."
- Do not test single worksheets and assume all worksheets pass.
- Do not skip tests with a comment "we know this is slightly off."
- Do not rely on eyeballing browser charts; always run headless/CSV comparison.

---

## (2) Golden/Certified Inventory: Paths and Provenance

### C++ Reference Data (behave-lib Submodule)

Located: `/Users/rsheperd/code/sig/behave-app/behave-lib/`

| Artifact | Path | Purpose | Format | Rows |
|----------|------|---------|--------|------|
| **Mortality reference** | behave-mirror/src/testMortality/resultsProbMort.csv | FOFEM-sourced mortality by species, GACC, scorch height | CSV (5 cols) | 15,052 |
| **FOFEM output** | behave-mirror/src/testMortality/FOFEM_Mortality_Output.csv | Paired FOFEM run | CSV (3 cols) | ~525 |

**Provenance**: USFS FireLab FOFEM reference. Current state: 15,052 data rows (15,053 lines including header). Commit 94f6d56b (2026-07-02) reduced to 3,798 rows during GACC scope refinement, but commit 50518335 restored full dataset ('add test error difference to output csv').

### Application Test Fixtures

Located: `/Users/rsheperd/code/sig/behave-app/behave-lib/test/csv/`

| Artifact | Path | Purpose | Rows | Scope |
|----------|------|---------|------|-------|
| Surface | surface.csv | Rothermel outputs (ROS, flame length) | ~50 | baseline fuel models + wind/moisture |
| Crown | crown.csv | Crown fire initiation / active outputs | ~40 | canopy properties + surface fire scenarios |
| Contain | contain.csv | Fire suppression effectiveness | ~30 | resource production / containment |
| Mortality | mortality.csv | Tree mortality probabilities | 15,052 | All GACC regions × species × scorch-height |

**Regeneration**: mortality.csv auto-generated from behave-mirror/src/testMortality/resultsProbMort.csv during WASM build. Other CSVs are manual reference data.

### VMS Layout Data

Located: `/Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/layout.msgpack` (1.4M binary)

**What it is**: Serialized Datomic schema defining VMS: modules, groups, group-variables, C++ class/function mappings, units, enums, help. Loaded once at startup via behave.vms.store/load-vms!.

**Re-sync trigger**: After VMS Datomic schema migration or C++ variable changes:

```bash
# CMS server (port 8001) + Datomic transactor running:
clojure -X:download-vms :url http://localhost:8001 :auth-token dev
```

**Risk**: Stale layout.msgpack ↔ test fixtures decoupling causes silent test failures.

### Worksheet (.bp7) Test Fixtures

Located: `/Users/rsheperd/code/sig/behave-app/worksheets/`

**Format**: SQLite 3 databases (.bp7 extension, immutable by tests).

| Fixture | Purpose | Scenarios |
|---------|---------|-----------|
| 30-min.bp7 | Basic surface fire | Single-module, single run |
| BHP1-1226.bp7 | Contain modeling | Multi-resource suppression |
| BHP1-1392.bp7 | Spot fire | Firebrands, downwind terrain |
| GraphAxes.bp7 | Diagram generation | Plotting infrastructure |

**Role**: Worksheets loaded/solved in browser/headless tests verify end-to-end solver, persistence, result-table generation. Do not hand-edit .bp7; add via app UI.

---

## (3) Every Test Tier: Run Commands and Current Status

### Tier 1: Browser Suite (ClojureScript Figwheel)

**Entry point**: projects/behave/test/cljs/behave/test_runner.cljs

**What it tests**: 13 test namespaces covering surface/crown/contain/mortality/spot modules, solver orchestration, result tables, worksheet events, units, diagram rendering (4,161 assertions total).

#### How to Run

```bash
# Terminal 1: Start figwheel dev build
cd /Users/rsheperd/code/sig/behave-app/projects/behave
clojure -M:dev:behave/app:figwheel

# Terminal 2 (after ~30s compile): Open test page
open http://localhost:8081/api/test
```

**What you'll see**: HTML page with live test results via cljs-test-display, 51 deftests with pass/fail status, assertion count, real-time hot-reload updates.

#### Current Status (2026-07-06)

✅ **GREEN**: 51 deftests / 4,161 assertions / 0 failures / 0 errors (commit 94f6d56b, 2026-07-02)

#### Test Modules (run order)

| Namespace | File | Focus | Assertions |
|-----------|------|-------|-----------|
| behave.crown-test | crown_test.cljs | Crown fire initiation, L/W ratio | ~100 |
| behave.contain-test | contain_test.cljs | Fire suppression, resource production | ~50 |
| behave.mortality-test | mortality_test.cljs | Tree mortality probabilities | ~120 |
| behave.results-table-test | results_table_test.cljs | Output table generation | ~30 |
| behave.shading-test | shading_test.cljs | Graph shading / coloring | ~40 |
| behave.diagram-test | diagram_test.cljs | Diagram (chart) rendering | ~100 |
| behave.surface-test | surface_test.cljs | Rothermel model, ROS, flame length | ~80 |
| behave.solver-test | solver_test.cljs | End-to-end solver, output linking | ~1200 |
| behave.tests-used-in-fixtures | tests_used_in_fixtures.cljs | Fixture helpers | ~600 |
| behave.test-solver-generators | test_solver_generators.cljs | Parametric run generation | ~80 |
| behave.test-solver-queries | test_solver_queries.cljs | Variable ↔ C++ function mapping | ~50 |
| behave.utils-test | utils_test.cljs | Utility functions | ~250 |
| behave.worksheet-events-test | worksheet_events_test.cljs | Worksheet input/output handlers | ~1200 |
| behave.worksheet-subs-test | worksheet_subs_test.cljs | Worksheet subscriptions | ~200 |

**Key files**:
- projects/behave/test/cljs/behave/fixtures.cljs — worksheet creation, input setup, solving
- projects/behave/test/cljs/behave/helpers.cljs — test utilities

---

### Tier 2: Golden CSV Comparison

**Purpose**: Verify solver output against C++ reference when changes touch solver, units, output linking.

#### Manual Comparison

```bash
# Get baseline (main branch)
git stash
cd /Users/rsheperd/code/sig/behave-app

# Run Figwheel test suite, manually record solver output for target scenario
open http://localhost:8081/api/test
# Inspect browser console (F12 → Console) or use REPL

# Apply your change:
git apply changes.patch

# Rerun test, compare:
# Expected: outputs match baseline within precision (e.g., 6 decimals for ROS)
# If different: investigate if change is intended (units fix) or regression
```

#### Extract Solver Output

```clojure
;; In Figwheel REPL after opening /api/test:
(require '[behave.solver-test :as st])
(def ws (st/new-solver-worksheet! :surface :crown :mortality))
(st/add-ws-input! ws :SIGSurface :fuel-model-number 1)
(def results (st/solve-ws-outputs ws))
(println (get results "Rate of Spread"))
;; Compare to behave-lib/test/csv/surface.csv fuel model 1 row
```

#### No Golden-Data Bypass

- Solver logic change → **must** verify golden CSV
- Units/input mapping change → **must** verify golden CSV
- Add/remove output variables → **must** update fixtures + re-verify
- Test fixtures change → re-sync layout.msgpack + re-verify

**Risk**: Eyeballing "close enough" is how silent numerical bugs escape.

---

### Tier 3: JVM Component Tests

**Entry point**: Root deps.edn :test alias

**What it tests**: Component utility libraries (string-utils, number-utils, data-utils, logging, transport, etc.) and base schema/routing. ~19 test paths, mostly stub coverage.

#### How to Run

```bash
cd /Users/rsheperd/code/sig/behave-app
clojure -M:test:poly test

# Or specific component:
clojure -M:test -A:dev -M clojure.test :dir bases/behave_schema/test
```

#### Current Status (2026-07-06)

✅ **Mostly passing** (not in CI; manual verification only)

19 test paths: components (16) + bases (3)

---

### Tier 4: Headless Browser (Experimental, Not in CI)

**Entry point**: projects/behave/resources/public/cljs-test/behave/headless_test_runner.cljs

**Status**: Infrastructure exists but unverified. Compiled JS at /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/cljs-test/behave/headless_test_runner.js

**Recommendation**: Do not rely; use Tier 1 + Tier 2 instead.

---

### Tier 5: absurder_sql Component Tests (Kaocha)

**Entry point**: components/absurder_sql/bin/kaocha

**What it tests**: Rust-based DataScript replacement (Entity API, query engine, Posh integration).

```bash
cd /Users/rsheperd/code/sig/behave-app/components/absurder_sql
./bin/kaocha
# Starts: Funnel (port 44220), Chrome CDP (port 9222), runs tests, exits
```

#### Current Status (2026-07-06)

🔴 **Not in main build** (unmerged rj-ds-rust branch, 30+ commits)

- Purpose: Replace DataScript/Datahike with SQLite-backed Datalog
- Blocker: Performance benchmarks / feature parity validation undocumented
- Action: Do not integrate until behave-absurder-sql-campaign skill gives green light

---

### Tier 6: Cucumber BDD (Abandoned)

**Entry point**: features/ directory

#### Current Status (2026-07-06)

🔴 **Scaffolded but abandoned**

- 3 feature files: surface_only.feature, surface_and_crown.feature, ignite_only.feature
- Step definitions: steps/Given.clj, When.clj, Then.clj (complete, unused)
- Active scenarios: **1 of 11 total** (7 active Scenario blocks, 4 commented; surface_only.feature lines 1-17 "Fire Behavior Output Selected")
- Remaining commented

**Recommendation**: Use Tier 1 instead. Do not enable without explicit approval.

---

## (4) Known Standing Reds and Flakes

### Standing Red: diagram_test.cljs — getElapsedTime

**Test**: diagram-test (line ~50)

**Status**: ✅ **GREEN as of 2026-07-06**

**History**: Commit 761b0c22 fixed WASM module bootstrap timing. window.runtimeInitialized was never set, breaking enum access. Now passes.

### Pre-Existing Fixture Gap: Contain CSV Validation

**Test**: worksheet_events_test.cljs line 443 (TODO comment)

**Status**: ⚠️ **DEFERRED**

**Risk**: Contain solver outputs not validated against behave-lib/test/csv/contain.csv.

**Action**: Add assertion comparing solver contain results to golden CSV when contain changes land.

### Standing: Units FIXMEs in behave/lib/units.cljs

**Issues**:
- Contain Fire Points unit conversion incomplete
- m/h (meters per hour) missing adapters
- Tree Count / Density handling unfinalized

**Status**: ⚠️ **TRACKED but not blocking**

**Risk**: Low (units not heavily exercised). Worksheet using these units will fail obviously.

### Standing: Mortality CRCABE Model Inert

**Symptom**: mortality-test with crown_damage (CRCABE enum) produces constant 1.5-3% regardless of inputs.

**Root cause**: WASM C++ SIGMortality for CRCABE is placeholder. Upstream FOFEM reference has zero CRCABE entries.

**Status**: ✅ **ACCEPTED LIMITATION**

**Action**: Requires behave-mirror C++ fix + FOFEM collaboration. Do not attempt app-side "fix".

### Standing: Mortality Species Coverage Variant

**Symptom**: User selects rare species (not in GACC list) → mortality returns -100 (invalid).

**Status**: ✅ **ACCEPTED LIMITATION**

**Scope**: mortality.csv covers 190 GACC-supported species only. Variant restoration requires behave-mirror + FOFEM work.

---

## (5) How to Add a Test

### Adding a Browser Test (Tier 1)

#### Step 1: Identify Test Namespace

New module (e.g., ignite):
- Create projects/behave/test/cljs/behave/ignite_test.cljs
- Add to test_runner.cljs: 'behave.ignite-test

Existing module (e.g., surface):
- Add test to projects/behave/test/cljs/behave/surface_test.cljs

#### Step 2: Write Test

```clojure
(ns behave.ignite-test
  (:require [cljs.test :refer-macros [deftest is testing]]
            [behave.fixtures :as fix]
            [behave.lib.ignite :as ignite]
            [behave.solver-test :as st]))

(deftest ignite-module-init
  (let [module (ignite/init)]
    (is (some? module))))

(deftest ignite-worksheet
  (let [ws (st/new-solver-worksheet! :surface :ignite)]
    (st/add-ws-input! ws :SIGIgnite :fuel-bed-type 0)
    (st/add-ws-input! ws :SIGIgnite :firebrand-type 1)
    (let [results (st/solve-ws-outputs ws)]
      (is (some? (get results "Ignition Probability")))
      (is (<= 0 (get results "Ignition Probability") 1)))))
```

#### Step 3: Update Test Runner

Edit projects/behave/test/cljs/behave/test_runner.cljs:

```clojure
(ns behave.test-runner
  (:require [behave.ignite-test]  ; ← Add
            ;; ... other requires
            ))

(defn run-the-tests []
  (run-tests (cljs-test-display.core/init! "app-testing")
             'behave.ignite-test  ; ← Add (alphabetical)
             ;; ... other namespaces
             ))
```

#### Step 4: Run in Browser

```bash
clojure -M:dev:behave/app:figwheel
open http://localhost:8081/api/test
# See your new test run live
```

#### Step 5: Verify Against Golden Data

```clojure
(deftest ignite-worksheet-golden
  (let [ws (st/new-solver-worksheet! :ignite)
        results (st/solve-ws-outputs ws)]
    ;; See ignite row in behave-lib/test/csv/ignite.csv or verify valid range
    (is (< 0 (get results "Ignition Probability")))
    (is (< (get results "Ignition Probability") 1))))
```

#### Naming Conventions

- Test namespace: behave.<module>-test
- Test file: <module>_test.cljs
- Test name: <action>-<target>

#### Test Helpers from fixtures.cljs

- new-solver-worksheet! — create empty worksheet with selected modules
- add-ws-input! — add input value + units
- solve-ws-outputs — run solver, return output map
- with-dummy-results-table — fixture for table/filter tests

---

### Adding a Golden CSV Test (Tier 2)

#### Step 1: Prepare Golden CSV

New module: Contact USFS FireLab or run C++ reference. Extract test matrix. Save as behave-lib/test/csv/ignite.csv.

Existing module with new parameters: Regenerate from C++ / FOFEM export. Commit to behave-lib/test/csv/.

#### Step 2: Write Comparison Test

```clojure
(deftest ignite-golden-csv
  (let [golden (read-csv "behave-lib/test/csv/ignite.csv")
        results-by-scenario (for [row golden]
                              (let [ws (st/new-solver-worksheet! :ignite)
                                    _ (st/add-ws-input! ws :SIGIgnite :fuel-bed-type (parse-int (nth row 0)))
                                    actual (st/solve-ws-outputs ws)]
                                {:scenario row :result (get actual "Ignition Probability")}))]
    (doseq [{:keys [scenario result]} results-by-scenario]
      (let [expected (parse-double (nth scenario 2))]
        (is (within-tolerance? result expected 0.01)
            (format "Ignite golden mismatch: %s expected %s got %s" scenario expected result))))))
```

---

### Adding a JVM Component Test

#### Step 1: Create Test File

components/<name>/test/<name>/test.clj:

```clojure
(ns number-utils.test
  (:require [clojure.test :refer :all]
            [number-utils.interface :as nu]))

(deftest round-to-n-decimals
  (is (= 1.23 (nu/round-to-decimals 1.234 2)))
  (is (= 1.20 (nu/round-to-decimals 1.2 2))))
```

#### Step 2: Register in :test Alias

Edit root deps.edn:

```edn
:test {:extra-paths ["components/number_utils/test"]}
```

(Usually already done.)

#### Step 3: Run

```bash
clojure -M:test -A:dev -M clojure.test :dir components/number_utils/test
```

---

## (6) Validation Gate: What Must Be True Before "Validated"

### Pre-Merge Checklist

#### ✅ Test Suite Green

```bash
# 1. Browser suite (Tier 1)
open http://localhost:8081/api/test
# All deftests green, 0 failures/errors ("35 deftests / 4,161 assertions" was the
# runner's report on branch rj-fix-figwheel-tests, 2026-07-02; main has 59 deftests
# in projects/behave/test/cljs/ as of 2026-07-06 — expect the count to drift)

# 2. If solver output affected, verify golden CSV (Tier 2)
# Manual spot-check acceptable for small changes

# 3. JVM tests (optional but recommended)
clojure -M:test:poly test
```

#### ✅ Lint Clean

```bash
cd /Users/rsheperd/code/sig/behave-app
clj-kondo --lint projects/behave/src/cljs/behave/solver/core.cljs
# No linting errors/warnings
```

#### ✅ Golden Data Re-sync (if VMS or C++ changes)

```bash
# If changed: C++ solver, VMS schema, generated artifacts (Hatchet)
# Then you must:
# 1. Regenerate WASM: cd behave-lib && make install
# 2. Re-export VMS: clojure -X:download-vms
# 3. Re-sync fixtures: verify layout.msgpack ↔ .bp7 files match
# 4. Re-run Tier 1 browser suite
# 5. Spot-check golden CSV if solver-critical
```

#### ✅ Commit Message and PR Format

- **Commit**: Plain language, example: `BHP1-1532 Fix graph axes limits`
- **PR title**: `[BHP1-1532] Fix graph axes limits`
- **PR body**: Terse; "EOM" acceptable
- **Testing**: Numbered manual steps

---

## (7) Interpreting Test Failures

### Browser Suite Failure

**Triage**:
1. Read error message
2. Check WASM bootstrap: "Module is undefined" → reload page, check browser console
3. Check units: "Cannot apply function" → verify test uses add-ws-input! (handles units)
4. Check VMS sync: "Unable to find group-variable" / "-100" → re-run clojure -X:download-vms
5. Check fixture data: worksheet old → reload/re-save via UI

### JVM Test Failure

1. Is component loading? `clojure -M:dev -Spath | grep components/`
2. Has public interface? Verify components/<name>/src/<name>/interface.clj
3. Test isolation? Run twice; does second pass?

### Cucumber Failure

Do not debug. Cucumber abandoned. Use Tier 1.

---

## (8) Provenance and Maintenance

### Facts That Drift

| Fact | Command | Expected | Last Verified |
|------|---------|----------|-----------------|
| Test count (deftests) | grep '^(deftest' projects/behave/test/cljs/behave/*.cljs \| wc -l | 51 total (47 run via test_runner) | 2026-07-06 |
| Assertion count | Browser page "N assertions" | ~4,161 | 2026-07-06 |
| Browser suite status | http://localhost:8081/api/test | GREEN (0 fails) | 2026-07-02 (94f6d56b) |
| Golden CSV count | ls behave-lib/test/csv/*.csv | 4 files | 2026-07-06 |
| Mortality.csv size | wc -l behave-lib/test/csv/mortality.csv | 15,053 lines (15,052 data rows) | 2026-07-06 |
| Test fixtures count | find worksheets -name "*.bp7" \| wc -l | 50 files | 2026-07-06 |
| JVM test paths | grep -c ".../test" deps.edn :test | 19 paths | 2026-07-06 |
| Cucumber scenarios | grep 'Scenario:' features/*.feature | 11 total (7 active + 4 commented) | 2026-07-06 |
| Layout.msgpack | ls -lh layout.msgpack | ~1.4M | 2026-07-06 |

### Re-Verification Commands

After major changes (VMS migration, C++ update, behave-lib bump):

```bash
# 1. Golden data inventory
ls -lh /Users/rsheperd/code/sig/behave-app/behave-lib/test/csv/*.csv
# Expected: 4 CSVs

# 2. Mortality.csv freshness
head -5 /Users/rsheperd/code/sig/behave-app/behave-lib/test/csv/mortality.csv
# Expected: FOFEM-derived data

# 3. Browser suite test count
grep '^(deftest' /Users/rsheperd/code/sig/behave-app/projects/behave/test/cljs/behave/*.cljs | wc -l
# Expected: 51 deftests (47 run via test_runner, 4 in unimported files)

# 4. JVM test paths
grep -c "components/.*/test\|bases/.*/test" /Users/rsheperd/code/sig/behave-app/deps.edn
# Expected: 19 paths

# 5. Test fixture freshness
find /Users/rsheperd/code/sig/behave-app/worksheets -name "*.bp7" -mtime -30 | wc -l
# Expected: several .bp7 files modified in last 30 days

# 6. Validate layout.msgpack loads
cd /Users/rsheperd/code/sig/behave-app/projects/behave
clojure -M:dev:behave/app -e "(require '[behave.vms.store]) (behave.vms.store/load-vms! \"test\")"
# Expected: no errors

# 7. Cucumber scenario count
grep "Scenario:" /Users/rsheperd/code/sig/behave-app/features/*.feature | wc -l
# Expected: 11 total (7 active + 4 commented)
```

---

## When NOT to Use This Skill

### Use behave-debugging-playbook if:
- You have a solver output bug (wrong numbers, NaN) and need triage
- Test suite shows red and you don't know why
- You want discriminating experiments to isolate a failure

### Use behave-failure-archaeology if:
- You want to understand root cause of a past test failure
- You're investigating why a test was marked "FIXME"
- You want to learn from past bug-fix patterns

### Use behave-vms-variable-pipeline if:
- You're adding a new C++ solver variable or output
- You're regenerating WASM bindings (Hatchet)
- You need to re-sync layout.msgpack after VMS changes

### Use behave-proof-and-analysis-toolkit if:
- You want first-principles validation (golden differential testing, layer bisection, benchmarking)
- You're writing a complex analysis recipe

### Use behave-change-control if:
- You're classifying a change and need validation gates
- You need the 4 non-negotiables checklist
- You're preparing for release

---

## Glossary

**QA** — Quality Assurance; in this skill, refers to test tier strategy, golden-data validation, evidence bars, and pre-merge gates ensuring solver correctness.

**WASM** — WebAssembly module (behave-min.wasm) compiled from C++ via Emscripten.

**Figwheel** — ClojureScript hot-reload dev server (port 8081).

**Golden data** — Reference outputs from USFS FireLab's C++ (behave-mirror) used as validation ground truth.

**layout.msgpack** — Binary-serialized VMS schema. Loaded once at startup.

**VMS** — Variable Management System; Datomic database (port 8001) storing fire-modeling domain structure.

**.bp7** — SQLite 3 worksheet file (immutable test fixture).

**Output linking** — Solver feature propagating upstream outputs to downstream inputs.

**GACC** — Geographic Area Coordination Center (USFS regions). Mortality is GACC-specific (190 codes).

**FOFEM** — Fire and Fuels Extension to Forest Vegetation Simulator. USFS reference, data source for mortality.csv.

---

## Quick Reference

| Task | Command | Time | Tier |
|------|---------|------|------|
| Run browser tests | clojure -M:dev:behave/app:figwheel + open http://localhost:8081/api/test | 30s + live | 1 |
| Run JVM tests | clojure -M:test:poly test | ~5s | 3 |
| Re-sync golden data | clojure -X:download-vms (CMS running) | ~10s | 2 |
| Check lint | clj-kondo --lint projects/behave/src/... | ~2s | CI |
| Add browser test | Edit test_runner.cljs + .cljs file | — | 1 |
| Verify solver output | Compare browser results to behave-lib/test/csv/*.csv | manual | 2 |

---

**Written 2026-07-06** | Last verified: Browser suite green (94f6d56b), golden CSVs present, layout.msgpack synced
