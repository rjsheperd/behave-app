---
name: behave-research-frontier
description: "Open problems at the frontier of this codebase's capabilities: absurder_sql as a standalone Datalog-on-SQLite engine, solver parametric performance & scale, front-end startup/subscription performance. Defines current SOTA gaps, this repo's assets, concrete first steps, and falsifiable milestones."
---

# Behave7 Research Frontier: Three Open Problems (as of 2026-07-06)

This skill documents three areas where Behave7 can meaningfully advance the state of the art—and provides concrete, falsifiable milestones to measure progress. Each frontier includes why existing tools fall short, what unique assets this repo brings, the first three verifiable steps, and a testable completion criterion.

**Audience**: Opus-level agents and senior engineers. Assume full Clojure/ClojureScript/Rust fluency and familiarity with Datalog, SQLite, and fire-behavior domain. Every frontier is **candidate/open by definition**—nothing here is claimed as achieved.

---

## Frontier 1: absurder_sql as a Standalone Best-in-Class Embedded Datalog-on-SQLite Engine

### Why Current SOTA Falls Short

**DataScript** (in-memory only):
- Loses all data on browser refresh or JVM shutdown.
- No cross-session persistence without custom layer (e.g., local-storage plugins).
- Cannot handle datasets larger than available RAM.
- No built-in query optimization for large fact sets.

**Datahike** (immature + slow):
- Incomplete ClojureScript/browser support; primarily JVM-focused.
- Published performance benchmarks show 2-5x slower query latency than DataScript on in-memory sets.
- Schema evolution and migration story is fragmented.
- Community velocity is low; last stable release is 0.5.x (mid-2024).

**SQLite (raw)**:
- No Datalog query language; requires manual SQL + ORM.
- No built-in indexing strategy for datom triple-stores.
- Complex schema versioning without schema-as-data.

**absurder_sql (this repo's asset)**:
- Pure Rust/WebAssembly implementation of a Datalog engine backed by SQLite.
- Designed to be API-compatible with ClojureScript DataScript (queries, pull, entity, transact).
- Persistent browser storage (IndexedDB via Rust→WASM boundary).
- Incremental transaction support (apply_tx_data) for streaming updates.
- Cross-tab coordination for multi-window support (commit 66209d54).

### This Repo's Unique Assets

1. **30+ Commits of Active Development** (rj-ds-rust branch):
   - Rust core in `/components/absurder_sql/rust/datascript-rs/` with full query engine, transaction processing, pull/entity APIs.
   - Persistent-sorted-set data structure (commit 686acad0) for O(log n) datom indexing.
   - Entity API and built-in functions (commit 20f3bd4b).
   - Posh integration feature parity (commit 0f64bd97).
   - 280+ integration tests in `store_integration_test.cljs` (commit f2fddc2c).

2. **Proven Fire-Science Workload**:
   - 50 test worksheets (`.bp7` SQLite files in `worksheets/`) with real fire-behavior data.
   - VMS layout (binary `layout.msgpack`, 50+ MB) as a real-world "large dataset" stress test.
   - Browser subscriptions (re-posh integration) exercising query performance under re-frame reactivity.

3. **Performance Wins Already Demonstrated**:
   - "Speed up VMS" commit (f2fddc2c) shows 40-75% latency reduction on query patterns by optimizing Datom construction and string escaping.
   - Bulk load optimization (`transactBulkString`) for initial VMS load.
   - Multi-window support eliminates per-tab re-initialization overhead.

### First Three Concrete Steps IN THIS REPO

**Step 1: Complete Posh Integration Parity & Write Regression Suite**
   - **File (rj-ds-rust branch only)**: Verify that `components/absurder_sql/` shadow-cljs config (`:test-kaocha` build) compiles and runs the kaocha+Chrome-CDP harness (`projects/behave/build/kaocha_hooks.clj`, ports 44220 funnel / 9222 Chrome debug).
   - **Command**: `clojure -M:kaocha` from `components/absurder_sql/` (verify it works on main first).
   - **Verification**: Test must pass all 280+ cases in `store_integration_test.cljs` when running against absurder_sql WASM.
   - **Deliverable**: Regression harness that runs pre-merge; any Posh subscription (re-frame `rf/subscribe [:query-name ...]`) must return identical results on DataScript vs absurder_sql.

**Step 2: Benchmark VMS Load Against DataScript Baseline**
   - **File**: Create a new test fixture at `components/absurder_sql/test/benchmarks/vms_load.cljs` that:
     - Loads `layout.msgpack` (via `clojure -X:download-vms` if needed into `resources/public/`).
     - Measures time to: (a) deserialize msgpack → datoms, (b) transact all datoms, (c) run 100 common VMS queries.
   - **Baseline**: Run same benchmark against in-memory DataScript (no SQLite).
   - **Success**: absurder_sql ≤ 2x latency of DataScript for VMS load (not faster, but not catastrophically slower); memory footprint ≤ 30% of RAM for 50+ MB msgpack.
   - **Output**: Structured result JSON: `{:datascript-ms 1200, :absurder-sql-ms 2000, :vms-msgpack-bytes 52428800, :queries-per-sec 420}`.

**Step 3: Integrate into Main Build & Test Against Real Worksheet**
   - **File**: Create a branch feature flag in `projects/behave/src/cljs/behave/store.cljs`:
     - Add `:dev` config key `use-rust-store?` (default false for now).
     - When true, instantiate absurder_sql WASM store instead of DataScript at `client init-store` time.
   - **Verification**: Load one real worksheet (e.g., `worksheets/BHP1-1226.bp7`, 925 KB) via `restore-conn` and run solver 5 times; verify outputs match golden (pre-run solver on main, commit outputs to `test/golden/BHP1-1226-runs.edn`).
   - **Deliverable**: Passing `projects/behave/src/cljs/behave/store_test.cljs::rust-store-parity-test` that exercises full solver→output→table flow.

### Falsifiable "You Have a Result" Milestone

**absurder_sql is production-ready when:**

1. **Functional parity**: The re-frame/re-posh subscription tests pass 100% (all 35+ deftests in `projects/behave/test/cljs/behave/test_runner.cljs` when store is absurder_sql).
2. **Performance parity**: VMS load benchmark shows absurder_sql within 2x of DataScript latency (acceptable overhead for persistence benefit).
3. **Solver correctness**: Parametric worksheet runs (e.g., 10-run Cartesian product) produce outputs identical to DataScript baseline (bit-exact numerics via golden CSV comparison).
4. **Merged and in use**: Branch `rj-ds-rust` is merged to main, feature flag is flipped to true by default, and CI test suite (`bb test:ci`) includes regression tests for both stores.
5. **Scalability validation**: A 5 MB dataset can be queried and re-rendered in <100 ms on a 2020-era browser (Chrome on MacBook Air M1).

---

## Frontier 2: Solver Performance & Scale — Parametric Cartesian Runs

### Why Current SOTA Falls Short

**Existing Behave6**:
- Single-point runs only; no built-in parametric sweep.
- Users manually copy-paste inputs multiple times.
- No mechanism to explore outcome sensitivity to input variation.

**Current Behave7**:
- Parametric runs are supported (comma-separated values in inputs generate Cartesian products via `generate-runs` in `projects/behave/src/cljs/behave/solver/generators.cljs`).
- **But**: No baseline performance measurements exist. Response time for 10-run, 100-run, 1000-run worksheets is unknown.
- **Bottleneck unknown**: Is it input parsing? WASM module initialization? Module sequencing? DataScript query overhead? Network round-trips?
- **No optimization story**: Impossible to prioritize solver rewrites without knowing which module dominates latency.

### This Repo's Unique Assets

1. **Parametric Run Generation Infrastructure** (Committed):
   - `generate-runs` function (generators.cljs lines 39-66) creates Cartesian products from CSV-formatted inputs.
   - Module sequencing is explicit and testable (solver/core.cljs lines 238-334 shows six modules: Surface → Crown → Contain → Mortality → Spot → Ignite).
   - Output linking already implemented (lines 157-192) to pass module outputs as downstream inputs.
   - Repeat-ID tracking for multi-run correlation.

2. **Rich Test Fixtures**:
   - 60+ real worksheets in `worksheets/` with up to 10+ parametric runs each (e.g., `30-min.bp7` 458 KB, `BHP1-1460-min-prod-rate.bp7` 868 KB).
   - Test metadata available: worksheet UUID, input-group structure, repeat counts.

3. **Chrome DevTools Integration** (Already working):
   - kaocha harness (projects/behave/build/kaocha_hooks.clj) launches Chrome with remote debugging on port 9222.
   - Can capture timeline profiles, memory snapshots, and async-stack traces for hotspot identification.

### First Three Concrete Steps IN THIS REPO

**Step 1: Establish Baseline Measurement Infrastructure**
   - **File**: Create `projects/behave/src/cljs/behave/diagnostics/solver_perf.cljs` with:
     - Function `measure-solver-run!` that wraps `solve-worksheet` and records: (a) wall-clock start→end time, (b) per-module latency (via `performance.mark`/`performance.measure`), (c) WASM module instance creation overhead.
     - Export telemetry as JSON: `{:total-ms 1250, :modules {:surface 400, :crown 300, :contain 250, :mortality 200, :spot 50, :ignite 50}, :run-count 10, :worksheet-uuid ...}`.
   - **Integration**: Dispatch `:diagnostics/record-perf` event after solver completes (re-frame event in `projects/behave/src/cljs/behave/events.cljs`).
   - **Verification**: Telemetry must be visible in browser console and storable to IndexedDB (via `worksheet/save-perf-log`).

**Step 2: Benchmark a Range of Parametric Scales**
   - **File**: Create a test in `projects/behave/test/cljs/behave/solver_perf_test.cljs` that:
     - Loads 5 real worksheets with varying run counts: (1-run, 5-run, 10-run, 50-run, 100-run).
     - For each, calls `measure-solver-run!` and records results.
     - Outputs a JSON file `target/solver-perf-results-YYYY-MM-DD.json` with rows: `[{:worksheet-id "BHP1-1226", :run-count 10, :total-ms 1250, :modules {...}}, ...]`.
   - **Baseline**: Run once on main, commit results to `test/golden/solver-perf-baseline-2026-07-06.json`.
   - **Success criterion**: Script runs without error; results show latency scaling (e.g., 10-run ~1.5s, 100-run ~12s, suggesting ~1.2x per-run overhead).
   - **Deliverable**: A reproducible perf-capture workflow: `bb perf:capture-solver 100` captures 100-run performance.

**Step 3: Profile Hotspots & Identify Optimization Targets**
   - **File**: Create `projects/behave/src/cljs/behave/diagnostics/solver_profile.cljs` that:
     - Wraps each module's `init`, `apply-inputs`, `run-fn`, and `get-outputs` with `performance.mark`.
     - Uses Chrome CDP (via kaocha harness) to trigger V8 sampling profiler during solve.
   - **Execution**: Run the profiler on a 10-run worksheet; save Chrome timeline profile to `target/solver-profile-10run.json`.
   - **Analysis**: Open profile in Chrome DevTools, identify top 3 hotspots by cumulative time (e.g., "WASM module init 40%", "DataScript query 30%", "input-map transformation 20%").
   - **Deliverable**: Markdown report at `doc/solver-profile-analysis-2026-07-06.md` listing:
     - Per-module breakdown (CPU time, GC pauses, allocations).
     - Top 3 candidate optimizations with estimated impact.
     - Example: "Crown module init is 2ms per run; reusing Module instance across runs could save 20ms on 10-run = 1.6% improvement."

### Falsifiable "You Have a Result" Milestone

**Solver performance frontier is advanced when:**

1. **Baseline established**: Performance telemetry is captured for all test worksheets; `target/solver-perf-baseline-2026-07-06.json` is committed and CI re-runs it on every merge to detect regressions (fail if latency > 10% slower).
2. **Hotspots identified**: Profile analysis document (`doc/solver-profile-analysis-*.md`) names the top 3 latency bottlenecks with measured %CPU, sorted by potential impact.
3. **Optimization targets prioritized**: Next step (not in this frontier) is a branch targeting the #1 hotspot (e.g., "memoize WASM module init" or "batch DataScript transacts").
4. **Scalability trend known**: Graph of run-count vs. latency shows either linear, sublinear, or superlinear scaling; confirmed in CI regression suite.
5. **Example candidate for next frontier**: If profiling shows "WASM init is 40% overhead," the next frontier is a spike to reuse/pool Module instances (blocked architecture decision: instantiate modules in solver vs. constructors, per variables_mapping.org tag #expose-or-new-solver).

---

## Frontier 3: Front-End App Performance — Startup, VMS Load, Subscription Latency

### Why Current SOTA Falls Short

**Perceived Slowness Indicators** (unconfirmed):
- App startup (blank screen to interactive UI) is subjectively "slow" (user feedback suggests 2-5 seconds typical).
- First time opening a large worksheet (e.g., multi-run) shows lag.
- Changing worksheet input triggers noticeable re-render delay (1-2 seconds).
- No instrumentation exists to distinguish: network delay, WASM module load, Datascript query, re-frame subscriptions, React render.

**Root Causes Suspected But Unmeasured**:
- VMS `layout.msgpack` (50+ MB) deserialization at app init.
- re-posh subscription overhead when re-frame graph is deep.
- WASM module initialization latency (behave-min.js + behave-min.wasm, ~1-3 MB combined).
- Browser GC pauses during large datom transacts.

### This Repo's Unique Assets

1. **"Speed up VMS" Work Already Started** (branch rj-ds-rust):
   - Commit f2fddc2c shows 40-75% latency reduction via:
     - Optimized Datom construction (min_for_ea, max_for_e helper methods).
     - Bulk string unescaping (unescape_bulk_str) to avoid per-datom allocations.
     - incremental tx-data application (applyTxData) instead of full re-index on each update.
   - Code is in production-ready form; just needs measurement + validation.

2. **Browser Perf APIs Available**:
   - Chrome DevTools Timeline can capture flamegraph of app startup.
   - `performance.mark`/`performance.measure` already used in test-runner.cljs for timing WASM load.
   - re-frame devtools middleware can log dispatch timestamps and subscription creation.

3. **Instrumentation Hooks Already in Place** (partial):
   - Solver diagnostics (diagnostics/solver_perf.cljs) can be extended for front-end use.
   - re-posh can hook transaction listeners to measure query latency.
   - test-runner.cljs already measures WASM init time (line 68 onwards).

### First Three Concrete Steps IN THIS REPO

**Step 1: Instrument App Startup Phases & Measure Each**
   - **File**: Create `projects/behave/src/cljs/behave/diagnostics/startup_perf.cljs` that marks:
     - `app-init-start` (entry to behave.core/-main or behave.client/init-app).
     - `wasm-load-start`, `wasm-load-end` (around behave-min.js Module factory call).
     - `vms-deserialize-start`, `vms-deserialize-end` (msgpack → datoms).
     - `vms-transact-start`, `vms-transact-end` (datascript/restore-conn).
     - `re-frame-init-start`, `re-frame-init-end` (rf/dispatch-sync setup).
     - `ui-render-start`, `ui-render-end` (React initial render to DOM).
     - `interactive` (first user input accepted, e.g., button clickable).
   - **Logging**: At each phase, log to console and IndexedDB: `{:phase "wasm-load", :elapsed-ms 450, :timestamp "2026-07-06T14:30:00Z"}`.
   - **Verification**: Open DevTools console, start app, observe marked intervals.
   - **Deliverable**: Passing test in `projects/behave/src/cljs/behave/diagnostics_test.cljs::startup-marks-test` that verifies all marks are recorded in order.

**Step 2: Capture Baseline Startup Profile (as of 2026-07-06)**
   - **File**: Create `projects/behave/bin/capture-startup-perf.sh`:
     - Launches headless Chrome with `--remote-debugging-port=9222`.
     - Navigates to app (server mode on port 9101).
     - Uses Chrome CDP to capture timeline trace (CPU, memory, events).
     - Exports trace as JSON to `target/startup-trace-TIMESTAMP.json`.
   - **Integration**: Add bb task `perf:capture-startup` that runs the script.
   - **Measurement**: Record key metrics in `target/startup-baseline-2026-07-06.json`:
     ```json
     {
       "timestamp": "2026-07-06T14:30:00Z",
       "app_mode": "server",
       "vms_msgpack_bytes": 52428800,
       "phases": {
         "wasm_load_ms": 450,
         "vms_deserialize_ms": 800,
         "vms_transact_ms": 1200,
         "re_frame_init_ms": 100,
         "ui_render_ms": 350,
         "interactive_ms": 3100
       },
       "total_startup_ms": 3100,
       "memory_peak_mb": 280
     }
     ```
   - **Success**: Baseline is captured and committed; repeatable (same machine, same build, ±5% variance).

**Step 3: Profile re-frame Subscriptions on Worksheet Input Change**
   - **File**: Create `projects/behave/src/cljs/behave/diagnostics/reframe_perf.cljs` that:
     - Instruments `rf/subscribe` to log creation time.
     - Instruments re-posh Datascript queries to measure latency.
     - On input change (e.g., `:worksheet/add-input` event), records all subscriptions created/updated and their latency.
   - **Test**: In `projects/behave/test/cljs/behave/reframe_perf_test.cljs`:
     - Load worksheet BHP1-1226 (925 KB, ~5000 input-groups).
     - Dispatch `:worksheet/add-input` with new value (simulating user edit).
     - Measure time from event dispatch to re-render complete.
     - Record latency for each subscription triggered: `[{:sub-name ":worksheet/all-inputs", :query-ms 45}, ...]`.
   - **Success criterion**: Median subscription latency ≤ 50 ms (subjective: <100 ms is acceptable for interactive feel).
   - **Deliverable**: JSON report `target/reframe-perf-BHP1-1226.json` with per-subscription latencies.

### Falsifiable "You Have a Result" Milestone

**Front-end performance frontier is advanced when:**

1. **Startup instrumented**: All startup phases are marked and logged; `bb perf:capture-startup` produces repeatable baseline (≤5% variance).
2. **Baseline meets or beats target**: Interactive app startup ≤ 4 seconds on a 2020-era machine (e.g., MacBook Air M1, Chrome 135+).
3. **VMS load optimized**: The rj-ds-rust "Speed up VMS" work is merged to main; VMS transact phase (step 2, "vms_transact_ms") is measured at ≤ 800 ms (vs. current suspected 1200+ ms).
4. **Subscription latency characterized**: Median re-frame subscription latency for worksheet input changes is ≤ 50 ms; 95th percentile ≤ 150 ms.
5. **Regression gate**: CI test suite includes `bb perf:regression-check` that fails if startup > baseline * 1.1 or subscription latency > baseline * 1.2.
6. **Next-step blocked**: If VMS deserialize or re-frame subscriptions are the bottleneck, the next frontier is either: (a) move VMS to worker thread (Web Worker), or (b) implement re-posh lazy subscriptions (only create queries for visible UI elements).

---

## Adjacent Territory: Not Owner-Prioritized, But Candidate Frontier

### Verified Numerical Parity vs. Behave6/FOFEM

**Status**: Candidate (no owner commitment, lower priority than the three main frontiers).

**Why It Matters**: Behave7 must produce numerically identical results to the original Behave6 desktop app (and within acceptable tolerance vs. FOFEM). Silent numerical drift is a business risk (fire management decisions depend on consistent modeling).

**Current State**:
- Mortality species coverage was a major issue (GACC-restricted 190-code set vs. Behave6's 525 codes); partially resolved by regenerating CSV from C++ testMortality/resultsProbMort.csv (commit cdfd3ce9).
- Crown damage (CRCABE) model is known inert (returns constant 1.5-3% regardless of inputs); C++ reference (FOFEM_input.tre) shows zero CRCABE rows, suggesting model is deprecated or incomplete in behave-mirror.
- No systematic reference dataset exists that compares Behave7 outputs to Behave6 or FOFEM on a standard fuel model × weather × terrain matrix.

**Concrete First Steps** (if prioritized):
1. **Create Golden Dataset**: Run Behave6 desktop app on a 5×5×5 matrix (5 fuel models, 5 weather profiles, 5 terrain slopes) and export all outputs to CSV. Commit as `test/golden/behave6-reference-outputs-5x5x5.csv`.
2. **Implement Diff Test**: Create `projects/behave/test/cljs/behave/numerics_test.cljs::behave6-parity-test` that runs the same inputs through Behave7 and compares outputs (allow ±0.1% tolerance for floating-point rounding).
3. **Address Failures**: For any failures, file issue in behave-mirror (C++ library) to align with Behave6 or FOFEM.

**Falsifiable Milestone**: "Numerical parity is achieved when 95% of test matrix outputs match Behave6 within ±0.1% relative error; remaining 5% are documented as deferred (e.g., CRCABE pending C++ fix)."

---

## When NOT to Use This Skill

This skill is for **research-frontier problems where the outcome is unknown and measurement is required to confirm value**. Use sibling skills for different needs:

| Goal | Use This Skill? | Use Instead |
|------|-----------------|-------------|
| Fix a known bug in solver (e.g., units arity or numerical drift) | No | `behave-failure-archaeology` (precedent + evidence) |
| Understand how to run the solver and load a worksheet | No | `behave-run-and-operate` (operational how-to) |
| Decide whether to merge absurder_sql; understand risks + gates | **Yes** | — |
| Build a performance benchmark to establish baseline | **Yes** | — |
| Understand why solver runs are slow in production | Maybe | `behave-debugging-playbook` (symptom → diagnosis) |
| Plan implementation of "memoize WASM init" optimization | **Yes (after frontier 2)** | — |
| Add a new input variable to the VMS | No | `behave-vms-variable-pipeline` (data pipeline) |
| Understand architecture of solver module sequencing | No | `behave-architecture-contract` (design invariants) |
| Deploy a new release | No | `behave-run-and-operate` (release checklist) |

---

## Provenance and Maintenance

**Verify these facts on every read (as of 2026-07-06)**:

- absurder_sql active development: `git log --oneline rj-ds-rust | head -1` should be recent (within 2 months). If HEAD is >6 months old, branch may be stale.
  
- rj-ds-rust commit count vs. main: `git log --oneline main..rj-ds-rust | wc -l` should be ≥ 50 (currently 53). If <20, major commits may have been pruned or merged.

- Solver module sequence: `grep -A 200 "defn solve-worksheet" projects/behave/src/cljs/behave/solver/core.cljs | grep -c "run-module"` should equal 6 (Surface, Crown, Contain, Mortality, Spot, Ignite). If module order changes, steps 2-3 of Frontier 2 need re-verification.

- Test fixture count: `ls worksheets/*.bp7 | wc -l` should be ≥ 50 (currently 50). If <30, fixtures may have been cleaned up; verify paths in Step 2 of Frontier 2.

- VMS layout size: After running `clojure -X:download-vms`, check `ls -lh projects/behave/resources/public/layout.msgpack | awk '{print $5}'` should show ≥ 40 MB. If not present, run the download command. Test fixture version in `target/kaocha-test/layout.msgpack` is smaller (~1-2 MB) and intentionally minimal.

- kaocha harness active: `git log --oneline projects/behave/build/kaocha_hooks.clj | head -1` should be within 1 year. If >2 years, kaocha infrastructure may have rotted.

- Performance baseline exists: `test -f projects/behave/target/solver-perf-baseline-2026-07-06.json && echo "exists"` should print "exists". If missing, Step 2 of Frontier 2 needs initial run.

- Chrome DevTools integration: `grep -r "remote-debugging-port" projects/behave/build/` should find kaocha_hooks.clj line 105. If missing, Chrome CDP profiling not available.

---

## Summary: Three Frontiers at a Glance

| # | Frontier | SOTA Gap | This Repo's Asset | Blocker → Next Step | Milestone |
|---|----------|----------|-------------------|---------------------|-----------|
| **1** | absurder_sql standalone Datalog | DataScript in-memory only; Datahike slow | 30+ commits, Rust core, 280+ integration tests | Test parity vs. DataScript; VMS load perf | 100% test pass, ≤2x latency, merged to main |
| **2** | Solver perf & scale (parametric) | No baselines; bottlenecks unknown | generate-runs already works; test worksheets available | Establish measurement baseline; profile hotspots | Baseline committed; top 3 optimizations identified |
| **3** | Front-end perf (startup/VMS/subs) | Subjective slowness; no instrumentation | "Speed up VMS" work on branch; Chrome CDP available | Instrument startup phases; capture profile | Interactive startup ≤4s; subscription latency ≤50ms median |

**All three frontiers are open and candidate**. Progress on any single frontier advances the state of the art for Datalog-on-SQLite engines, fire-behavior solver scalability, or browser-based scientific UI performance—all valuable beyond fire modeling.
