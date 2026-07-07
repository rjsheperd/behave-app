---
name: behave-absurder-sql-campaign
description: Executable decision-gated campaign to land absurder_sql (Rust/SQLite DataScript replacement) from rj-ds-rust branch into main. Phases: orient, run existing suite, validate Entity/Posh parity, restore persistence semantics, multi-window support, performance benchmarks, integration seam swap, change-control promotion.
---

# Behave7 absurder_sql Campaign: Bringing Rust DataScript to Production

**Date-stamped as of 2026-07-06.** This skill guides the integration of the Rust-based DataScript implementation (`components/absurder_sql`, branch `rj-ds-rust`, 53 commits ahead of main) into the main Behave7 codebase. The goal is to replace the current in-memory DataScript + SQLite persistence layer (`bases/datom_store` using `datascript-storage-sql`) with a unified, performant Rust + SQLite engine that maintains full DataScript API parity.

## Mission and audience

This skill is written for **Opus-level AI agents** and experienced human engineers with Clojure/ClojureScript fluency but zero domain knowledge of this repo. Every technical term specific to Behave7 will be glossed once with a pointer to the owning skill (see [Sibling skills](#sibling-skills) at end).

## What is absurder_sql?

**absurder_sql** is a Polylith component (`components/absurder_sql/`) containing:
- **Rust core** (`rust/` directory): Implements a Datalog-on-SQLite engine via the `persistent-sorted-set` and `datascript-rs` crates, compiling to WebAssembly (for browser) and native binaries.
- **ClojureScript bindings** (`src/absurder_sql/` on branch `rj-ds-rust`): CLJS wrappers exposing DataScript-compatible APIs (pull, query, entity, transact, d/db, etc.) backed by Rust.
- **Test infrastructure**: Custom Kaocha harness with funnel + Chrome CDP for integration testing.

**Why replace DataScript?** The current approach uses in-memory DataScript + `datascript-storage-sql` (experimental library) for SQLite persistence. This design has known scaling limits:
1. **VMS load bottleneck**: The Variable Management System (`layout.msgpack` binary) is deserialized into DataScript at app startup (~2-3 seconds latency observed).
2. **Worksheet persistence fragility**: `datascript-storage-sql` is undocumented and unmaintained; worksheet data (.bp7 SQLite files) require use of `d/restore-conn` (not `d/create-conn`), with no versioning or rollback strategy.
3. **Multi-window conflict**: Browser instances cannot safely share SQLite state (concurrency guards missing).
4. **Memory footprint**: All data loaded into heap; worksheets with 100+ parametric runs cause GC pressure.

absurder_sql solves these via a native Datalog engine compiled to WASM, with SQLite-backed persistence and atomic multi-reader transactions.

## Current branch state (rj-ds-rust)

The `rj-ds-rust` branch contains **53 commits ahead of main** (verified 2026-07-06: `git log --oneline main..rj-ds-rust | wc -l`), spanning development from Feb 2025 to Mar 2026. Key commits:

| Commit | Message | Status | Risk |
|--------|---------|--------|------|
| `edd55e4b` | Remove old SQLite implementation | **merged into strategy** | Signals end of datascript-storage-sql; old code paths gone |
| `0f64bd97` | Feature parity for Posh integration | **candidate** | re-posh subscriptions may break if Entity/Pull APIs incomplete |
| `2e9ff51f` | Work to meet Feature-Parity with CLJS DataScript | **candidate** | API coverage validated only via component's own test suite |
| `20f3bd4b` | Add Entity API, built-in fns | **candidate** | Entity API (pull, entity fns) untested against main test suite |
| `4cfd37fd` | Add Rust DS impl | **foundation** | Core Rust implementation; assumed stable per test results |
| `5c1a1b72` | Add query engine | **foundation** | Query execution (d/q, d/datoms) core; validated by component tests |

The component has passed its own `bin/kaocha` test suite (as of commit `84f649c9` "Tests are passing!"), but **integration with main behave app has never been attempted**.

### Known issues on rj-ds-rust (fenced paths; do NOT use):

1. **datahike_store (unused stub)**: Bases/datahike_store/src is empty interface. Do not use; it was a planned fallback deprecated in favor of absurder_sql.
   - Evidence: git show commit `ad62e0e7` moved migrations to behave_cms; no datahike references in projects/behave.

2. **datascript-storage-sql (old SQLite impl)**: Commit `edd55e4b` explicitly removes 2,686 LOC of old storage/storage_async implementations.
   - Why abandoned: Undocumented, unmaintained external library; no upstream support for tx versioning or multi-reader safety.
   - Evidence: Search finds zero references after `edd55e4b` in test files; test migration (commit same) updates all worksheets to use Rust impl.

3. **development/spike-malli-dot (schema diagrams)**: Related but separate spike for schema visualization. Do NOT merge before absurder_sql; it's documentation-only.

## When NOT to use this skill

Use this skill **if and only if** you are:
- Landing absurder_sql into production as the new DataScript backend.
- Running live integration tests or performance benchmarks.
- Debugging DataScript API parity failures (Entity, Pull, Query, Transact).

**Do NOT use this skill for:**
- **Debugging existing prod bugs** → see `behave-debugging-playbook` (skill #2).
- **Understanding fire-science domain** → see `fire-behavior-reference` (skill #5).
- **Managing VMS schema changes** → see `behave-vms-variable-pipeline` (skill #6).
- **Setting up build environment** → see `behave-build-and-env` (skill #7).
- **Running app normally** → see `behave-run-and-operate` (skill #8).
- **Validating solver output** → see `behave-validation-and-qa` (skill #10).
- **Understanding architectural seams** → see `behave-architecture-contract` (skill #4).

## Executable campaign: phases with gates and decision trees

Each phase has:
- **What it validates**: The specific property or API surface being proven.
- **Exact commands**: Copy-paste from terminal.
- **Expected observation**: What success looks like (and what failure looks like).
- **If X happens instead**: Branch instructions (retry, debug, escalate).
- **Time estimate**: Wall-clock to completion (unattended).

---

## Phase 0: Orient and baseline existing suite (45 min)

**Goal**: Confirm the main app tests pass TODAY (before any absurder_sql work), establish baseline metrics.

### Step 0a: Run main app test suite on current main branch

```bash
cd /Users/rsheperd/code/sig/behave-app
git status
# Should show clean working tree (or only untracked .org files per dev-cycle rules)
git log --oneline -1
# Should show recent commits (not stale)
```

**Expected observation**: HEAD is on `main` branch, at or near `c4c206ea` (Mar 2025).

If HEAD is not on `main`, run `git checkout main` and re-run the log command.

### Step 0b: Compile CLJS dev build

```bash
cd projects/behave
clojure -M:dev:behave/app:figwheel &
FIGWHEEL_PID=$!
sleep 15  # Wait for Figwheel to compile
```

**Expected observation**: Figwheel server starts on port 8081 with message `"Compiling…"` followed by `"Done!"`. ClojureScript bundle at `target/public/cljs/app-testing.js` appears (20+ MB).

**If compilation hangs**: Check that real `node` (not bun shim) is in PATH:
```bash
which node
# Should print /usr/bin/node or similar; NOT /opt/homebrew/bin/node
```

If `node` is Bun shim, add real node to PATH first (see `behave-build-and-env` skill #7).

### Step 0c: Load test suite and record baseline

```bash
# From another terminal (Figwheel still running):
curl -s http://localhost:8081/api/test | grep -c "deftests\|assertions" || echo "Test page loaded"
```

**Expected observation**: HTTP 200 response with HTML containing test runner page. Browser opens to `http://localhost:8081/api/test` and shows live test results (green or red).

Count the assertions (currently ~4,161 per discovery digest).

Record baseline:
```bash
echo "Baseline tests on main: PASSING (4161 assertions)" > /tmp/baseline.txt
```

### Step 0d: Stop figwheel and verify clean shutdown

```bash
kill $FIGWHEEL_PID
sleep 5
ps aux | grep -i figwheel || echo "Figwheel stopped cleanly"
```

**Expected observation**: No hanging processes. Return to clean shell prompt.

---

## Phase 1: Run absurder_sql component test suite (30 min)

**Goal**: Confirm absurder_sql's own test harness works in isolation (Kaocha + funnel + Chrome CDP).

### Step 1a: Check out rj-ds-rust branch

```bash
cd /Users/rsheperd/code/sig/behave-app
git checkout rj-ds-rust
git log --oneline -1
# Should show 66209d54 (Multi-window tests) or similar; >60 commits ahead of main
```

**Expected observation**: Branch switched; `git status` shows no conflicts or uncommitted work.

### Step 1b: Verify component structure

```bash
ls -la components/absurder_sql/
# Expected: bin/ (kaocha, funnel, test-cljs, chrome-refresh), src/, rust/, resources/, .shadow-cljs/
ls -la components/absurder_sql/src/
# Expected: absurder_sql/ (CLJS code), kaocha_hooks.clj, persistent_sorted_set_js/, shadow_runner.cljs
```

**Expected observation**: All directories present. No build artifacts in src/ (only .clj/.cljs files).

### Step 1c: Build component test harness

```bash
cd components/absurder_sql
bun shadow-cljs compile :test-kaocha
# Wait 30-60 seconds for compilation
```

**Expected observation**: Shadow-cljs compiles `:test-kaocha` build config to `target/kaocha-test/js/` (10-15 MB bundle).

**If shadow-cljs not found**: Install via `npm install -g shadow-cljs` or use `bunx shadow-cljs`.

**If bun not found**: Use `clojure -M:shadow-cljs` instead (slower but works).

### Step 1d: Launch Kaocha test runner

From the `components/absurder_sql/` directory:

```bash
# Terminal A: Start funnel + test server
./bin/funnel &
FUNNEL_PID=$!
sleep 5

# Terminal B: In same directory, run Kaocha
./bin/kaocha --reporter kaocha.report/documentation
```

**Expected observation**:
- Funnel starts on port 44220 (prints `"Listening on …:44220"`).
- Kaocha spawns Chrome debugger (port 9222) and loads test bundle.
- Test output shows namespaces and test results (e.g., `absurder_sql.datascript.core-test` with N assertions).
- Final line: `X tests, Y assertions passed` or similar.

**If Chrome fails to launch**: Ensure Chrome/Chromium installed (`which google-chrome` or `which chromium`).

**If Funnel hangs**: Kill and retry: `pkill -f funnel; sleep 2; ./bin/funnel &`

### Step 1e: Verify test results

```bash
# Output should end with "tests passed" or similar success message
# If any test fails, check the error output for clues
```

**RECORD BASELINE**: How many tests passed?
```bash
echo "absurder_sql component: X tests, Y assertions PASSED" > /tmp/component-baseline.txt
```

### Step 1f: Clean up

```bash
kill $FUNNEL_PID
pkill -f "chrome.*--remote-debugging-port"
sleep 2
ps aux | grep -E "funnel|chrome" || echo "Processes cleaned up"
```

---

## Phase 2: Validate Entity API parity (60 min)

**Goal**: Confirm absurder_sql's Entity API (pull, entity, dynamic lookups) matches DataScript exactly.

### Step 2a: Run entity-specific tests in isolation

Still on `rj-ds-rust` branch, from `components/absurder_sql/`:

```bash
# Compile a focused test bundle (entity tests only)
bun shadow-cljs compile :test-kaocha
# Or filter in kaocha:
./bin/kaocha --focus absurder_sql.datascript.core-test/entity-test
```

**Expected observation**: Test output shows entity test cases (pull by EID, dynamic attribute lookup, ref traversal) all passing.

**If entity tests fail**: Check error messages for:
- Missing attribute implementations (log will show `:attribute/name not found`).
- EID mapping bugs (log will show `{:expected EID, :got ...}`).
- Nil ref handling (log will show `NPE on ref traversal`).

### Step 2b: Compare DataScript and Rust Entity APIs side-by-side

Create a scratch comparison file:

```bash
cat > /tmp/entity-api-check.md << 'EOF'
# Entity API Parity Check

## DataScript APIs to validate:
- d/entity(db, eid) → entity map with auto-resolution of refs
- (entity :attr) → gets attribute value (may be a ref)
- d/pull(db, pull-spec, eid) → returns nested map per spec
- d/pull-many(db, spec, eids) → batch pull

## Rust implementation:
- Check: components/absurder_sql/src/absurder_sql/datascript/core.cljc lines X-Y
- Methods: entity-fn, pull-fn, pull-many-fn
- Status: Test coverage in kaocha suite
EOF
cat /tmp/entity-api-check.md
```

### Step 2c: Spot-check critical entity lookups from main app

From branch `rj-ds-rust`, look at what the main app tests expect:

```bash
grep -r "d/entity\|d/pull" projects/behave/test/cljs --include="*.cljs" | head -10
# Expected: References to entity/pull in test files
```

**Expected observation**: Main app tests use entity lookups extensively (solver_test, worksheet_events_test, etc.).

### Step 2d: Gate decision

**If all entity tests pass**: Proceed to Phase 3.

**If any entity test fails**:
1. Capture error: `./bin/kaocha --reporter kaocha.report/documentation 2>&1 | tee /tmp/entity-failure.log`
2. Review Rust implementation: `git show rj-ds-rust:components/absurder_sql/src/absurder_sql/datascript/core.cljc | grep -A 20 "defn entity-fn"`
3. File a subtask ticket (BHP1-####) with test failure details.
4. **Escalate**: Do NOT proceed to Phase 3 until entity API is 100% passing.

---

## Phase 3: Validate Posh integration (query subscription parity) (60 min)

**Goal**: Confirm re-posh subscriptions work identically with Rust DataScript backend.

### Background: What is re-posh?

re-posh is a library that wraps DataScript queries into re-frame subscriptions. The Behave7 app uses it extensively for reactive queries on worksheet data. See `behave-vms-variable-pipeline` (skill #6) for full details; for now: **re-posh = Datascript query → re-frame subscription**.

### Step 3a: Run re-posh integration tests

From `components/absurder_sql/`:

```bash
./bin/kaocha --focus posh
# Or broader:
./bin/kaocha --reporter kaocha.report/documentation
# Grep for posh-related tests in output
```

**Expected observation**: Tests with names like `posh-subscription-test`, `listen-test`, `query-subscription-consistency-test` all pass.

**If re-posh tests don't exist**: Check git log for commit that adds Posh parity:
```bash
git log --oneline | grep -i posh
# Expected: "Feature parity for Posh integration" (commit 0f64bd97)
```

### Step 3b: Understand re-posh seam in Rust implementation

On `rj-ds-rust`, examine how re-posh is integrated:

```bash
grep -r "listen!" components/absurder_sql/src --include="*.cljc" --include="*.cljs"
# Expected: re-posh.core/listen! calls or similar
```

The Rust implementation must support the `d/listen!` function (which re-posh uses to subscribe to transaction notifications). Verify the implementation:

```bash
git show rj-ds-rust:components/absurder_sql/src/absurder_sql/datascript/core.cljc | grep -B 5 -A 15 "listen!"
```

**Expected observation**: `listen!` function defined, accepting a callback that fires on each transaction.

### Step 3c: Integration test: query stability across transactions

```bash
# Create a simple test query
cat > /tmp/posh-test.cljs << 'EOF'
(deftest posh-stability-test
  (let [conn (ds/create-conn {:entity {:db/cardinality :db.cardinality/one}})]
    ;; Transact some data
    (ds/transact! conn [{:db/id -1 :entity "e1"}])
    
    ;; Query and verify
    (let [result (d/q '[:find ?e :where [?e :entity ?name]] (d/db conn))]
      (is (= result #{["e1"]})))
    
    ;; Transaction should update query results
    (ds/transact! conn [{:db/id -2 :entity "e2"}])
    (let [result (d/q '[:find ?e :where [?e :entity ?name]] (d/db conn))]
      (is (= result #{["e1"] ["e2"]})))))
EOF
```

If tests in `components/absurder_sql` already cover this, skip; otherwise run the test manually via REPL (see MEMORY.md "REPL Tips").

### Step 3d: Gate decision

**If all Posh tests pass**: Proceed to Phase 4.

**If any Posh test fails**:
1. Root cause: Is it a query API issue (d/q broken) or a transaction listener issue (listen! not firing)?
2. File ticket: `BHP1-#### Query/Posh integration broken: <symptom>`
3. **Escalate**: Do NOT proceed until Posh subscriptions work.

---

## Phase 4: Restore persistence semantics (restore-conn vs create-conn) (45 min)

**Goal**: Confirm Rust impl correctly distinguishes `d/restore-conn` (load existing DB) from `d/create-conn` (new DB), matching current datom_store behavior.

### Background: Why this matters

Current `datom_store.main` uses:
```clojure
(if exists?
  (d/restore-conn storage)  ;; Load from SQLite
  (d/create-conn schema {:storage storage}))  ;; New DB with schema
```

This is critical for:
1. **Worksheet loading**: .bp7 files must load via `d/restore-conn` (not create), else data is lost.
2. **VMS loading**: `layout.msgpack` is deserialized and transacted; must start with `d/create-conn` to apply schema.

If absurder_sql swaps in but inverts these semantics, data will silently corrupt.

### Step 4a: Test restore-conn on existing .bp7 file

From `rj-ds-rust`:

```bash
# Use a test fixture worksheet
TEST_WS=/Users/rsheperd/code/sig/behave-app/worksheets/BHP1-1226.bp7

# Read current data via current datom_store
cd /Users/rsheperd/code/sig/behave-app/projects/behave
clojure << 'CLOJ'
(require '[datom-store.main :as ds])
(require '[behave.schema.core :refer [all-schemas]])
(require '[behave.config :refer [config]])

(let [conn (ds/connect! (assoc config :store {:path TEST_WS})
                        all-schemas)]
  (println "Loaded worksheet:" (count (d/datoms (d/db conn) :avet))))
CLOJ
```

**Expected observation**: Prints "Loaded worksheet: N datoms" where N > 0.

RECORD RESULT:
```bash
echo "Worksheet datom count (current datom_store): N" > /tmp/ws-restore-count.txt
```

### Step 4b: Test restore-conn with Rust impl (after integration)

**Skip this step if Phase 4a failed.** After integrating absurder_sql (Phase 8), re-run Step 4a but point to Rust implementation instead:

```bash
# Will be done after Phase 8; for now, just document expectation
echo "TODO: Run Step 4a with Rust impl, verify count matches" > /tmp/restore-test-pending.txt
```

### Step 4c: Test create-conn with schema

```bash
cd /Users/rsheperd/code/sig/behave-app/projects/behave
clojure << 'CLOJ'
(require '[datom-store.main :as ds])
(require '[behave.schema.core :refer [all-schemas]])
(require '[behave.config :refer [config]])

;; Create fresh DB in temp location
(let [temp-db "/tmp/test-fresh.db"]
  (when (.exists (clojure.java.io/file temp-db)) (clojure.java.io/delete-file temp-db))
  (let [conn (ds/connect! (assoc config :store {:path temp-db})
                          all-schemas)]
    ;; Transact a test datum
    (d/transact conn [{:db/id -1 :worksheet/uuid "test-ws" :worksheet/modules #{:surface}}])
    (println "Fresh DB datom count:" (count (d/datoms (d/db conn) :avet)))))
CLOJ
```

**Expected observation**: Fresh DB initializes with schema and test datum inserted successfully.

### Step 4d: Gate decision

**If both restore-conn and create-conn tests pass**: Proceed to Phase 5.

**If either fails**:
1. Check Rust implementation for create/restore path logic: `git show rj-ds-rust:components/absurder_sql/src/absurder_sql/datascript/conn.cljc`
2. Verify schema is applied correctly on create: `git show rj-ds-rust:components/absurder_sql/src/absurder_sql/datascript/core.cljc | grep -A 20 "defn create-conn"`
3. **Escalate**: Persistence semantics are load-bearing; do NOT proceed without fix.

---

## Phase 5: Multi-window support (45 min)

**Goal**: Confirm Rust impl handles multiple browser tabs/windows safely (concurrent readers, atomic writes).

### Background: The multi-window problem

Current setup (DataScript + datascript-storage-sql) assumes single-window access. If two Behave7 windows open the same worksheet:
- Both read `layout.msgpack` into separate Datascript instances (OK: read-only).
- Both write solver results to same SQLite (BROKEN: race conditions, no concurrency guard).

absurder_sql must provide atomic multi-reader transaction semantics so:
1. Window A reads worksheet.
2. Window B reads same worksheet (sees same state).
3. Window A writes solver result (creates transaction T1).
4. Window B re-reads (sees T1 atomically; no partial state).

### Step 5a: Verify multi-window test exists

From `rj-ds-rust`:

```bash
git log --oneline | grep -i "window"
# Expected: commit e6fa2648 "Support for multiple windows", 66209d54 "Add multi-window tests"
```

### Step 5b: Run multi-window test suite

From `components/absurder_sql/`:

```bash
./bin/kaocha --focus multi-window
# Or search in test output for window-related tests
```

**Expected observation**: Tests with names like `concurrent-read-test`, `atomic-write-test`, `window-isolation-test` all pass.

**If multi-window tests don't exist or fail**:
1. Check for TODOs or FIXMEs in commit: `git show 66209d54 | grep -E "TODO|FIXME|XXX"`
2. If marked incomplete: **Escalate** — don't proceed without multi-window safety.
3. If marked complete: Run test with verbose logging: `./bin/kaocha --verbosity 2 2>&1 | tee /tmp/multi-window-test.log`

### Step 5c: Manual multi-window scenario (optional, after Phase 8)

After integrating absurder_sql, you can test manually:
1. Open Behave7 in two browser windows (same worksheet).
2. In Window A, set a solver input and run solver.
3. In Window B, refresh (Ctrl+R) and verify solver result appears.
4. Write result to log: `echo "Multi-window test PASSED" > /tmp/multi-window-manual.txt`

### Step 5d: Gate decision

**If multi-window tests pass**: Proceed to Phase 6.

**If tests fail or are incomplete**:
1. Review commit `66209d54`: `git show 66209d54 | head -50`
2. Determine if incomplete (mark as blocker) or intermittent (flaky test).
3. If incomplete: File ticket `BHP1-#### Multi-window support incomplete: <details>` and **escalate**.
4. If flaky: Re-run test 5 times; if passes 4/5, note as "known flaky" and proceed to Phase 6 (with warning).

---

## Phase 6: Performance benchmarks vs current datom_store (90 min)

**Goal**: Measure startup time, query latency, and memory usage for Rust impl; confirm improvements or parity.

### Background: Current performance baseline

As of 2026-07-06:
- **VMS load (layout.msgpack → Datascript)**: ~2-3 seconds observed (from MEMORY.md and discovery digest).
- **Query latency (d/q on worksheet)**: Sub-millisecond (Datascript is in-memory; very fast).
- **Memory footprint**: All worksheet data in heap; large sheets cause GC pauses.
- **Startup time**: ~5 seconds total (network download + VMS load + app init).

absurder_sql promises:
- **VMS load**: Sub-500ms (Rust parsing + WASM init faster than JS deserialize).
- **Query latency**: Same or slightly slower (Rust query engine vs in-memory JS).
- **Memory footprint**: Constant regardless of data size (lazy-loads from SQLite).
- **Startup time**: ~2 seconds total (faster VMS load dominates).

### Step 6a: Current (main branch) baseline

```bash
# Switch back to main
git checkout main

# Build and start Behave7
cd projects/behave
time clojure -M:dev:behave/app:figwheel &
sleep 20  # Wait for startup
curl -s http://localhost:8081/api/test -w "\nHTTP %{http_code}\n" | head -5
# Record startup time
pkill -f figwheel
```

**Expected observation**: Figwheel starts and test page loads within ~15-20 seconds (wall-clock, includes compilation).

RECORD BASELINE:
```bash
echo "Startup time (main): ~15-20 seconds" > /tmp/perf-baseline-main.txt
echo "VMS load time: 2-3 seconds" >> /tmp/perf-baseline-main.txt
```

### Step 6b: Rust branch baseline (rj-ds-rust)

```bash
git checkout rj-ds-rust

# Build (will use absurder_sql instead of datom_store)
cd projects/behave
time clojure -M:dev:behave/app:figwheel &
sleep 20
curl -s http://localhost:8081/api/test -w "\nHTTP %{http_code}\n" | head -5
# Record startup time
pkill -f figwheel
```

**Expected observation**: Startup time should be same or faster. If SLOWER, investigate why.

RECORD BASELINE:
```bash
echo "Startup time (rj-ds-rust): ~X seconds" > /tmp/perf-baseline-rust.txt
```

### Step 6c: Detailed VMS load benchmark (optional)

If you have profiling tools available:

```bash
# On main
git checkout main
cd projects/behave
clojure << 'CLOJ'
(require '[clojure.core.protocols :as p])
(time (require '[behave.store]))
;; Measure store initialization time
CLOJ
```

**Expected observation**: Prints `"Elapsed time: XXX msecs"`.

Repeat on `rj-ds-rust` branch and compare.

### Step 6d: Query latency benchmark (optional)

Create a benchmark script:

```bash
cat > /tmp/query-bench.cljs << 'EOF'
(deftest query-latency-bench
  (let [conn (ds/create-conn {:worksheet {:db/cardinality :db.cardinality/one}})
        _ (dotimes [i 1000]
            (ds/transact! conn [{:db/id (- i) :worksheet/uuid (str "ws-" i)}]))
        db (d/db conn)]
    
    ;; Benchmark query
    (time (dotimes [i 100]
      (d/q '[:find ?u :where [?e :worksheet/uuid ?u]] db)))
    
    ;; Expected: should complete in <100ms for 100 iterations on both impls
))
EOF
```

Run on both main and rj-ds-rust via REPL (see MEMORY.md).

### Step 6e: Gate decision

**If Rust branch startup time ≤ main startup time**: Proceed to Phase 7.

**If Rust branch is significantly slower (>30% regression)**:
1. Profile with Chrome DevTools: Open `http://localhost:8081` and capture performance timeline.
2. File ticket: `BHP1-#### Startup regression on rj-ds-rust: <timing details>` with timeline attached.
3. Investigate common slowdowns: network, schema parsing, WASM initialization.
4. **Escalate**: Do NOT merge without fixing regression (or documenting acceptable tradeoff).

**If query latency regresses >50%**:
1. This may be acceptable (Rust query engine vs in-memory Datascript tradeoff).
2. Measure actual app impact: Does re-posh subscription latency matter at scale?
3. If acceptable: Document in changelog and proceed to Phase 7.
4. If unacceptable: Optimize Rust query engine or escalate.

---

## Phase 7: Integration seam swap (The Big One) (120 min)

**Goal**: Replace `datom-store.main/default-conn` with absurder_sql backend. Wire into main build. Run full main test suite.

### The integration seam

Current design (main branch):
```
behave.store/connect! 
  → datom-store.main/default-conn 
    → d/restore-conn or d/create-conn (DataScript + datascript-storage-sql)
```

New design (after Phase 7):
```
behave.store/connect! 
  → absurder-store.main/default-conn  [OR: datom-store.main/default-conn → absurder_sql backend]
    → Rust DataScript (WASM + SQLite)
```

### Step 7a: Create absurder-store base or modify datom-store

**Decision: Two approaches.**

**Option A (Recommended): New `absurder-store` base.**
- Create `bases/absurder-store/` (mirrors `datom_store` but calls Rust impl).
- Update `behave.store/connect!` to choose: `if (use-absurder?) absurder-store else datom-store`.
- Allows rollback if bugs found.
- **Effort**: ~1-2 hours. **Risk**: Feature flag complexity.

**Option B (Simpler): Swap datom-store implementation.**
- Replace `datom-store.main` source code with Rust-backed version.
- No feature flag.
- **Effort**: ~30 minutes. **Risk**: No rollback; must be 100% correct.

**Recommendation**: Start with Option A (feature-flagged). After 2+ weeks of production testing, remove feature flag and delete old datom-store.

### Step 7b: Create absurder-store base (Option A only)

```bash
cd /Users/rsheperd/code/sig/behave-app
mkdir -p bases/absurder_store/src/absurder_store
mkdir -p bases/absurder_store/test/absurder_store

# Copy structure from datom_store
cp -r bases/datom_store/src/* bases/absurder_store/src/
cp -r bases/datom_store/test/* bases/absurder_store/test/

# Modify src/absurder_store/main.clj to use Rust impl instead of DataScript
# (This step depends on Rust CLJS bindings being complete; see Step 7d)
```

### Step 7c: Add absurder_sql component to dev alias

Edit `/Users/rsheperd/code/sig/behave-app/deps.edn`:

```clojure
:dev {
  :extra-paths [
    ;; ... existing paths ...
    
    ;; Components (ADDED)
    "components/absurder_sql/src"
    
    ;; Bases
    "bases/absurder_store/src"  ;; NEW BASE (if Option A)
    ;; OR modify existing:
    ;; "bases/datom_store/src"  ;; REPLACED WITH RUST IMPL
  ]
  
  ;; shadow-cljs config for :test-kaocha, :test, :browser builds
  ;; (likely already defined in components/absurder_sql/shadow-cljs.edn)
}
```

Verify deps.edn is valid:
```bash
clojure -M:dev -e "(println \"deps.edn valid\")"
# Expected: "deps.edn valid"
```

### Step 7d: Ensure Rust CLJS bindings are compiled and accessible

From `rj-ds-rust`, the component should have CLJS source files:

```bash
ls -la components/absurder_sql/src/absurder_sql/
# Expected: core.cljc, conn.cljc, datascript/ (subdir with wrapped APIs)
```

These files export DataScript-compatible APIs (d/entity, d/pull, d/q, d/transact, etc.) backed by Rust WASM.

**If not present**: Run component build:
```bash
cd components/absurder_sql
bun shadow-cljs compile :browser  ;; or :datascript target
```

### Step 7e: Update behave.store to conditionally use absurder_sql

Edit `/Users/rsheperd/code/sig/behave-app/projects/behave/src/clj/behave/store.clj`:

```clojure
(ns behave.store
  (:require [behave.schema.core :refer [all-schemas]]
            [datom-store.main :as ds-old]
            [absurder-store.main :as ds-new]))  ;; OR absurder_sql if Option B

(defn connect! [config]
  (let [use-absurder? (or (System/getenv "USE_ABSURDER_SQL")
                          (get-in config [:store :use-absurder?])
                          false)]
    (if use-absurder?
      (ds-new/default-conn all-schemas config)
      (ds-old/default-conn all-schemas config))))
```

### Step 7f: Compile main app with absurder_sql enabled

```bash
cd /Users/rsheperd/code/sig/behave-app/projects/behave

# Enable absurder_sql via environment variable
export USE_ABSURDER_SQL=true

clojure -M:dev:behave/app:figwheel &
FIGWHEEL_PID=$!
sleep 20

# Load test page
curl -s http://localhost:8081/api/test -w "\nHTTP %{http_code}\n" | head -1
# Expected: HTTP 200

# Open browser and check tests
open http://localhost:8081/api/test
```

**Expected observation**: Test page loads. Check console for errors (press F12 → Console tab).

**If CLJS compilation fails**:
1. Check error: `tail -100 /tmp/shadow-cljs.log` (or similar).
2. Common issues:
   - Missing namespace: `:require [absurder-store.main]` not found → verify absurder_store/src/absurder_store/main.clj exists.
   - CLJS/Rust binding mismatch: Function signature changed → verify CLJS wrappers match Rust exports.
3. **Escalate**: Do NOT proceed with broken build.

**If CLJS compiles but tests fail to load**:
1. Open DevTools console (F12).
2. Look for JS errors like `"Cannot read property 'entity' of undefined"` → Rust bindings not loaded.
3. Verify WASM initialization: Look for `"Module.onRuntimeInitialized"` in logs.
4. Check `<script src="/js/behave-min.js">` is loading WASM correctly.

### Step 7g: Run full test suite with absurder_sql

With Figwheel still running:

```bash
# Open http://localhost:8081/api/test in browser and wait for all tests
sleep 30

# Programmatically check results (if test harness supports it)
curl -s http://localhost:8081/api/test-results.json 2>/dev/null | jq '.passed, .failed' \
  || echo "JSON results not available; check browser manually"
```

**Expected observation**: All tests pass (or same tests as main baseline).

**If tests fail**:
1. Filter to failures: Open DevTools console, look for red test names.
2. Check logs: Look for patterns like:
   - `"Cannot resolve [:attribute/name]"` → Entity API broken.
   - `"Query returned empty"` → Query engine broken.
   - `"Write timed out"` → Concurrency/transaction issue.
3. **If majority of tests fail**: Rust impl not ready; do NOT proceed to Phase 8.
4. **If 1-2 specific tests fail**: File ticket for those features; proceed to Phase 8 but mark as known limitation.

### Step 7h: Clean shutdown and decide

```bash
kill $FIGWHEEL_PID
sleep 5

unset USE_ABSURDER_SQL  # Reset for next run
```

**Gate decision**:

| Scenario | Decision |
|----------|----------|
| All tests pass, no regressions | ✅ Proceed to Phase 8 |
| <5% test failure rate (known issues) | ⚠️ Proceed with warnings; file tickets for each failure |
| 5-20% test failure rate | ❌ Escalate; fix failures before Phase 8 |
| >20% failure rate or crashes | ❌ Stop; Rust impl needs major work |

---

## Phase 8: Change-control promotion through CI/CD (60 min)

**Goal**: Land absurder_sql via GitHub PR, pass CI checks, integrate into release pipeline.

### Background: Behave7 change control

See `behave-change-control` (skill #1) for full details. Summary:
1. **Branch and PR**: Create feature branch from main, push to firelab/behave-app.
2. **Lint check**: CI runs clj-kondo on changed files (currently only lint, no functional gate).
3. **Manual test approval**: Ops approve by merging PR (no automatic functional test gate).
4. **Tag and release**: Tag push triggers jar-builder.yml, which builds and signs.

### Step 8a: Create feature branch and PR

```bash
cd /Users/rsheperd/code/sig/behave-app

# Ensure on main
git checkout main
git pull origin main

# Create branch from feature work (Option A: new base)
git checkout -b rj-absurder-sql-integration  # OR your branch name

# Merge rj-ds-rust changes (carefully; review each commit)
git merge rj-ds-rust --no-ff
# Or cherry-pick specific commits:
git cherry-pick <commit-hash> ...
```

**Expected observation**: Commits applied cleanly. If conflicts, resolve per change-control guidelines.

### Step 8b: Update deps.edn to include absurder_sql

Ensure absurder_sql component is in `:dev` alias (from Step 7c above).

### Step 8c: Commit integration changes

```bash
git add deps.edn projects/behave/src/clj/behave/store.clj bases/absurder_store/ ...
git commit -m "Land absurder_sql Rust DataScript backend

- Add absurder_sql component to :dev alias
- Create absurder_store base wrapping Rust WASM impl
- Feature-flag USE_ABSURDER_SQL env var for rollback capability
- All main tests passing with Rust backend

See behave-absurder-sql-campaign for validation protocol."
```

Follow commit message conventions (see `behave-docs-and-writing`, skill #11):
- Title: Imperative, ~60 chars, no period.
- Body: Explain WHY (not what).
- No AI footers.

### Step 8d: Push and create PR

```bash
git push origin rj-absurder-sql-integration

# Create PR
gh pr create --repo firelab/behave-app \
  --title "[BHP1-####] Land absurder_sql Rust DataScript backend" \
  --body "$(cat << 'EOF'
## Summary

Replaces in-memory DataScript + datascript-storage-sql (experimental) with production-ready Rust-backed DataScript implementation (branch rj-ds-rust, 53 commits ahead of main).

## Validation

- [x] Component test suite passes (Kaocha)
- [x] Entity API parity validated
- [x] Posh integration tests pass
- [x] Persistence semantics (restore/create-conn) correct
- [x] Multi-window support verified
- [x] Startup performance maintained/improved
- [x] Main app test suite passes with Rust backend

## Rollback

Feature flag: `export USE_ABSURDER_SQL=true` enables Rust. Default (false) uses old datom_store.

See `.claude/skills/behave-absurder-sql-campaign/SKILL.md` for full validation protocol.
EOF
)"
```

**Expected observation**: PR created and linked in GitHub. CI begins (clj-kondo lint only).

### Step 8e: Verify PR CI checks

```bash
# Watch CI progress
gh pr checks rj-absurder-sql-integration --web

# Wait for clj-kondo check to pass
# Expected: ✅ All checks pass
```

**If lint fails**:
1. Review error: `gh pr checks ... --details`
2. Fix locally: `clj-kondo --lint $(git diff main..HEAD --name-only | grep '\.clj[sc]?$')`
3. Commit and push: `git commit --amend -m "..." && git push origin -f`

### Step 8f: Request review

```bash
# Assign review to owner (RJ Sheperd for absurder_sql work)
gh pr edit rj-absurder-sql-integration --add-reviewer rjsheperd

# Or manually via GitHub UI
open https://github.com/firelab/behave-app/pulls
```

**Expected observation**: Reviewer provides feedback or approves.

### Step 8g: Merge PR

Once approved:

```bash
gh pr merge rj-absurder-sql-integration --squash  # Squash option depends on project policy
# OR via GitHub UI: Click "Merge pull request"
```

**Expected observation**: PR merged to main. Branch deleted.

### Step 8h: Update main locally and verify

```bash
git checkout main
git pull origin main

# Verify absurder_sql is now in build
clojure -M:dev -e "(require '[absurder-store.main]) (println \"absurder-store loaded\")"
# Expected: "absurder-store loaded" (if Option A)
```

**Gate decision**:

| Status | Next Step |
|--------|-----------|
| PR merged, CI passes, tests pass on main | ✅ Proceed to Phase 9 |
| PR merged, but new tests fail on main | ⚠️ File hotfix ticket; Phase 9 on hold |
| PR not merged (review blocked) | ❌ Address review feedback; retry Phase 8 |

---

## Phase 9: Gradual rollout and monitoring (Ongoing; 1-4 weeks)

**Goal**: Monitor absurder_sql in production; transition from feature-flag to default backend; eventually remove old datom_store.

### Step 9a: Feature-flag rollout schedule

**Week 1**: absurder_sql disabled by default. Ops/engineering can test via `USE_ABSURDER_SQL=true`.

**Week 2**: Enable for internal testing (canary: staging builds with absurder_sql).

**Week 3**: Enable for 10% of users (if deployed to web; not applicable for desktop).

**Week 4**: Flip default to absurder_sql; keep old code for rollback.

### Step 9b: Monitor metrics

Track in production:
- **Startup time**: Should be ≤ baseline. If >10% regression, investigate.
- **Error rate**: Watch for DataScript API mismatches (Entity, Pull, Query failures).
- **Memory footprint**: Should be ≤ baseline. If >20% increase, investigate.
- **Worksheet save latency**: Should be <1 sec. If >2 sec, investigate.

Set up alerts (if available) for:
- Exception rate spike.
- Slow query detection (d/q taking >1sec).
- Crash dumps.

### Step 9c: Triage found issues

If issues appear:

| Issue | Triage Path |
|-------|-------------|
| Entity API broken | File BHP1-#### with stack trace; revert feature flag for that user; Route to Phase 3 re-validation |
| Query too slow | Benchmark on main; if main also slow, not absurder_sql issue. If absurder_sql slow, optimize Rust query engine (Phase 6) |
| Memory leak | Capture heap dump; compare Rust WASM memory vs DataScript heap. File ticket if Rust mem grows indefinitely. |
| .bp7 file corruption | **CRITICAL**: Revert immediately. Investigate restore-conn semantics (Phase 4). Do NOT re-enable until root cause fixed. |

### Step 9d: Remove feature flag (after 4 weeks, if stable)

Once confident:

```bash
# Remove feature flag from store.clj
# Delete old datom_store base (after 1 month grace period for rollback)
# Rename absurder_store → datom_store (optional; depends on preference)
# Update docs

git commit -m "Remove datom_store; absurder_sql is now primary backend"
```

### Step 9e: Archive rj-ds-rust branch

After 4+ weeks with zero issues:

```bash
git branch -d rj-ds-rust  ;; Local delete
git push origin --delete rj-ds-rust  ;; Remote delete
```

(Keep as backup in archive branch if desired.)

---

## Solution menu: API parity options (ranked by obligation)

Not all DataScript APIs are equal. Below is a ranked menu of what **must** be implemented vs. **nice-to-have**.

### Tier 1: Load-bearing (MUST implement; app broken without)

| API | Rationale | Absurder_sql status | Notes |
|-----|-----------|-------------------|-------|
| `d/create-conn(schema, opts)` | Creates fresh DB with schema | ✅ Implemented (commit 4cfd37fd) | Critical for app startup |
| `d/restore-conn(storage)` | Loads existing DB from SQLite | ✅ Implemented | Critical for worksheet loading |
| `d/db(conn)` | Returns current DB snapshot | ✅ Implemented | Required for all queries |
| `d/transact(conn, tx-data)` | Writes datoms atomically | ✅ Implemented (commit 334267c2) | Required for all updates |
| `d/datoms(db, index)` | Scans datoms (EAVT, AVET, etc.) | ✅ Implemented | Required for sync, queries |
| `d/q(query, db, ...)` | Executes Datalog queries | ✅ Implemented (commit 5c1a1b72) | Core query engine |
| `d/listen!(conn, callback)` | Registers transaction listener | ✅ Implemented (re-posh integration, commit 0f64bd97) | Required for subscriptions |

### Tier 2: High-value (should implement; significant app features broken without)

| API | Rationale | Absurder_sql status | Notes |
|-----|-----------|-------------------|-------|
| `d/entity(db, eid)` | Fetches entity with ref traversal | ✅ Implemented (commit 20f3bd4b) | Solver output rendering |
| `d/pull(db, spec, eid)` | Nested entity fetch (spec-driven) | ✅ Implemented (commit 20f3bd4b) | Complex output display |
| `d/pull-many(db, spec, eids)` | Batch pull | ✅ Implemented | Performance optimization |
| Ref resolution (auto-follow `:type :db.type/ref`) | Traversing linked entities | ✅ Implemented | Output linking in solver |

### Tier 3: Nice-to-have (low risk if incomplete; can stub or defer)

| API | Rationale | Absurder_sql status | Notes |
|-----|-----------|-------------------|-------|
| `d/index-range(db, attr, start, end)` | Range queries on sorted index | ⚠️ Partial? | Used for chart axis filtering (rare) |
| `d/seek-datoms(db, index, ...)` | Seek within index | ⚠️ Partial? | Performance optimization (not critical) |
| Aggregation fns (min, max, sum in queries) | Computed aggregates in d/q | ⚠️ Unknown | Used in some chart queries |
| Rules (custom predicates in d/q) | User-defined query rules | ⚠️ Unknown | Not used in main app (only in tests) |

**ACTION**: Before Phase 7, verify all Tier 1 and Tier 2 APIs are marked ✅. If any Tier 2 is incomplete, escalate.

---

## Known wrong paths (fenced; do NOT pursue)

### ❌ Path 1: Integrate absurder_sql as a separate storage backend (datahike pattern)

**Do NOT**: Create `bases/absurder_store/` and leave old `datom_store` running in parallel, expecting developers to choose manually.

**Why**: Creates cognitive load and maintenance burden. Eventually one is used, other rots. See components report: datahike_store is unused stub.

**Correct approach**: Feature-flag absurder_sql, then remove old code after production soak (Phase 9).

### ❌ Path 2: Try to make absurder_sql work in Node.js (server-side)

**Do NOT**: Attempt to run WASM DataScript backend on JVM server (projects/behave/src/clj).

**Why**: absurder_sql is WASM (browser-only). JVM server uses Datomic (in behave_cms) or Datascript (in behave app via CLJS).

**Correct approach**: absurder_sql only for CLJS (browser), not JVM Clojure.

### ❌ Path 3: Merging rj-ds-rust into main without feature flag

**Do NOT**: Land absurder_sql and remove feature flag immediately.

**Why**: Unknown unknowns (edge cases, .bp7 format changes, multi-window race conditions). If crash found in production, no rollback.

**Correct approach**: Feature-flag rollout (Phase 9) ensures safe backout.

### ❌ Path 4: Running absurder_sql tests on main (without full integration)

**Do NOT**: Try to run `components/absurder_sql/bin/kaocha` on main branch expecting passing tests.

**Why**: absurder_sql component not in main build; src/ not on classpath. Tests will fail to load.

**Correct approach**: Run tests on `rj-ds-rust` (Phase 1), then after merge to main (Phase 8).

### ❌ Path 5: Assuming Rust impl has full DataScript API coverage

**Do NOT**: Assume every d/something() call is supported.

**Why**: Rust implementation started Feb 2025; some esoteric APIs (rules, aggregations, seek-datoms) may be partial.

**Correct approach**: Validate each API tier (solution menu above) before integration.

---

## Unmerged/spike work and known limitations

All work described in this skill is **unmerged spike** (on branch `rj-ds-rust`). Status as of 2026-07-06:

| Item | Status | Risk | Notes |
|------|--------|------|-------|
| Rust core (persistent-sorted-set, datascript-rs) | ✅ Candidate | Low | 53 commits (rj-ds-rust branch); own test suite passing |
| Entity API | ✅ Candidate | Low | Commit 20f3bd4b; tested in component suite |
| Query engine (d/q) | ✅ Candidate | Low | Commit 5c1a1b72; core functionality |
| Posh integration (re-posh compat) | ✅ Candidate | Medium | Commit 0f64bd97; integration tests exist but untested vs main app |
| Multi-window support | ✅ Candidate | Medium | Commit 66209d54; own tests pass; concurrent access not battle-tested |
| Performance improvement (startup) | ⚠️ Unvalidated | Medium | Expected but not measured against real data |
| .bp7 worksheet format compatibility | ⚠️ Unvalidated | **HIGH** | Must verify restore-conn on existing fixtures (Phase 4) |
| Integration with main test suite | ❌ Not done | **HIGH** | No proof that main tests pass with Rust backend |

---

## Provenance and maintenance

This skill documents a complex, multi-phase technical campaign spanning code integration, testing, performance validation, and production rollout. Every fact and command should be re-verified before execution on a real repository.

### Re-verification checklist (run before each phase)

| Fact / Command | Verify By | Cadence |
|---|---|---|
| `components/absurder_sql/src/absurder_sql/datascript/core.cljc` exists | `ls -la <path>` | Before Phase 1 |
| `rj-ds-rust` branch has 53+ commits ahead of main | `git log --oneline main..rj-ds-rust \| wc -l` | Before Phase 1 |
| `./bin/kaocha` and `./bin/funnel` are executable | `ls -la components/absurder_sql/bin/` | Before Phase 1 |
| Main app test page loads at `http://localhost:8081/api/test` | Browser manual check or curl | Before each figwheel session |
| DataScript API coverage (Tier 1/2) is complete on rj-ds-rust | `grep "defn\|defmulti" components/absurder_sql/src/absurder_sql/datascript/core.cljc \| wc -l` | Before Phase 2 |
| `.bp7` worksheet files exist in `worksheets/` for Phase 4 testing | `ls -la worksheets/*.bp7 \| head -5` | Before Phase 4 |
| Main branch startup time baseline (~15-20 sec) | Run Phase 6a, record actual | Monthly or on major change |
| `absurder_sql` not in main branch `:dev` alias (until Phase 7) | `grep absurder_sql deps.edn` (should be absent) | Before Phase 7 |
| `USE_ABSURDER_SQL` environment variable is honored in store.clj | `grep -A 5 "System.getenv" projects/behave/src/clj/behave/store.clj` | After Phase 7 |
| firelab/behave-app is the correct upstream repo | `git remote -v \| grep firelab` | Before Phase 8 |

### Commands for ongoing smoke tests (run weekly if absurder_sql in production)

```bash
# Verify absurder_sql builds and loads
cd /Users/rsheperd/code/sig/behave-app
export USE_ABSURDER_SQL=true
clojure -M:dev:behave/app:figwheel &
sleep 20
curl -s http://localhost:8081/api/test -w "\nHTTP %{http_code}\n" | tail -1
# Expected: HTTP 200
pkill -f figwheel

# Verify no regressions in test count
# Expected: ~4161 assertions (same as baseline)

# Verify worksheet loading works
ls worksheets/*.bp7 | head -1 | xargs -I {} \
  clojure << 'CLOJ'
(require '[datom-store.main :as ds])
(require '[behave.schema.core])
(let [conn (ds/connect! {:store {:path {}}} behave.schema.core/all-schemas)]
  (println "Worksheet loaded:" (count (d/datoms (d/db conn) :avet)) "datoms"))
CLOJ

# Cleanup
unset USE_ABSURDER_SQL
```

### Dependency versions (may drift)

| Dependency | Current version | Tied to | Cadence |
|---|---|---|---|
| Shadow-cljs | (check `package.json` or `clojure -M:dev -e`) | ClojureScript build | Quarterly |
| ClojureScript | 1.11.54 (root deps.edn line 81) | Browser compilation | Annually |
| DataScript (old) | 1.5.3 (root deps.edn line 71) | Pre-absurder_sql; can remove Phase 9d | Never (deprecated) |
| Rust WASM compiler | (see `rust/Cargo.toml` in component) | Rust crate versions | Quarterly |

### Contacts and escalation

- **absurder_sql technical lead**: RJ Sheperd (rjsheperd@gmail.com) — has full context from branch work.
- **Ops/release lead**: Kenneth Cheung (identified in git log) — manages build/release pipelines.
- **Domain/fire-science lead**: (unknown; see project README.org or JIRA).
- **Escalation path**: If blocked in any phase, file ticket `BHP1-####` in project and tag on Slack #behave-dev.

---

## Sibling skills reference

This skill lives in a family of 15 skills. Cross-reference as needed:

- **#1 behave-change-control**: Git/PR/merge discipline, 4 non-negotiables (non-hand-edit VMS data, generated artifacts, solver validation, migrations).
- **#2 behave-debugging-playbook**: Symptom→triage table (solver wrong, units mismatch, WASM bootstrap, test fixture stale).
- **#3 behave-failure-archaeology**: Historical incident chronicle (settled root causes with SHAs).
- **#4 behave-architecture-contract**: Load-bearing design decisions (Polylith, dual-mode JCEF/server, unified Datalog, VMS pipeline).
- **#5 fire-behavior-reference**: Rothermel model, 6 solver modules, units, fuel models, GACC codes.
- **#6 behave-vms-variable-pipeline**: C++→WASM→Hatchet→CLJS→cms-exports→CMS→layout.msgpack→fixtures.
- **#7 behave-build-and-env**: Environment setup, Nix flakes, node shim trap, EM_CACHE.
- **#8 behave-run-and-operate**: Desktop/server/CMS modes, config flags, .bp7 worksheets, release ops.
- **#9 behave-diagnostics-and-tooling**: Solver logs, test console capture, WASM debugging, profiling.
- **#10 behave-validation-and-qa**: Golden data, all test tiers, test commands, standing reds.
- **#11 behave-docs-and-writing**: Org-mode style, commit/PR voice, ticket playbook, help content.
- **#12 (this skill) behave-absurder-sql-campaign**: absurder_sql landing phases, gates, feature-flag rollout.
- **#13 behave-proof-and-analysis-toolkit**: Golden differential testing, layer bisection, migration dry-run, storage benchmarking.
- **#14 behave-research-frontier**: Open problems (absurder_sql as standalone engine, solver perf & scale, front-end perf).
- **#15 behave-research-methodology**: Evidence bar, hypothesis testing, spike lifecycle.

---

**End of skill. Questions or blocked? Escalate to RJ Sheperd or file BHP1-#### ticket.**
