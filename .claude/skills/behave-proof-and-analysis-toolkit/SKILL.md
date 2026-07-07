---
name: behave-proof-and-analysis-toolkit
description: First-principles analysis recipes for fire-behavior solver validation: golden differential testing, layer bisection for wrong numbers, migration dry-run + fixture re-sync, storage benchmarking, hypothesis-before-running discipline, and WASM boundary proofs. Each recipe includes worked examples from this repo's history.
---

# Behave Proof and Analysis Toolkit

**Last verified: 2026-07-06**

## When to use this skill

- The solver produces wrong numbers and you need to isolate the bug to a specific layer (VMS, CLJS wrapper, WASM, C++)
- A migration touches the data model and test fixtures silently drift
- Benchmarking storage engines (especially absurder_sql) to prove performance claims
- Validating that solver changes don't regress against reference data
- Proving a value crosses the WASM/CLJS boundary correctly
- Before running any "fix," you need to predict what the measurements should show

## When NOT to use this skill

Use sibling skills for:
- **Unit testing at the Clojure/ClojureScript tier**: see `behave-validation-and-qa` for test tier architecture and runbook
- **Isolating WASM build or Node.js environment issues**: see `behave-build-and-env` for toolchain diagnostics
- **Debugging a test failure's stack trace or error message**: see `behave-debugging-playbook` for triage flowchart
- **Understanding solver architecture and invariants**: see `behave-architecture-contract` for load-bearing design
- **Reading help/VMS/config changes end-to-end**: see `behave-vms-variable-pipeline` for the full propagation path

---

## Recipe 1: Golden Differential Testing — Regenerate Expectations from an Authoritative Upstream

**When to use:** A test fails because expected values drift from the model (not because the model is wrong, but because the test's golden data is stale, out-of-scope, or mixes model variants).

**Why this recipe works:** The Behave7 solver is a pure computation with deterministic inputs and outputs. When you suspect test data corruption, regenerate from the upstream authoritative source (not hand-edited CSVs or spreadsheets). Differential testing (observed vs. golden, within tolerance) is the validation bar.

### Worked Example: mortality.csv Regenerated from C++ Reference (MORTALITY_TEST_HANDOFF.org)

**Context:** `mortality_test.cljs` runs 15,052 CSV rows through the WASM mortality module, comparing computed tree mortality % to `MortAvgPercent` golden. Result: 7,119 rows returned -100 (species not found), 3,444 rows were wrong values, 4,489 passed. The test was useless.

**Root cause (via layer bisection below):** Two unrelated problems revealed by equation type:

1. **Coverage bug (7,119 CRNSCH rows):** The WASM species table is the reduced 190-code GACC set (behave-mirror compiled state); `mortality.csv` referenced 525 codes → 339 misses. The CSV is over-broad.
2. **Model inertness bug (3,400 CRCABE rows):** Crown_damage setters had no effect; upstream reference (behave-mirror C++ testMortality) never validated crown_damage at all.

**The fix:** Regenerate `mortality.csv` from the authoritative upstream source:

```bash
# 1. Locate the C++ reference in the submodule
cd behave-lib/behave-mirror
ls src/testMortality/
# Output: FOFEM_input.tre (inputs), resultsProbMort.csv (golden)

# 2. Read the reference structure
head -20 src/testMortality/FOFEM_input.tre
head -10 src/testMortality/resultsProbMort.csv
# FOFEM_input.tre: 3,825 rows (CRNSCH + BOLCHR only, NO CRCABE)
# resultsProbMort.csv: BehaveProbability vs FOFEMProbability columns

# 3. Generate new CSV: for each FOFEM row, emit input cols 1-20 + FOFEMProbability as MortAvgPercent
# Exclude rows where |BehaveProbability - FOFEMProbability| >= 4 (known Behave-vs-FOFEM discrepancies)
# Example: POTR12 (aspen) x21 rows, a high-scorch edge case x6 rows

# 4. [PLANNED WORK] Regenerate CSV (pseudocode in MORTALITY_TEST_HANDOFF.org)
# Target: 15,052 → 3,798 rows (drop 339 unsupported variant species + 3,832 unvalidated CRCABE)
# Current state (as of 2026-07-06): file still has 15,052 data rows pending regeneration

# 5. Test: run in-page and confirm green
# Figwheel: localhost:8081/api/test
# Expect (after regeneration): mortality_test 3,798 pass, 0 -100, 0 wrong

cd /Users/rsheperd/code/sig/behave-app
# Check actual file (it's a symlink to behave-lib/test/csv/)
ls -la projects/behave/resources/public/csv/mortality.csv
# lrwxr-xr-x  projects/behave/resources/public/csv/mortality.csv \
#   -> ../../../../behave-lib/test/csv/mortality.csv
wc -l behave-lib/test/csv/mortality.csv
# 15053 lines (15,052 data + 1 header) — regeneration not yet completed
```

**Key principle:** **Never hand-edit golden data.** If the source updates (e.g., behave-mirror C++ reference changes), regenerate deterministically. Document the source commit/version.

**Re-verification steps (as of 2026-07-06):**

- Verify that `mortality.csv` is a symlink to behave-lib: `readlink projects/behave/resources/public/csv/mortality.csv`
- Verify current row count: `wc -l behave-lib/test/csv/mortality.csv | awk '{print $1}'` currently 15053 (regeneration planned, not yet completed)
- Verify the regeneration plan is in `MORTALITY_TEST_HANDOFF.org` line 265-289
- After regeneration is complete, confirm test passes by running figwheel suite at `localhost:8081/api/test` and checking mortality assertions

---

## Recipe 2: Layer Bisection for Wrong Numbers — Isolate the Bug to VMS / CLJS Wrapper / WASM / C++

**When to use:** The solver computes a wrong value (e.g., spread rate 82.44 ch/h instead of 19.68), and you need to narrow down which layer the bug is in: app-side data, CLJS wrapper, WASM bindings, or C++ model.

**Why this recipe works:** The Behave7 solver has discrete layers with API boundaries. By testing each layer independently (skipping the layers above), you locate the bug precisely and avoid cascading misdiagnosis.

### Layer Stack (Top to Bottom)

| Layer | Code | API Input | API Output | Example Fail Mode |
|-------|------|-----------|------------|-------------------|
| **App/VMS** | `projects/behave/src/cljs/behave/solver/core.cljs` | Worksheet data (input-groups) + VMS units-uuid | Outputs in VMS units | Units-uuid is nil or wrong enum |
| **CLJS wrapper** | `projects/behave/src/cljs/behave/lib/*.cljs` (auto-generated) | Value + WASM unit enum | Value in output units | Wrapper calls wrong setter, skips setters |
| **WASM bindings** | `behave.lib.enums`, `behave.lib.units` | WASM unit enum int, value in base units | Value in result units | Enum mismatch, unit conversion bug |
| **C++ model** | `behave-lib/behave-mirror` (C++, Emscripten) | Function call on SIGSurface/Crown/etc. | Computed value | Model math wrong |

### Worked Example: Units-UUID Persistence Bug (SOLVER_TEST_HANDOFF.org)

**Symptom:** `surface-worksheet` test computed 82.44 ch/h (wrong by 4.2x) where direct calculation was 19.68 ch/h. Wind looked "amplified."

**Attack plan (layer bisection):**

**Step 1: Test the C++ model directly (Layer 4)**
```clojure
; In spel (browser automation) or CLJS REPL at localhost:8081
; Call the WASM directly, bypassing worksheet machinery

(let [m (js/Module.SIGSurface)
      _ (.setFuelModelNumber m 124)
      _ (.setMoisture1HrDead m 6 (get-in-page (enums/probability-units "Percent")))
      _ ; ... set all inputs with explicit units ...
      _ (.doSurfaceRun m)
      result (.getSpreadRate m)]
  (prn "Direct WASM spread:" result))
; Expected: 19.68 (matches golden)
; If this is correct, the bug is above layer 4 (not in C++).
; If this is wrong, the bug is in C++ model or unit enum mismatch.
```

**Step 2: Test the CLJS wrapper (Layer 3)**
```clojure
; Call the generated wrapper functions directly (no worksheet)

(let [m (surface/init)
      _ (surface/setFuelModelNumber m 124)
      _ (surface/setMoisture1HrDead m 6 (enums/probability-units "Percent"))
      _ ; ... all inputs ...
      _ (surface/doSurfaceRun m)
      result (surface/getSpreadRate m)]
  (prn "CLJS wrapper spread:" result))
; Expected: 19.68
; If wrong: the wrapper or unit enum is broken.
; If correct: the bug is in app-side data or input delivery (layer 2).
```

**Step 3: Test input delivery via worksheet (Layer 2)**
```clojure
; Instrument apply-single-cpp-fn to log every setter call

; In solver/core.cljs line 34, add a log:
(log-solver [:SINGLE fn-name value unit])
; Then run the worksheet solve through the app.

; Open localhost:8081/api/test and watch console logs.
; Expect: 'SINGLE setMoisture1HrDead 6 651dadb7-...' (units-uuid)
; If unit is nil or is an entity ID (e.g. 4874), that's the bug.
```

**Step 4: Test VMS data (Layer 1)**
```clojure
; Query the VMS in the Figwheel console

(let [vms-conn (behave.vms.store/vms-conn)]
  (prn (d/q '[:find ?v ?u
              :where [?v :bp/uuid "units:probability:percent"]
                     [?v :domain/units-uuid ?u]]
            (d/db vms-conn))))
; If this returns nil or multiple entities, the VMS has units-uuid resolution bugs.
```

**Outcome (Feb 2026):** Testing revealed Layer 2 bug:

- `add-ws-input!` dispatched `:worksheet/upsert-input-variable` with 6 args, but the handler only destructured 5 → units-uuid silently dropped.
- Solver then fell back to variable's *native* unit, causing silent misconversions:
  - Moisture stored as fractions (0.2) but labeled Percent (enum 1) → WASM read 0.2% → bone-dry fuels → spread over-amplified.
  - Canopy bulk density stored as lb/ft³ labeled kg/m³ → 16x too low.

**Fix:** Rewrote `add-ws-input!` to dispatch `:worksheet/update-input-units` separately, mirroring the app's real event flow.

**Re-verification steps (as of 2026-07-06):**

- Run `git log --oneline | grep -E 'units-uuid|solver.*dispatch'` to find the fix commit
- Verify the fix is in place: `grep -A 5 "update-input-units" projects/behave/resources/public/cljs-test/behave/solver_test.cljs`
- Confirm worksheet test passes: run figwheel suite, check `surface-worksheet` and `crown-worksheet` assertions

---

## Recipe 3: Migration Dry-Run + Fixture Re-Sync Verification

**When to use:** A VMS migration (e.g., adding a variable, renaming a field) is committed, and tests that depend on the VMS silently drift because fixtures weren't re-synced.

**Why this recipe works:** The VMS (Variable Management System) is a Datomic database living in behave_cms. When the schema changes (via migrations in `projects/behave_cms/resources/migrations/`), the client's Datascript view and test fixtures must re-sync. Without this, tests pass the assertions but measure garbage (silent data corruption).

### Worked Example: GACC Region Rename Migration (MORTALITY_TEST_HANDOFF.org)

**Context:** A CMS migration renamed the `mortality_region` field to `gacc_region` in the VMS. The solver test called `mortality/setGACCRegion` but the VMS still exported a `setRegion` function. Tests compiled but produced wrong enums.

**The migration (behave_cms, pseudocode example):**
```clojure
; Example migration pattern for renaming a VMS function
; Actual migrations in projects/behave_cms/resources/migrations/ may differ
; (e.g., 2025_02_20_add_gacc_lookup_functions.clj, 2025_02_20_add_gacc_tags_to_mortality_tree_species.clj)
(d/transact conn [[:db.fn/retractEntity [:cpp.function/name "setRegion"]]
                   [{:cpp.function/name "setGACCRegion"
                     :cpp.class/uuid (d/entity-id "SIGMortality")
                     ; ... other attrs ...
                   }]])
```

**Problem:** The test and worksheet fixtures used the old enum value. When the function disappeared from the VMS, solver lookups failed silently.

**Dry-run + fixture re-sync:**

```bash
# 1. Run the migration locally (dry-run is implicit; Datomic is transactional)
cd /Users/rsheperd/code/sig/behave-app
clojure -M:dev:behave/cms:server &
# Wait for Datomic transactor (port 4334)

# 2. Query the VMS before migration
# Use behave_cms Datomic REPL (via clojure-mcp)
(require '[datomic.api :as d])
(def conn (d/connect "datomic:sql://..."))

; Count old function names
(d/q '[:find (count ?e)
       :where [?e :cpp.function/name "setRegion"]]
  (d/db conn))
; Expect: 1 before migration

# 3. Apply the migration
(schema-migrate.runner/run-pending-migrations! conn)

# 4. Query after and verify the new function exists
(d/q '[:find (count ?e)
       :where [?e :cpp.function/name "setGACCRegion"]]
  (d/db conn))
; Expect: 1 after migration

# 5. Export layout.msgpack from the CMS
clojure -M:dev:behave/cms:server \
  -X behave-cms.server/export-vms-layout :output-path layout.msgpack

# 6. Copy layout.msgpack to the test fixtures
cp layout.msgpack projects/behave/resources/public/layout.msgpack

# 7. Update test code that references the old enum
# Before: (mortality/setRegion module (enums/gacc "SouthernArea"))
# After:  (mortality/setGACCRegion module (enums/gacc "SouthernArea"))

# 8. Rebuild and run tests
clojure -M:dev:behave/app:figwheel &
# Open localhost:8081/api/test
# Check that mortality-related tests still pass

# If fixtures drift:
# - Test results change (e.g., mortality values suddenly all -100 or NaN)
# - No error messages (silent corruption — this is the trap!)
```

**Key principle:** **Migrations require THREE synchronized updates:**
1. Datomic schema (the migration file)
2. Test fixtures (e.g., `layout.msgpack`)
3. Test code (enum names, function names, hardcoded values)

**Re-verification steps (as of 2026-07-06):**

- List all CMS migrations: `ls projects/behave_cms/resources/migrations/ | tail -10`
- Verify GACC rename exists: `grep -l "gacc\|setGACCRegion" projects/behave_cms/resources/migrations/*`
- Check that solver_test.cljs uses the new enum: `grep "setGACCRegion" projects/behave/test/cljs/behave/solver_test.cljs`
- Confirm layout.msgpack is recent: `stat projects/behave/resources/public/layout.msgpack | grep Modify`

---

## Recipe 4: Storage-Engine Benchmarking Methodology for absurder_sql

**When to use:** Evaluating the absurder_sql Rust+SQLite DataScript replacement against the current JavaScript DataScript. Proving performance claims before migration.

**Why this recipe works:** Storage engines are the lowest-level performance lever. Benchmarks must control variables (same dataset, warmup, median latency, not average), isolate operations (load, query, transact), and report confidence intervals. Eyeballing is unacceptable.

### What to Measure

| Operation | Why | Unit | Example Threshold |
|-----------|-----|------|-------------------|
| **VMS load time** | App startup latency (layout.msgpack deserialization + Datascript insertion) | ms | < 500 ms (current ~300-400) |
| **Query latency (p50, p99)** | Common queries: variable lookups, rule resolution | ms | p50 < 10 ms, p99 < 50 ms |
| **Transaction throughput** | Worksheet input mutations (solver writes 1000s of outputs) | ops/sec | > 10k ops/sec |
| **Memory overhead** | Browser memory footprint (DataScript indices consume RAM) | MB | < 100 MB over baseline |
| **Worksheet.bp7 load** | SQLite DB restore from disk | ms | < 200 ms |

### Benchmark Template

```clojure
; File: components/absurder_sql/benchmarks/vms_bench.clj
; Assumes: real layout.msgpack, worksheet fixture, and both DataScript + absurder_sql impls loaded

(ns absurder-sql.benchmarks.vms-bench
  (:require [criterium.core :refer [benchmark with-progress-reporting]]
            [datascript.core :as ds-old]
            [absurder-sql.core :as ds-new]))

(defn load-vms-datascript []
  "Load layout.msgpack via current JS DataScript impl (as baseline)"
  ; Deserialize msgpack, transact all entities into a fresh Datascript conn
  ; Return: [conn elapsed-ms]
  )

(defn load-vms-absurder []
  "Load layout.msgpack via Rust SQLite impl"
  ; Same data, Rust backend
  )

(defn variable-lookup-query [conn var-name]
  "Q: find all variables matching a name prefix"
  (ds/q '[:find ?e ?v :where [?e :variable/name ?v]]
        (ds/db conn)))

(defn bench-vms-load []
  (with-progress-reporting
    (println "DataScript VMS load:")
    (benchmark (load-vms-datascript) :samples 10)
    
    (println "absurder_sql VMS load:")
    (benchmark (load-vms-absurder) :samples 10)))

(defn bench-queries [conn]
  (with-progress-reporting
    (println "Variable lookup x1000:")
    (benchmark 
      (doseq [_ (range 1000)]
        (variable-lookup-query conn "fuel"))
      :samples 100)))

; Usage: lein bench or clj -M:benchmark
```

**Key disciplines:**

1. **Same dataset:** Use the exact same `layout.msgpack` (real VMS, not synthetic) for both impls.
2. **Warmup:** Run 3-5 iterations before measuring to eliminate JIT/cache cold-start.
3. **Medians, not averages:** Report p50, p95, p99 latencies. Outliers skew mean.
4. **Isolation:** Measure each operation independently (load, query, transact separately). Don't conflate.
5. **Confidence intervals:** Report ±2σ if sample size < 100. Fewer than 30 samples means high uncertainty.

**Red flags (wrong benchmarking):**

- "absurder_sql is 2x faster" without median/p99 numbers → likely measuring only hot path
- Benchmarking on synthetic (toy) data → doesn't reflect real workload
- Averaging latencies (mean) instead of reporting p50/p95/p99 → outliers hide truth
- No warmup → JIT compilation time pollutes first run

**Re-verification steps (as of 2026-07-06):**

- Check if benchmarks exist: `ls components/absurder_sql/benchmarks/ 2>/dev/null || echo "no benchmarks found"`
- Confirm absurder_sql branch exists: `git branch -a | grep ds-rust`
- Look for performance claims in commits: `git log --oneline --all | grep -i "speed\|perf\|bench"`
- (Unmerged as of 2026-07, so benchmarks likely in branch only)

---

## Recipe 5: Hypothesis-Predicts-Numbers-Before-Running Discipline

**When to use:** Before running any test, solver fix, or investigation, formalize what you expect to observe and why.

**Why this recipe works:** Unstructured debugging leads to confirmation bias ("I found something, it must be the bug") and wasted time. Writing the hypothesis first forces rigor and creates a check against false positives.

### Hypothesis Template

```
**If hypothesis H holds, then command C should print X ± ε.**

Example:
  Hypothesis: "Units-uuid is nil because add-ws-input! drops the 6th arg."
  If true, then:
    Command: (log-solver [:SINGLE fn-name value unit]) before calling solver
    Expected: Unit is nil for all 2-param setters (moisture, tree height, DBH)
    Tolerance: All affected setters (e.g., setMoisture1HrDead, setDBH) report nil
    Confidence: >90% if 10+ setters show nil, <30% if only 1-2 show nil
```

### Worked Example: Canopy Bulk Density Input Bug (SOLVER_TEST_HANDOFF.org)

**Hypothesis chain:**

1. **H1:** "Crown fireType is constant Torching because canopy inputs (canopy bulk density) don't land."
   - If true, then: Direct WASM call with CBD values → fireType varies
   - Verification: `let m = Module.SIGCrown(); m.setCanopyBulkDensity(...); m.doCrownRun(); m.getFireType()` should vary by CBD
   - **Outcome:** True. Direct WASM call varied fireType as expected.

2. **H2:** "The worksheet path mis-delivers canopy inputs because units are wrong (labeled kg/m³ but value in lb/ft³)."
   - If true, then: Layer bisection (recipe 2) should show WASM gets kg/m³ enum with lb/ft³ value
   - Verification: Log the setter call: `[SINGLE setCanopyBulkDensity 0.03 kg-m3-enum]` where 0.03 is lb/ft³
   - Expected: Solver multiplies by 16× conversion factor, making CBD too low → active-ratio threshold never crossed
   - **Outcome:** True. CBD 0.03 lb/ft³ labeled as kg/m³ → WASM read as 0.48 kg/m³ (16x) → active ratio never triggered → fireType frozen at Surface
   - **Confidence:** 95%+ (direct math: 0.03 lb/ft³ × 16 = 0.48 kg/m³, and 0.48 is below the 0.06 threshold for active crown fire)

3. **H3:** "The fix is to dispatch `:worksheet/update-input-units` after `:worksheet/upsert-input-variable` so units are normalized before solve."
   - If true, then: Rewrite `add-ws-input!` to dispatch both events, and re-run `crown-worksheet` test
   - Expected: fireType now varies (0, 1, 2, 3 matching crown.csv) instead of constant Torching
   - **Outcome:** True. Test now green: `fireType = [0 1 3 3 2]` matching golden `[0 1 3 3 2]`
   - **Confidence:** 99% (direct assertion passed)

**Output format:**
```
Hypothesis: <one-sentence claim>
  If true, command <copy-pasteable command>
  Expect: <exact observable output, with variance tolerance>
  Confidence level: X% (reasoning)
  Outcome: ✓ True / ✗ False / ? Uncertain (reason)
```

**Red flags (wrong hypothesis):**

- "The solver is broken" without specifying what computation is wrong → too vague to test
- "The WASM is slow" without baseline or comparing vs. direct JS → unmeasurable
- Hypothesis requires running 3+ commands to verify → too complex, break into smaller hypotheses

**Re-verification steps (as of 2026-07-06):**

- Read the SOLVER_TEST_HANDOFF.org Hypothesis sections (lines 88-100, 108-121): all hypotheses were written *before* fixes, then outcomes recorded
- Confirm crown-worksheet test now passes: `grep -A 5 "crown-worksheet" projects/behave/resources/public/cljs-test/behave/solver_test.cljs`
- Check the fix commit message for hypothesis-style reasoning: `git log --oneline | grep -i 'update-input-units\|units-uuid' | head -5`

---

## Recipe 6: Reading the WASM Boundary — Proving a Value Crosses Correctly

**When to use:** A WASM module getter returns a wrong value, or a setter silently ignores its input. You need to prove whether the bug is in the Emscripten bindings, the unit enum mapping, or the C++ code.

**Why this recipe works:** The WASM boundary is where ClojureScript values transform into C++ enums and floats. Mismatches here are silent (no error, just wrong computation). Direct unit-enum comparison against behaveUnits.h pinpoints the mismatch.

### The WASM Unit Enum Bridge

The C++ code defines unit enums in `behave-lib/behave-mirror/src/behave/behaveUnits.h`:

```cpp
// behaveUnits.h (excerpt)
enum ProbabilityUnits {
  ProbabilityUnits_Percent = 0,
  ProbabilityUnits_Fraction = 1
};

enum SpeedUnits {
  SpeedUnits_MetersPerMinute = 0,
  SpeedUnits_FeetPerMinute = 1,
  SpeedUnits_MilesPerHour = 5  // Note: not 2, 3, 4 — there's a gap!
};
```

The ClojureScript enums are auto-generated by Hatchet and live in `behave/lib/enums.cljs`:

```clojure
(def probability-units
  (enum "ProbabilityUnits"
    ["ProbabilityUnits::Percent"      ; index 0
     "ProbabilityUnits::Fraction"]))   ; index 1

(def speed-units
  (enum "SpeedUnits"
    ["SpeedUnits::MetersPerMinute"     ; index 0
     "SpeedUnits::FeetPerMinute"       ; index 1
     "SpeedUnits::MilesPerHour"]))     ; index 2 (WRONG! should map to C++ enum value 5)
```

**The trap:** The CLJS enum index (0, 1, 2, ...) may NOT match the C++ enum value (0, 1, 5, ...). If the CLJS wrapper passes index instead of value, the WASM receives the wrong unit.

### Worked Example: Probability Units (Percent vs. Fraction)

**Symptom:** Mortality module returned 64.98% when fed inputs in Fraction units (0.2), but expected 65%.

**Investigation:**

```bash
# Step 1: Verify the C++ enum values
cd behave-lib/behave-mirror
grep -A 3 "enum ProbabilityUnits" src/behaveUnits.h
# Output:
# enum ProbabilityUnits {
#   ProbabilityUnits_Percent = 0,
#   ProbabilityUnits_Fraction = 1
# };

# Step 2: Check the CLJS enum
grep -A 3 "def probability-units" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/enums.cljs
# Output:
# (def probability-units
#   (enum "ProbabilityUnits"
#     ["ProbabilityUnits::Percent"
#      "ProbabilityUnits::Fraction"]))

# Step 3: In-page test: call the enum and verify value
# Via spel (browser automation):
(require '[behave.lib.enums])
(prn (behave.lib.enums/probability-units "Percent"))   ; Expected: 0
(prn (behave.lib.enums/probability-units "Fraction"))  ; Expected: 1

# Step 4: Direct WASM call with explicit unit enum
# Call the mortality module with Fraction = 1
(let [m (js/Module.SIGMortality)
      _ (.setSpeciesCode m "ABAM")
      _ (.setGACCRegion m 1)  ; Enum value for SouthernArea
      _ (.setProbabilityUnits m 1)  ; Explicitly pass Fraction = 1
      _ (.setEquationType m 0)  ; CRNSCH
      _ (.setScorchHeight m 4)  ; Fraction units (0.3 fraction * 100 ft ~= 4 ft visible)
      _ (.calculateMortalityAllDirections m 1)  ; Use Fraction units
      result (.getProbabilityOfMortality m 1)]  ; Read in Fraction units
  (prn "Mortality (Fraction units):" result))
; Expected: 0.65 (65% as a fraction)
# If result is 64.98, the CLJS wrapper is correctly passing the enum value.
# If result is garbage, the enum mapping is wrong.

# Step 5: Wrapper-layer test (CLJS functions)
; Call the generated wrapper function
(let [m (mortality/init (species-master-table/init))
      _ (mortality/setSpeciesCode m "ABAM")
      _ (mortality/setGACCRegion m (enums/gacc "SouthernArea"))
      _ (mortality/setEquationType m (enums/fire-type "crown_scorch"))
      _ (mortality/setScorchHeight m 4)
      _ (mortality/calculateMortalityAllDirections m (enums/probability-units "Fraction"))
      result (mortality/getProbabilityOfMortality m (enums/probability-units "Fraction"))]
  (prn "Wrapper result:" result))
; If this matches the direct WASM call, the wrapper is faithfully forwarding enums.
```

**Interpretation table:**

| Direct WASM | Wrapper | Diagnosis |
|------------|---------|-----------|
| ✓ Correct (0.65) | ✓ Correct (0.65) | Wrapper is correct; bug is in app layer |
| ✓ Correct (0.65) | ✗ Wrong (0.23) | Wrapper is mis-forwarding the enum or value |
| ✗ Wrong (0.23) | ✗ Wrong (0.23) | C++ model or Emscripten enum binding is wrong |

### Proof Template: Round-Trip Unit Conversion

**Assert that a value survives the round-trip: CLJS → C++ (convert to base units) → read back (convert from base).**

```clojure
; Test: passing wind speed 5 mi/h and reading it back

(let [input-value 5.0
      input-unit (enums/speed-units "MilesPerHour")  ; enum value 5
      module (surface/init)
      _ (surface/setWindSpeed module input-value input-unit)
      ; The WASM converts 5 mi/h to base units (ft/min):
      ; 5 mi/h = 5 * 5280 / 60 = 440 ft/min
      read-back-ft-min (surface/getWindSpeed module (enums/speed-units "FeetPerMinute"))
      ; Expect: ≈ 440 ft/min
      read-back-mi-h (surface/getWindSpeed module (enums/speed-units "MilesPerHour"))
      ; Expect: ≈ 5 mi/h
      ]
  (prn "Input (mi/h):" input-value)
  (prn "Read back (ft/min):" read-back-ft-min "expect ≈440")
  (prn "Read back (mi/h):" read-back-mi-h "expect ≈5")
  ; If both match, units round-trip correctly.
  )
```

**Key assertion:** If `input` ≠ `read-back` (after converting to the same unit), the WASM boundary has a conversion or enum bug.

**Re-verification steps (as of 2026-07-06):**

- Verify enum definitions exist: `grep -c "def.*-units" projects/behave/src/cljs/behave/lib/enums.cljs` (should be >20)
- Check that Hatchet auto-generates the enums: `git log --all --oneline | grep -i hatchet | head -5`
- Verify GACC enum matches species_master_table.h: `grep "enum class GACC" behave-lib/behave-mirror/src/behave/species_master_table.h`
- Confirm mortality setGACCRegion is generated: `grep "setGACCRegion\|setRegion" projects/behave/src/cljs/behave/lib/mortality.cljs`

---

## Cross-Layer Validation Checklist

Before closing any "solver produces wrong numbers" bug, verify all layers:

```
□ C++ model math
  - [ ] Direct WASM call with hardcoded values matches expected math
  - [ ] Verify C++ reference (FOFEM, BehavePlus) agrees on expected value
  
□ Emscripten enums
  - [ ] Enum values in enums.cljs match C++ behaveUnits.h
  - [ ] Enums are not indices (0, 1, 2) but actual C++ enum values (sometimes 0, 1, 5)
  
□ CLJS wrapper
  - [ ] Generated wrapper functions forward unit enums correctly
  - [ ] No silent failures when unit is nil (apply-single-cpp-fn should error, not skip)
  
□ Input delivery (worksheet → wrapper)
  - [ ] VMS units-uuid resolve to valid WASM enum values
  - [ ] Worksheet events dispatch in correct order (value first, then units)
  
□ Test fixtures
  - [ ] Golden data matches the current model scope (regenerated from authoritative source)
  - [ ] Test fixture VMS is in sync with layout.msgpack after migrations
```

---

## Provenance and Maintenance

**Last verified against live repo: 2026-07-06**

**Corrections applied (2026-07-06):**
- Fixed file path references: `behaveUnits.h` now correctly at `src/behave/behaveUnits.h`
- Fixed GACC enum location: now correctly references `species_master_table.h` instead of `behaveUnits.h`
- Fixed `add-ws-input!` file path: now correctly points to `resources/public/cljs-test/behave/solver_test.cljs`
- Removed reference to non-existent commit 49548d1f
- Clarified that mortality.csv regeneration is planned work (current state: 15,052 data rows, not yet reduced to 3,798)
- Marked migration file example as pseudocode (actual migrations exist but filename differs)

| Fact | Re-Verification Command |
|------|------------------------|
| mortality.csv row count (should be 3,798) | `wc -l projects/behave/resources/public/csv/mortality.csv \| awk '{print $1}'` |
| GACC enum exists and has 10 regions | `grep "def gacc" projects/behave/src/cljs/behave/lib/enums.cljs && wc -l` |
| Solver_test.cljs contains add-ws-input! helper | `grep -c "defn add-ws-input" projects/behave/resources/public/cljs-test/behave/solver_test.cljs` |
| Crown-worksheet test passes L/W assertion | `grep "lengthToWidthRatio" projects/behave/resources/public/cljs-test/behave/solver_test.cljs` |
| Units-uuid persistence fix is merged | `git log --oneline main \| grep -c 'units-uuid\|update-input-units' \| test $(cat) -gt 0 && echo OK` |
| CLJS test suite is green (as of last commit) | Manual: `localhost:8081/api/test`, look for "0 failures / 0 errors" |
| CMS migrations are applied to local Datomic | `clojure -M:dev:behave/cms:server && datomic-console` (check schema version) |
| behave-lib submodule is on rj-rust-port branch | `cd behave-lib && git branch --show-current` |
| Layout.msgpack exists and is recent | `stat projects/behave/resources/public/layout.msgpack \| grep Modify` |

**Gaps and uncertainties as of 2026-07-06:**

- `absurder_sql` storage benchmarks do not yet exist (branch rj-ds-rust is unmerged; benchmarks likely in branch only)
- FOFEM reference regeneration script (step 3 in Recipe 1) is pseudo-code; actual implementation in project history
- CRCABE (crown_damage) model path remains inert (deferred to behave-mirror fix)

**When to update this skill:**

- After merging `rj-ds-rust` (absurder_sql): add actual benchmark commands and results
- After completing the species restoration effort (MORTALITY_TEST_HANDOFF.org follow-up): expand Recipe 1 with variant-code strategy
- After any major WASM schema change: re-verify enum mappings in Recipe 6
- Quarterly: run the "last verified" commands above and update timestamps
