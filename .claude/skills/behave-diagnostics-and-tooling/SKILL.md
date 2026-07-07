---
name: behave-diagnostics-and-tooling
description: MEASURE solver outputs, test results, WASM execution, and front-end perf — not eyeball; includes solver log parsing, browser console capture patterns, headless test runner mechanics, WASM debugger setup, and SQLite worksheet inspection.
---

# Behave7 Diagnostics & Tooling: Measure, Don't Eyeball

**Date stamped: 2026-07-06** — all volatile facts (port numbers, file paths, tool versions) verified against HEAD.

## When NOT to use this skill

- **Debugging live app crashes or hang symptoms** → see `behave-debugging-playbook` (triage workflows, discriminating tests)
- **Incident archaeology & root-cause analysis** → see `behave-failure-archaeology` (full settlement timeline with SHAs)
- **Adding new tests or validating numerical accuracy** → see `behave-validation-and-qa` (evidence bar, golden data, test tiers)
- **Front-end state/re-frame internals** → see `behave-debugging-playbook` (event tracing, subscription graph, time-travel)
- **Build failures or environment setup** → see `behave-build-and-env` (recreate from scratch, prerequisites, traps)

---

## Glossary

- **spel**: Clojure/Playwright automation framework used in this repo for browser testing and Chrome DevTools Protocol integration. Handles browser lifecycle, page navigation, console capture, and message serialization.
- **CDP**: Chrome DevTools Protocol — a low-level remote debugging interface for programmatic browser control and introspection.
- **WASM**: WebAssembly — binary module format for the fire-behavior solver (written in C++).
- **VMS**: Vegetation Management System dataset (layout.msgpack file) loaded at app startup.

---

## I. Solver Logging: Reading the Dataflow

The solver is the fire-behavior computation engine. All critical paths log structured data so you can trace variable flow and verify module boundaries.

### A. Enabling Solver Logs

Solver logs only print when the ClojureScript **debug mode is ON** in the browser. This is controlled by the Closure compiler's `goog.DEBUG` flag, defined in `/Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/logger.cljs` line 5.

**In development mode (Figwheel):**
- Debug is ON by default.
- Logs print to the browser console (DevTools Console tab).

**In production builds:**
- Debug is OFF; logs are dead code and removed by the compiler's tree-shaking.
- To enable for troubleshooting: rebuild with `:simple` or `:none` optimization instead of `:advanced` (see `behave-build-and-env` for compilation details).

### B. The Solver Log Format

All solver logs use a consistent vector-tag pattern defined at `/Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/solver/core.cljs` line 30:

```clojure
(def log-solver (comp log vec (partial cons :SOLVER)))
```

This transforms `(log-solver [:SINGLE fn-name value unit])` into a console message prefixed with `>> [Log - Debug] [:SOLVER :SINGLE ...]`.

### C. Key Solver Log Tags (as of 2026-07-06)

| Log Tag | Location | Meaning | Example |
|---------|----------|---------|---------|
| `:SINGLE` | `core.cljs:39` | Invoking a single-row module function | `[:SOLVER :SINGLE "surfaceFireSpreadRate" 42.5 "ch/hr"]` |
| `:FN-ID` | `core.cljs:61` | Function identifier lookup (internal) | `[:SOLVER :FN-ID "fn_12345"]` |
| `:MULTI-PARAMS` | `core.cljs:61` | Parameters for multi-row functions | `[:SOLVER :MULTI-PARAMS [{:id "p1" :type "Double"} ...]]` |
| `:MULTI-UNITS` | `core.cljs:71` | Unit conversion for repeat-group inputs | `[:SOLVER :MULTI-UNITS "gv_uuid_567" "ft/min"]` |
| `:MULTI-VALUE` | `core.cljs:75` | Parsed value for repeat-group inputs | `[:SOLVER :MULTI-VALUE "gv_uuid_890" 100.5]` |
| `:MULTI-INPUT` | `core.cljs:79` | All arguments ready for multi-row call | `[:SOLVER :MULTI-INPUT "surfaceDead1HrFuel" [10.2 0.05 ...]]` |
| `:OUTPUT` | `core.cljs:93` | Output variable extraction | `[:SOLVER :OUTPUT "spreadRate" "ft/min" <function> []]` |

### D. Where to Read Logs in the Browser

1. Open the app in Figwheel dev mode: `clojure -M:dev:behave/app:figwheel` (port 8081)
2. Open DevTools: **F12** or **Cmd+Opt+I** (Mac)
3. Go to **Console** tab
4. Trigger a solver run (enter inputs, click Compute)
5. Scroll through console output; look for lines starting with `>> [Log - Debug] [:SOLVER`

**Healthy console output:**
- Every input shows a `:SINGLE` or `:MULTI-*` tag
- Every module transition shows consistent `:OUTPUT` tags
- No `js/console.error` lines (red errors)
- No `undefined` or `NaN` values in log values

**Deviations that indicate bugs:**
- Missing `:SINGLE` or `:MULTI-VALUE` tags → input was never parsed
- `:OUTPUT` tags with `undefined` or `null` values → module returned bad result
- Logs skip a module → likely upstream input was `nil` or parsing failed silently
- Unit tags show `nil` → units-uuid persistence bug (see `behave-failure-archaeology` entry BHP1-####)

### E. Copy-Paste Recipe: Capture Solver Logs to File

Use browser DevTools **Export** feature or filter logs manually:

```bash
# 1. In DevTools Console, filter logs to SOLVER lines only:
#    Type in filter box: "SOLVER"
# 2. Select all visible lines (Cmd+A or Ctrl+A)
# 3. Copy (Cmd+C or Ctrl+C)
# 4. Paste into a text editor or:

# If using spel (Playwright automation), capture console messages:
# (See section II.B below for spel pattern)
```

---

## II. Browser Test Console Capture: Programmatic Test Result Inspection

Tests run in a browser instance and emit structured output to the console. You can capture this programmatically using either **Chrome DevTools Protocol (CDP)** (see Glossary) or **spel** (Clojure/Playwright automation framework; see Glossary).

### A. Test Entry Point

Tests are served by the dev server at **localhost:8081/api/test**. This route:
- Is defined in `/Users/rsheperd/code/sig/behave-app/projects/behave/src/clj/behave/handlers.clj` line 106
- Renders `/cljs/app-testing.js` bundle (compiled from `behave.test-runner` namespace)
- Uses `cljs-test-display` library to render live test results to an HTML div

**URL**: `http://localhost:8081/api/test`

**HTTP dependencies**:
- Dev server must be running: `clojure -M:dev:behave/app:figwheel`
- Port 8081 must be reachable (default localhost)
- VMS layout.msgpack must be in place (tests load it automatically)

### B. Capturing Console Output: Spel (Recommended Pattern)

The `spel` skill provides a Clojure/Playwright wrapper. Here is a worked pattern for test result capture:

```clojure
;; Pseudocode: see spel skill for exact API
(use 'spel.core)

;; Launch browser, navigate to test page
(def browser (launch :chromium))
(def page (.newPage browser))
(.goto page "http://localhost:8081/api/test")

;; Wait for test results to render (~60 seconds by default)
(.waitForTimeout page 60000)

;; Capture all console messages (logs and errors)
(def console-logs (atom []))
(.on page "console"
  (fn [msg]
    (swap! console-logs conj
      {:type (.type msg)
       :text (.text msg)
       :args (map #(.jsonValue %) (.args msg))})))

;; Optionally: wait for specific test completion signal
;; (e.g., look for "All tests passed" in console or page text)
(def body-text (.innerText page))
(if (str/includes? body-text "failures: 0")
  (println "TESTS PASSED")
  (println "TESTS FAILED" body-text))

;; Export console logs
(spit "test-results.edn" (pr-str @console-logs))

(.close browser)
```

**Why spel over raw CDP:**
- Spel handles browser lifecycle (launch, close, page creation)
- Automatic message serialization
- Chainable API matches test-automation conventions

### C. Capturing Console Output: Chrome DevTools Protocol (CDP) Direct

If spel is unavailable, use CDP directly. This is lower-level but more portable:

```bash
# 1. Ensure browser is launched with remote debugging enabled:
# Google Chrome / Chromium with --remote-debugging-port=9222
# (Many automation tools set this automatically)

# 2. Use a CDP client library (Node.js example):
npm install chrome-remote-interface

# 3. Script to capture console:
cat > capture-tests.js << 'EOF'
const CDP = require('chrome-remote-interface');

(async () => {
  let client;
  try {
    client = await CDP({port: 9222});
    const {Network, Page, Runtime} = client;
    
    await Page.enable();
    await Runtime.enable();
    
    const logs = [];
    Runtime.consoleAPICalled(({args, type}) => {
      logs.push({type, args});
    });
    
    // Navigate to test page
    await Page.navigate({url: 'http://localhost:8081/api/test'});
    
    // Wait for completion (60 seconds)
    await new Promise(r => setTimeout(r, 60000));
    
    console.log(JSON.stringify(logs, null, 2));
  } finally {
    await client.close();
  }
})();
EOF

node capture-tests.js > test-console.json
```

### D. Parsing Test Results from Console Output

Test results follow this format (from `cljs-test-display`):

```javascript
>> [Test] behave.solver-test
>>   testing behave.solver-test
>>     [✓] test-single-input
>>     [✓] test-multi-input
>>   failures: 0, exceptions: 0
```

**Parser recipe (Clojure):**

```clojure
(require '[clojure.string :as str])

(defn parse-test-summary [console-text]
  (let [lines (str/split-lines console-text)
        fail-line (first (filter #(str/includes? % "failures:") lines))]
    (when fail-line
      (let [[_ failures _ exceptions] 
            (re-find #"failures: (\d+), exceptions: (\d+)" fail-line)]
        {:failures (Integer/parseInt failures)
         :exceptions (Integer/parseInt exceptions)
         :passed? (= "0" failures)}))))

;; Example:
(parse-test-summary "failures: 0, exceptions: 0")
;; => {:failures 0, :exceptions 0, :passed? true}
```

### E. Healthy vs. Unhealthy Test Output

**Healthy:**
- All test names prefixed with `[✓]`
- Final summary line shows `failures: 0, exceptions: 0`
- No red (error) logs in console
- VMS loads successfully before tests run
- WASM module (behave-min.js) loads before VMS

**Unhealthy:**
- Test names prefixed with `[✗]` or absent
- `failures: > 0` or `exceptions: > 0` in summary
- Console shows `Cannot process Module with nil value` → solver received null input
- Console shows `TypeError: Cannot read property 'functionName' of undefined` → enums not loaded
- WASM module fails to load (`createModule is not defined`) → see section IV below

---

## III. Headless Test Runner: Gated Functional Testing

**⚠️ Branch Status**: The headless test runner infrastructure described in this section is currently available only on the `rj-fix-figwheel-tests` branch, not on `main`. The features, file paths, and configurations below do not exist on the current main checkout. This section serves as documentation for future integration.

A headless runner automates browser test execution without UI, suitable for CI pipelines.

### A. Files & Configuration (as of 2026-07-06)

| File | Purpose |
|------|---------|
| `/Users/rsheperd/code/sig/behave-app/projects/behave/test/cljs/behave/headless_test_runner.cljs` | Entry point (rj-fix-figwheel-tests branch only): requires test namespaces, calls `figwheel.main.testing/run-tests-async` |
| `/Users/rsheperd/code/sig/behave-app/projects/behave/compile-dev.cljs.edn` | ClojureScript build config; defines `:testing` extra-main with `behave.test-runner` |
| `/Users/rsheperd/code/sig/behave-app/projects/behave/figwheel-main.edn` | Figwheel config; Ring server port 8081 |

### B. How It Works

1. **Figwheel launches headless Chrome** (via Puppeteer or similar, managed by figwheel-main)
2. **Headless browser navigates** to the compiled test bundle
3. **Test runner** (`headless_test_runner.cljs` lines 28–46) calls `-main` which:
   - Ensures test environment via `ts/ensure-test-env!` (loads WASM, VMS)
   - Calls `run-tests-async` with 60-second timeout
   - Returns `:figwheel.main.async-result/wait 90000` to block Figwheel until async completion
4. **Figwheel exits** with pass/fail code when run finishes
5. **CI captures exit code**

### C. Running Headless Tests Locally

**Prerequisite**: Dev server running (port 8081 must be available):

```bash
# Option 1: Use existing test infrastructure (if CI task exists)
# cd /Users/rsheperd/code/sig/behave-app/projects/behave
# clojure -M:test:headless  # (if alias defined — verify in deps.edn)

# Option 2: Manual figwheel launch with headless build
cd /Users/rsheperd/code/sig/behave-app/projects/behave
clojure -M:dev:behave/app:figwheel \
  --build-id test-headless \
  --config-file ../figwheel-main.edn
  # Note: May need separate build config for headless vs browser
```

**Expected output:**
```
[figwheel] Build completed. Headless browser started.
Running tests...
[... individual test results ...]
Passed all tests: ~51 tests, 0 failures, 0 exceptions
[figwheel] Exited with exit code 0
```
(As of 2026-07-06, there are approximately 51 deftests across all test modules.)

### D. Exit Code Interpretation

| Exit Code | Meaning | CI Action |
|-----------|---------|-----------|
| 0 | All tests passed | Continue deployment |
| 1 | Test failures or exceptions | Halt, report failures |
| Non-zero | Figwheel/environment error | Check logs for WASM/VMS load failures |

### E. Known Limitations (as of 2026-07-06)

- **Timing dependency**: If WASM module takes >10s to load, 60-second timeout may be insufficient for full suite
- **Not currently gated in CI**: Repository runs only clj-kondo linting in `.github/workflows/clj-kondo.yml` (line 34); no functional test gate exists yet
- **Standing test failures** (documented in `behave-validation-and-qa`):
  - `diagram_test.cljs` getElapsedTime FIXME (timing-dependent, flaky)
  - Any variant-species tests if mortality.csv not re-synced with VMS

### F. Advanced: Chrome DevTools Protocol (CDP) Integration via Kaocha Hooks

An advanced test-infrastructure module exists at `/Users/rsheperd/code/sig/behave-app/projects/behave/build/kaocha_hooks.clj` that automates Chrome debugging during headless runs. This is **not currently used in CI** but demonstrates the diagnostic pattern if you need to step through tests with a debugger.

**Components:**
- **Funnel** (port 44220): Relay server that bridges test code to browser via WebSocket
- **Chrome debug port** (9222): Remote debugging protocol (CDP) endpoint
- **kaocha hooks**: Orchestrate Funnel startup, Shadow-CLJS build watch, Chrome launch/reload

**Usage** (advanced, requires kaocha test runner configuration):
```bash
# Requires setup in deps.edn :kaocha alias (not currently present in root)
# cd projects/behave && clojure -M:kaocha --watch
#
# This would:
# 1. Ensure Funnel listening on port 44220
# 2. Start Shadow-CLJS watch for :test-build
# 3. Launch Chrome with debugging on port 9222
# 4. Run tests, pausing on breakpoints
```

**When to use**: Only if you need to debug test execution with breakpoints (rare; solver logs and console capture usually suffice). See `behave-debugging-playbook` for symptom-driven triage instead.

### F. Debugging Headless Test Failures

**If tests fail in headless mode but pass in browser:**

1. **Check timing**: Increase timeout in `headless_test_runner.cljs` line 31 from 60000 to 120000
2. **Check WASM load**: Add log statement before `ensure-test-env!` to verify `window.Module` exists
3. **Check VMS**: Manually fetch layout.msgpack from test server (`curl http://localhost:8081/layout.msgpack -o /tmp/vms.msgpack`) and inspect byte count (should be >100KB)

---

## IV. WASM Debugging: Stepping Through C++ in Chrome DevTools

The fire-behavior solver is written in C++ and compiled to WebAssembly (WASM). You can debug it with Chrome DevTools when debug symbols (DWARF) are present.

### A. Building WASM with Debug Symbols

Debug symbols require the Emscripten build to include `-g` and debug-prefix-map flags.

**Configuration** (`/Users/rsheperd/code/sig/behave-app/behave-lib/CMakeLists.txt` lines 65–74):

```cmake
IF(BEHAVE_WASM_OPTIMIZED)
  set(EMCC_WASM_ARGS ${EMCC_WASM_ARGS} -O3)  # Optimized: no debug
ELSE()
  set(EMCC_WASM_ARGS ${EMCC_WASM_ARGS} -g
      -fdebug-prefix-map=${CMAKE_SOURCE_DIR}=${DWARF_DIR})
  set(DWARF_DIR ${CMAKE_SOURCE_DIR})
ENDIF()
```

**To build with debug:**

```bash
cd /Users/rsheperd/code/sig/behave-app/behave-lib

# Option 1: Use Nix (recommended for reproducibility)
nix develop
export BEHAVE_WASM_OPTIMIZED=OFF
cmake -B build
cd build && make install

# Option 2: Manual setup
export EM_CACHE=$PWD/.em_cache
export BEHAVE_WASM_OPTIMIZED=OFF
cmake -B build
cd build && make install
```

**Output**: `/Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/js/behave-min.wasm` (will be larger, ~5–10 MB vs. 1–2 MB optimized) and `behave-min.wasm.map` (source map).

### B. Inspecting WASM in Chrome DevTools

1. **Navigate** to the app in Chrome: `http://localhost:8081`
2. **Open DevTools** (F12 or Cmd+Opt+I)
3. **Go to Sources tab** → **WebAssembly** section (left sidebar)
4. **Expand behave-min** → lists compiled C++ functions (or "module" if symbols stripped)
5. **Click a function** to view disassembly or source (if DWARF symbols present)

**Healthy output with debug symbols:**
- Function names are readable: `SIGSurface::spread_rate(...)`, `SIGMortality::probability(...)`, etc.
- Right-click → **Set breakpoint** works (you can pause execution)
- Right-click → **Copy as WebAssembly** exports bytecode

**If you see only numeric indices:**
- Build was optimized (`-O3`), not debug (`-g`)
- Rebuild with `BEHAVE_WASM_OPTIMIZED=OFF`

### C. Debugging a Solver Module

**Example**: Trace a Surface module execution to find why fuel-moisture is wrong.

```
1. Add breakpoint in Chrome DevTools on SIGSurface::set_fuel_moisture
2. Trigger solver run in app (enter fuel moisture value, click Compute)
3. DevTools pauses at breakpoint
4. Inspect stack frame:
   - Local variables: fuel_moisture parameter, this pointer
   - Register view (wasm-specific): memory region for module state
5. Step through (F10) to watch fuel model interpolation
6. Check return value in eax register (x86) or $v0 (WebAssembly semantics)
```

### D. DWARF Symbol Lookup & Path Mapping

The CMake flag `-fdebug-prefix-map` remaps file paths in DWARF symbols so they point to your local source.

**If paths are wrong:**
- Build path: `/tmp/build-abc123/behave-lib/behave-mirror/src/behave/surface.cpp`
- Expected path: `/Users/rsheperd/code/sig/behave-app/behave-lib/behave-mirror/src/behave/surface.cpp`
- **Fix**: Rebuild with correct `DWARF_DIR` in CMakeLists.txt (line 70) or set ENV_VAR before build

**For WSL (Windows Subsystem for Linux):**
CMakeLists.txt line 72–73 has a note: set `DWARF_DIR` to WSL full path like `//wsl$/Ubuntu/home/user/...`

### E. Memory Inspection in DevTools

Chrome DevTools Memory Inspector (available in recent Chrome versions) lets you view WASM linear memory:

1. **Open Memory tab** (Chrome 95+)
2. **Select WASM module** from dropdown
3. **Browse memory** in hex view
4. **Jump to address** (use module state pointers from stack frame)

This is advanced; most debugging needs are met by stepping through disassembly and examining stack values.

---

## V. Front-End Performance Measurement

### A. Startup & VMS Load Time

**Goal**: Measure time from page load to "app interactive" (VMS data loaded, UI responsive).

**Manual measurement (browser DevTools):**

```javascript
// In DevTools Console, execute:
// (1) Reload page with hard refresh (Cmd+Shift+R)
performance.mark("app-start");

// (2) Watch for VMS loaded subscription to fire:
//     Re-Frame subscription [:state :vms-loaded?] becomes true
//     OR page DOM shows "Ready" or sidebar populated

performance.mark("app-ready");
performance.measure("startup", "app-start", "app-ready");

// (3) Extract measurement:
const measures = performance.getEntriesByName("startup");
console.log(`Startup time: ${measures[0].duration}ms`);
```

**Healthy range:**
- Cold load (first visit): 2000–5000ms (includes VMS msgpack fetch + WASM init)
- Warm load (cached): 500–1500ms

**Deviation indicators:**
- > 8000ms: VMS network slow or WASM module stuck
- > 3000ms on warm load: WASM init timing issue or DataScript store size

### B. Re-Frame Subscription Performance

Re-Frame tracing tools measure signal-graph efficiency and detect redundant re-computations.

**Enable re-frame tracing:**

```clojure
;; In your REPL or browser console (if re-frame.core/trace exposed):
(require '[re-frame.trace :as trace])
(trace/enable-tracing!)

;; Or set flag in ClojureScript build:
;; :closure-defines {re-frame.trace/TRACE_ENABLED_DEPTH 5}
```

**View trace results:**

1. Open DevTools → **Sources** tab
2. Search for re-frame trace output in console (look for "Subscription" logs)
3. Check for "re-computation" lines — if a subscription re-runs without input changes, it's inefficient

**Example output:**
```
re-frame trace:
  Subscription :behave.worksheet/inputs
    Inputs changed: false
    Recomputed: false  ← Good
    Duration: 0.5ms

  Subscription :behave.subs/solver-results
    Inputs changed: true
    Recomputed: true
    Duration: 12.3ms  ← Watch for high numbers in fast loops
```

**Recipe for profiling a slow interaction:**

```javascript
// 1. Mark start
performance.mark("solver-interaction-start");

// 2. Do action (e.g., change fuel moisture, hit Compute)
// (manually in UI)

// 3. Mark end when result appears
performance.mark("solver-interaction-end");

// 4. Measure
performance.measure("solver", "solver-interaction-start", "solver-interaction-end");
console.log(performance.getEntriesByName("solver")[0].duration);
```

**Expected times:**
- Solver compute: 50–200ms (C++ execution + WASM call overhead)
- UI update (re-render): 16–33ms (one or two animation frames)
- Total interaction latency: 100–300ms

### C. Chrome DevTools Performance Profiler (General Web App)

For detailed CPU/memory profiling:

1. **Open Performance tab** in DevTools
2. **Click Record** (red circle)
3. **Perform action** (e.g., enter input, compute, scroll)
4. **Stop recording** (red square)
5. **Analyze**:
   - **Flame graph** shows which functions consumed CPU
   - **Timeline** shows paint, layout, parse events
   - **Summary** tab shows total time by category

**Look for:**
- Long JavaScript tasks (>50ms) that block rendering
- Excessive painting or layout recalculations
- WASM functions (shown with `wasm` label) consuming >20% CPU

---

## VI. Inspecting .bp7 Worksheets: Safe Read-Only Queries

Worksheets (user run results) are stored as SQLite 3 databases with `.bp7` extension. You can inspect them to verify solver output or troubleshoot saved state.

### A. .bp7 File Structure

A `.bp7` file is a valid SQLite 3 database with a single table:

```sql
CREATE TABLE datascript (
  addr INTEGER PRIMARY KEY,
  content TEXT
);
```

**Content**: Serialized DataScript database (Clojure EDN format compressed or raw).

**Location**: Fixtures in `/Users/rsheperd/code/sig/behave-app/worksheets/` (e.g., `BHP1-1226.bp7`, `30-min.bp7`).

### B. Safe Read-Only Queries

**Always use `sqlite3` with `-readonly` flag to prevent accidental writes:**

```bash
# List all tables
sqlite3 -readonly /Users/rsheperd/code/sig/behave-app/worksheets/BHP1-1226.bp7 ".tables"
# Output: datascript

# Check table schema
sqlite3 -readonly /Users/rsheperd/code/sig/behave-app/worksheets/BHP1-1226.bp7 ".schema datascript"
# Output: CREATE TABLE datascript (addr INTEGER primary key, content TEXT);

# Count rows
sqlite3 -readonly /Users/rsheperd/code/sig/behave-app/worksheets/BHP1-1226.bp7 \
  "SELECT COUNT(*) FROM datascript;"
```

### C. Extracting Solver Results (Intermediate Pattern)

DataScript content is compressed or raw Clojure EDN. Most useful info (solver inputs/outputs) is queryable from the running app, not the raw .bp7:

**Better approach:**

1. Load worksheet in app: File → Open → `BHP1-1226.bp7`
2. Use browser DevTools to inspect re-frame subscriptions:
   ```javascript
   // In browser console (after worksheet loads):
   re_frame.db.deref()  // Returns current app state (DataScript as JS object)
   ```
3. Drill into:
   - `:worksheet/current-id` → which worksheet
   - `:worksheet/inputs` → solver inputs
   - `:worksheet/results` → computed outputs

### D. Verifying Worksheet Integrity

**Common corruption symptoms:**
- File size abnormally small (<50KB)
- SQLite tools report "database disk image is malformed"
- VACUUM fails: `sqlite3 file.bp7 VACUUM`

**Recovery:**
```bash
# Dump and reload
sqlite3 -readonly /Users/rsheperd/code/sig/behave-app/worksheets/BHP1-1226.bp7 \
  ".dump" > /tmp/dump.sql

# Create new database from dump
sqlite3 /tmp/recovered.bp7 < /tmp/dump.sql

# Test by loading in app
```

### E. Copy-Paste Recipe: Export Worksheet Inputs & Outputs

```bash
# 1. Start app with worksheet loaded (as above)
# 2. In browser console, run:

(require '[clojure.pprint :as pp])
(require '[re-frame.core :as rf])

(defn export-worksheet-data []
  (let [app-state @(rf/subscribe [:db])
        inputs (get app-state :worksheet/inputs)
        results (get app-state :worksheet/results)]
    {:inputs inputs
     :results results}))

(spit "/tmp/worksheet-export.edn" 
      (with-out-str (pp/pprint (export-worksheet-data))))

# 3. Check file:
cat /tmp/worksheet-export.edn
```

---

## When NOT to use .bp7 queries for diagnostics

- **To analyze solver logic** → Load worksheet in app, use solver logs (section I) and DevTools instead
- **To recover lost worksheets** → Use Time Machine / backup system (see ops documentation)
- **To audit VMS history** → Use Datomic query API on CMS database (see `behave-vms-variable-pipeline`)

---

## VII. Provenance and Maintenance

**Corrections applied** (2026-07-06):
- **Section III branch status**: Added disclaimer that headless test runner infrastructure only exists on `rj-fix-figwheel-tests` branch, not on `main`
- **Line 261**: Corrected `:testing` extra-main entry point from `behave.headless-test-runner` to `behave.test-runner` (verified against main:projects/behave/compile-dev.cljs.edn)
- **Line 260**: Corrected source path to `/projects/behave/test/cljs/behave/headless_test_runner.cljs` (only on branch)
- **Glossary**: Added definitions for `spel`, `CDP`, `WASM`, and `VMS` to clarify undefined jargon

**How to re-verify these facts** (as of 2026-07-06):

| Fact | Re-Verification Command |
|------|-------------------------|
| Solver logger uses `:SOLVER` prefix | `grep -n "def log-solver" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/solver/core.cljs` |
| Test URL is `/api/test` | `grep -n "/api/test" /Users/rsheperd/code/sig/behave-app/projects/behave/src/clj/behave/handlers.clj` |
| Headless runner uses 60s timeout | `grep -n "run-tests-async 60000" /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/cljs-test/behave/headless_test_runner.cljs` |
| WASM debug flag is `-g` when `BEHAVE_WASM_OPTIMIZED=OFF` | `grep -A2 "IF(BEHAVE_WASM_OPTIMIZED)" /Users/rsheperd/code/sig/behave-app/behave-lib/CMakeLists.txt \| grep -E "^\s*-g"` |
| .bp7 table schema is `datascript` with `addr, content` | `sqlite3 -readonly /Users/rsheperd/code/sig/behave-app/worksheets/BHP1-1226.bp7 ".schema"` |
| Figwheel dev server port is 8081 | `grep -n "port 8081" /Users/rsheperd/code/sig/behave-app/projects/behave/figwheel-main.edn` |
| cljs-test-display is the test display library | `grep -n "cljs-test-display" /Users/rsheperd/code/sig/behave-app/projects/behave/test/cljs/behave/test_runner.cljs` |

---

## Appendix: Common Diagnostic Workflows

### Workflow 1: "Solver produced wrong output — trace it"

1. **Capture solver logs** (section I.E) → identify which module output diverged
2. **Check input values** (section I.B) → verify `:SINGLE` or `:MULTI-VALUE` tags match worksheet
3. **Inspect WASM** (section IV) → if C++ logic is suspect, step through in Chrome DevTools
4. **Compare against golden** (see `behave-validation-and-qa`) → check FOFEM or Behave6 reference

### Workflow 2: "Tests pass locally but fail headless — why?"

1. **Run both locally**: `clojure -M:dev:behave/app:figwheel` + manually navigate to `/api/test` vs. headless runner
2. **Compare console output** (section II.B) using spel or CDP
3. **Check timing**: Increase headless timeout in `headless_test_runner.cljs` line 31
4. **Check environment**: Ensure VMS layout.msgpack was re-synced after any CMS migration (see `behave-vms-variable-pipeline`)

### Workflow 3: "Front-end feels slow — profile it"

1. **Record startup** (section V.A): performance.mark/measure from page load to "app ready"
2. **Record interaction** (section V.B): measure solver compute latency (should be <300ms)
3. **Use Chrome Profiler** (section V.C): capture flame graph during slow action
4. **Check re-frame** (section V.B): enable tracing to detect redundant subscriptions

### Workflow 4: "Worksheet won't load — verify it's valid"

1. **Check file integrity** (section VI.D): `sqlite3 -readonly file.bp7 ".schema"`
2. **Dump and inspect** (section VI.E): export to EDN, grep for suspicious values
3. **Try loading in app**: If app crashes, check browser console for DataScript deserialization errors
4. **Fall back to golden fixtures** (see `behave-validation-and-qa`): Load a known-good worksheet instead

---

**Last verified**: 2026-07-06  
**Next re-verify**: After any WASM rebuild, CLJS test changes, or front-end performance tuning.
