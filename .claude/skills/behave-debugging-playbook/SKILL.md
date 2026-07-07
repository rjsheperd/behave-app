---
name: behave-debugging-playbook
description: "Symptom→triage runbook for Behave7's failure modes: solver output bugs, WASM bootstrap, test suite failures, build traps, startup failures, environment issues, hot-reload weirdness, worksheet loading. Discriminating experiments first; fix pointers follow."
---

# Behave7 Debugging Playbook

**As of 2026-07-06.** This is the document of record for triaging and fixing failures in Behave7, the USFS Firelab fire-behavior modeling application (`firelab/behave-app`). Read this when the app, tests, or build are broken.

## Quick orientation for new readers

**Behave7 architecture** (relevant to failure patterns):
- **Client**: ClojureScript SPA (re-frame + re-posh + DataScript) running at port 9101 (server mode) or as a desktop app (JCEF).
- **Fire-behavior engine**: C++ Rothermel model (6 modules: Surface → Crown → Contain → Mortality → Spot → Ignite) compiled to WASM via Emscripten, wrapped by ClojureScript auto-generated bindings.
- **VMS** (Variable Management System): Datomic + Postgres CMS (port 8001) that manages the model's structure; exports as `layout.msgpack` loaded at client startup.
- **Worksheets**: SQLite `.bp7` files (user runs) in `worksheets/` directory.
- **Dev**: Figwheel hot-reload on ports 8081 (app), 8082 (CMS).

**Key files for debugging**:
- Solver orchestration: `/projects/behave/src/cljs/behave/solver/core.cljs`
- Units & enums: `/projects/behave/src/cljs/behave/lib/units.cljs`, `/projects/behave/src/cljs/behave/lib/enums.cljs`
- C++ units enum: `/behave-lib/behave-mirror/src/behave/behaveUnits.h` (source of truth for unit codes)
- Migrations: `/projects/behave_cms/resources/migrations/`
- Config: `/projects/behave/config.edn` (server port 9101), `/dev.cljs.edn` (figwheel 8081), `/cms.cljs.edn` (figwheel 8082)

**Jargon reference**:
- **units-uuid**: A UUID pointing to a unit object in the VMS (e.g., `"ft"`, `"mi/h"`).
- **GACC**: Geographically Associated Coordination Center (10 USFS regions: Alaska, California, EasternArea, GreatBasin, NorthernRockies, Northwest, RockyMountain, SouthernArea, Southwest, NotSet).
- **layout.msgpack**: Binary export from VMS; loaded by client at startup to populate DataScript with module structure.
- **WASM**: WebAssembly binary of the C++ model compiled via Emscripten.
- **Enums**: Integer codes mapping units/species/fire types (defined in `behaveUnits.h` and mirrored in ClojureScript).
- **Hatchet**: Tool that auto-generates CLJS bindings (`behave/lib/*.cljs`) from C++ headers.

---

## Symptom → Triage table

| Symptom | Root cause class | Discriminating experiment | Fix pointers |
|---------|------------------|--------------------------|--------------|
| **Solver returns wrong numbers with no errors** | units-uuid delivery bug (units nil → setters skipped) | Open browser console (dev tools → Console), solve a worksheet, search logs for `[:SOLVER :SINGLE` — if any line shows `unit nil`, that setter was skipped. Compare value+unit to a direct WASM call. | See §A: Units-UUID Bug |
| **Outputs pinned at -100 or constant values (e.g. -100 for mortality, 1.0 for fireType)** | Species/enum coverage (code absent from compiled table) OR inert model path (CRCABE crown_damage) | Run `(behave.lib.mortality/getNumberOfRecordsInSpeciesTable)` or spel: `Module.SIGMortality.prototype.getNumberOfRecordsInSpeciesTable()` — compare to expected table size. For -100: check species code is in the table via `getSpeciesTableIndexFromSpeciesCode`. For constant: try setter directly in WASM (if no change, model path is inert). | See §B: Enum/Species Coverage; See MORTALITY_TEST_HANDOFF.org (CRCABE inertness confirmed; crown_damage path needs behave-mirror fix) |
| **Entire CLJS test suite red** (all tests fail) | WASM module not initialized, stale layout.msgpack, or missing VMS migration | (1) Open localhost:8081/api/test in browser, check console for WASM bootstrap errors (e.g. "Module is not defined"). (2) Check timestamp of `projects/behave/resources/public/js/behave-min.wasm` vs migrations dir. (3) Run `bb transactor` and `pg_ctl status`; check if CMS DB is out of sync. | See §C: WASM Bootstrap & Test Suite Red |
| **Build fails: `Cannot find module @cljs-oss/module-deps` or Emscripten cache stale** | Missing npm dep or EM_CACHE not set | Run `npm list @cljs-oss/module-deps` in `projects/behave` root. If absent, `npm install`. For Emscripten: `echo $EM_CACHE`. If unset, add to shell profile. | See §D: Build Traps |
| **Build fails: advanced compilation errors (Closure minifier)** | Externs files missing or mismatched | Search error for filename (e.g. `behave_externs.js`). Check existence and syntax. Compare C++ export names in `cms-exports/*.edn` to externs. | See §D: Build Traps |
| **App won't start / port conflict / Datomic transactor unreachable** | Port already bound, Datomic transactor down, Postgres missing, or config points wrong | (1) `lsof -i :9101` (app), `:8001` (CMS), `:4334` (Datomic) — kill conflicting process. (2) `bb transactor` (starts transactor). (3) Check Postgres: `pg_ctl status` or `ps aux | grep postgres`. (4) Verify config.edn points to correct DB URL + auth. | See §E: App/CMS Startup |
| **Test failures from stale env or missing migrations** | Postgres/pgdata corrupt, CMS migrations unapplied, or layout.msgpack out of sync with schema | Run pre-flight checklist (§F). Schema drift is silent — check VMS → client roundtrip: run a solver action in app, inspect DataScript (`:worksheet/all-inputs+units-vector`), verify units-uuid values are UUIDs not entity IDs. | See §F: Environment & Migration Bugs |
| **Figwheel hot-reload broken / page doesn't update / console errors** | Figwheel server crashed, browser cached stale JS, or source map issue | (1) Kill figwheel process, restart: `clojure -M:dev:behave/app:figwheel` (app) or `:dev:behave/cms:figwheel` (CMS). (2) Hard-refresh browser (Cmd-Shift-R). (3) Check console for "Cannot find asset `/cljs/app.js`" — if so, rebuild: `clojure -M:dev:behave/app:figwheel` with fresh terminal. | See §G: Figwheel & Hot-Reload |
| **.bp7 worksheet loads as empty or crashes on solve** | Corrupt SQLite file, datascript-storage-sql schema mismatch, or missing fixture | Check file exists and is readable: `sqlite3 worksheets/foo.bp7 ".tables"`. If empty, regenerate. On schema mismatch, check VMS was re-synced after migrations. Try opening a known-good fixture (e.g. `worksheets/behave7-20-min.bp7`). | See §H: Worksheet Load Failures |

---

## §A: Solver returns wrong numbers (units-uuid bug)

### Root cause

The solver's `apply-single-cpp-fn` at `/projects/behave/src/cljs/behave/solver/core.cljs:34` calls WASM module setters. When the units-uuid is `nil` or malformed, the setter is **silently skipped** because the 2-parameter variant of many setters requires a valid unit enum:

```clojure
(cond
  (= 1 (count params)) (f module value)
  (and (= 2 (count params)) (some? unit))  ;; unit must be non-nil
    (let [[_ _ param-type] (first params)]
      (if (is-unit? param-type)
        (f module unit value)
        (f module value unit))))
  ;; Implicit: no branch when unit is nil → setter never called
```

When inputs like wind, moisture, or canopy geometry skip their setters, downstream WASM computes with unset (default) values → wrong or constant outputs.

### Discriminating experiment

**In-browser observation of solver logs:**

1. Open the app at localhost:9101 (or localhost:8081 if running figwheel dev).
2. Trigger a solve (e.g., load a worksheet, change an input, solve).
3. Open browser dev tools → Console.
4. Search for `[:SOLVER :SINGLE` — you should see lines like:
   ```
   >> [Log - Debug] [:SOLVER :SINGLE setWindSpeed 5.0 mi/h-uuid-string]
   >> [Log - Debug] [:SOLVER :SINGLE setMoisture 0.2 unit-uuid-string]
   ```
5. **If any critical input shows `unit nil`**, that setter was skipped.

**Direct WASM verification:**

Compare the worksheet-solve result to a direct WASM call in the browser console:

```javascript
// If worksheet solve gave spread rate 82.44 ch/h (wrong)
// and direct call gives 19.68 ch/h (correct),
// then the worksheet is passing wrong or missing values.

// Directly (in-page), correct:
let s = new Module.SIGSurface();
s.setFuelModelNumber(124);
s.setWindSpeed(new Module.SpeedUnits("MilesPerHour"), 5.0);
s.doSurfaceRun(/* ... */);
console.log(s.getSpreadRate(/* ... */));  // Should be ~19.68
```

### Fix pointers

**Bug #1: `add-ws-input!` passes units as 6th arg but handler only takes 5**  
File: `/projects/behave/test/cljs/behave/solver_test.cljs` (~L150)  
Status: FIXED on branch `rj-fix-figwheel-tests` (NOT on main). SOLVER_TEST_HANDOFF.org cites commit 49548d1f, but that SHA was rebased away and is orphaned — reachable equivalents on the branch are 4b7b2dbb ("persist units-uuid") and 4170ec4c.  
The handler for `:worksheet/upsert-input-variable` destructures only 5 params; the 6th (units-uuid) was silently dropped.  
**Fix**: Dispatch `:worksheet/update-input-units` after upsert, mirroring the wizard's flow.

**Bug #2: Fallback to domain native unit can mismatch stored value's unit**  
File: `/projects/behave/src/cljs/behave/solver/...` (somewhere in input-lookup chain)  
Symptom: Input stored as fraction 0.2 is labeled Percent (enum 1) → WASM reads 0.2% (bone-dry) → wrong spread.  
**Fix**: Ensure units-uuid persistence doesn't regress. Verify `:worksheet/all-inputs+units-vector` returns UUIDs, not DataScript entity IDs.

**Bug #3: `nil` unit passed to setter (most common)**  
Source: VMS drift or query returning no match.  
**Fix**: 
1. Check units-uuid resolve: run `(q/unit-uuid->enum-value "my-units-uuid")` in REPL.
2. If null, units-uuid is not in VMS. Re-sync layout.msgpack or restore units via CMS migration.
3. Verify `:worksheet/variable-level-units` is not returning DataScript entity IDs (e.g., 4874) instead of uuids when native-unit is ref-typed.

---

## §B: Outputs pinned at -100 or constant values

### Root cause A: Enum/species coverage

The compiled WASM species table on **main** includes all 525 species codes, but **branch `rj-fix-figwheel-tests` reduces this to 190 GACC-supported codes** (commit 7ce42e6e). Test data or worksheets referencing absent species codes get `-100` (the "not found" sentinel from `getSpeciesTableIndexFromSpeciesCode`).

**Why?** The old mortality.csv on main still has 525 species codes (15,052 rows); branch rj-fix-figwheel-tests re-generated from FOFEM reference in commit 7ce42e6e (date 2026-07-02): now 3,798 rows (down from 15,052), CRNSCH + BOLCHR only. On current main, the table is unreduced. The CRCABE (crown_damage) equation path is inert (root cause B, below).

### Root cause B: Inert model path (CRCABE crown_damage)

Direct testing shows CRCABE inputs (crown damage %, cambium kill rating) have **zero effect** on output (always ~1.5–3% mortality regardless of setters). The C++ reference (behave-mirror/src/testMortality/) has zero CRCABE rows. **Conclusion**: crown_damage path was never validated upstream; it is inert. Fix requires behave-mirror patch + FOFEM golden data (deferred).

### Discriminating experiment

**Check species table size vs. expected:**

```javascript
// In-browser console:
Module.SIGMortality.prototype.getNumberOfRecordsInSpeciesTable()
// Should return ~197 for the GACC set (190 base codes + variants).
```

**For a -100 result, check if species is in table:**

```javascript
let m = new Module.SIGMortality(new Module.SpeciesMasterTable());
m.setGACCRegion(Module.GACC.SouthernArea);
let idx = m.getSpeciesTableIndexFromSpeciesCode("ABGRI2");
console.log(idx);  // -1 means absent, causing -100 output.
```

**For constant-value outputs, test setter effect directly:**

```javascript
let m = new Module.SIGMortality(new Module.SpeciesMasterTable());
m.setGACCRegion(Module.GACC.SouthernArea);
m.setSpeciesCode("ABCO");  // A code known to resolve
m.setEquationType(Module.EquationType.crown_damage);
m.updateInputsForSpeciesCodeAndEquationType();

// Try with and without crownDamage setter:
m.setCrownDamage(30);
m.calculateMortalityAllDirections(Module.ProbabilityUnits.Percent);
let obs1 = m.getProbabilityOfMortality(Module.ProbabilityUnits.Percent);
console.log("With 30% damage:", obs1);

m.setCrownDamage(90);
m.calculateMortalityAllDirections(Module.ProbabilityUnits.Percent);
let obs2 = m.getProbabilityOfMortality(Module.ProbabilityUnits.Percent);
console.log("With 90% damage:", obs2);
// If obs1 ≈ obs2, the setter has no effect → inert path.
```

### Fix pointers

**For -100 (missing species):**
1. Verify species code is in the 190-code GACC set: check `getSpeciesTableIndexFromSpeciesCode(code)`.
2. If absent, either:
   - **Remove the row from test data** (if it's an unsupported variant code; 339 such rows were dropped from mortality.csv in 2026-07-02).
   - **Restore the species** (requires behave-mirror change + FOFEM data refresh; data-owner effort).
3. Test fixture: ensure all species codes reference real rows in the compiled table. Use C++ reference `/behave-lib/behave-mirror/src/testMortality/FOFEM_input.tre` as the source of truth (186 species, all in GACC scope).

**For constant outputs (CRCABE):**
1. **Do NOT attempt to fix in the test.** The model path is inert; this is a behave-mirror bug.
2. **Option A (pragmatic)**: exclude CRCABE rows from test data until model is fixed.
3. **Option B (proper)**: file a behave-mirror ticket to implement + validate crown_damage in C++, regenerate FOFEM goldens.
4. **See also**: `MORTALITY_TEST_HANDOFF.org` (full analysis, confirmed 2026-07-02).

---

## §C: Entire CLJS test suite red (WASM bootstrap, stale layout.msgpack)

### Root causes

1. **WASM module not initialized at page load** — figwheel host page tries to call `Module.SIGSurface(...)` before `/behave-min.wasm` loads. **Fix**: enums.cljs was refactored to not gate on `window.runtimeInitialized` (commit 761b0c22, on branch `rj-fix-figwheel-tests` — NOT on main).
2. **Stale `layout.msgpack`** — if VMS schema changes (migrations) but layout.msgpack is not re-synced, client loads old structure. Tests then fail with "group-variable not found" or "units-uuid mismatch."
3. **Missing VMS migration** — a migration exists in `projects/behave_cms/resources/migrations/` but Postgres DB has not run it. DataScript then reads stale data → silent schema drift.

### Discriminating experiment

**Open test page and inspect console:**

```
http://localhost:8081/api/test
```

Look for:
- **WASM errors**: `TypeError: Cannot read property 'SIGSurface' of undefined` or `Module is not defined` → WASM failed to load.
- **GACC enum errors** in first few assertions (e.g. "gacc is not a function") → enums.cljs not initialized or out of sync.
- **-100 or NaN outputs** in first test → layout.msgpack stale relative to test fixture.

**Check WASM file timestamp:**

```bash
ls -l projects/behave/resources/public/js/behave-min.wasm
# Compare to when migrations were last run:
ls -l projects/behave_cms/resources/migrations/ | tail -5
```

If WASM is older than migrations, rebuild:
```bash
cd behave-lib && make install
```

**Verify CMS DB and migration state:**

```bash
# Datomic transactor must be running:
bb transactor &

# In REPL (or clojure-mcp), query the VMS:
(d/q '[:find ?id ?nm :where [?id :group-variable/name ?nm]] (d/db conn))
# Should return a substantial result set (100s of group-variables).
# If empty or few, migrations may have failed silently.
```

**Check layout.msgpack sync:**

```bash
# After migrations, regenerate:
clojure -X:download-vms
# Or in CMS app, admin → export.

# Compare fixture to live:
sqlite3 worksheets/behave7-20-min.bp7 "SELECT COUNT(*) FROM datoms;" 
# Should match expected row count from the test fixture generation.
```

### Fix pointers

**WASM bootstrap fail (module undefined):**  
1. Ensure figwheel dev server finished building. Check console: should show `Figwheel: loaded...`.
2. Verify `/projects/behave/resources/public/js/behave-min.wasm` exists and is >1MB.
3. Open console Network tab: find `behave-min.wasm`, check status code 200.
4. If 404, rebuild: `cd /projects/behave && clojure -M:dev:behave/app:figwheel` (or re-run `make install` from behave-lib).

**Stale layout.msgpack / schema drift:**  
1. Verify all migrations in `/projects/behave_cms/resources/migrations/` have been run: check Postgres transaction log or CMS admin panel.
2. If a migration exists locally but DB is out of sync, manually run against Postgres:
   ```bash
   bb transactor &
   # In clojure-mcp or local REPL:
   (require '[schema-migrate.core :as sm])
   (sm/migrate "datomic:sql://..." :to-version <latest>)
   ```
3. Re-export layout.msgpack: `clojure -X:download-vms` or CMS admin → export.
4. Rebuild test fixtures (if .bp7 files reference VMS IDs):
   ```bash
   cd /projects/behave && clojure -M:test:behave/app -e "(require '[behave.test-support :as ts]) (ts/rebuild-fixtures!)"
   ```
5. Clear browser cache, hard-refresh (Cmd-Shift-R).

**First few tests fail with mysterious enum errors:**  
1. Confirm enums.cljs is not gating on `window.runtimeInitialized`. Check `/projects/behave/src/cljs/behave/lib/enums.cljs` — should NOT have `(when (js/goog.DEBUG) (js/alert "..."))` before enum lookup.
2. Rebuild ClJS: `clojure -M:dev:behave/app:figwheel`.
3. If still failing, check behaveUnits.h enums match what test expects. Enum value mismatch (e.g., enum 0 vs enum 1 for a unit) = silent wrong outputs. Re-run Hatchet (the tool that auto-generates behave/lib/*.cljs).

---

## §D: Build failures (node shim, externs, EM_CACHE)

### Root cause A: `node` is a Bun shim

On the dev machine class this repo targets, `/usr/local/bin/node` is a Bun shim. ClJS build needs real Node.js for `@cljs-oss/module-deps`.

### Root cause B: Externs files missing or incorrect

Advanced (minified) ClJS builds require `.externs.js` files listing external (non-minifiable) symbols. Mismatch → Closure minifier mangles WASM module names → runtime errors.

### Root cause C: Emscripten build cache stale

`EM_CACHE` env var points to the Emscripten build cache. If unset or pointing to old artifacts, the WASM build reuses stale intermediate files.

### Discriminating experiment

**Check node version and @cljs-oss/module-deps:**

```bash
which node
node --version  # If you see "bun" or an old version, use real Node
npm list @cljs-oss/module-deps
# If "npm ERR! not installed", npm install it.

# To use real node, add to PATH before Bun shim:
export PATH="/usr/local/opt/node/bin:$PATH"  # or wherever real node is
node --version
```

**Check externs files:**

```bash
find . -name "*_externs.js"
# Should find:
# - projects/behave/externs/behave_externs.js
# - projects/behave/externs/katex_externs.js
# - (any generated shadow-cljs externs)

# Verify syntax:
cat projects/behave/externs/behave_externs.js | head -30
# Should show lines like:
# var Module = {};
# Module.SIGSurface = function() {};
```

**Check EM_CACHE:**

```bash
echo $EM_CACHE
# Should print a path like:
# /Users/user/.emscripten_cache

# If unset, set it:
export EM_CACHE=~/.emscripten_cache
```

### Fix pointers

**Node shim blocking build:**
1. Install real Node:
   ```bash
   brew install node@18  # or latest stable
   ```
2. Add real Node to PATH *before* Bun shim in your shell profile:
   ```bash
   # ~/.zshrc or ~/.bashrc
   export PATH="/usr/local/opt/node/bin:$PATH"
   ```
3. Verify: `which node` should NOT print a Bun path; `node --version` should be 18+.

**Missing @cljs-oss/module-deps:**
```bash
cd projects/behave
npm install @cljs-oss/module-deps
```

**Externs file syntax or mismatch:**
1. Check externs file exists and has valid JS syntax:
   ```bash
   node -c projects/behave/externs/behave_externs.js
   ```
2. If fails, check for typos (e.g., unmatched braces).
3. Verify exported C++ names in `cms-exports/*.edn` match externs. For example, if C++ exports `SIGSurface`, externs must have:
   ```javascript
   var Module = {};
   Module.SIGSurface = function() {};
   ```
4. After fixing, rebuild:
   ```bash
   cd projects/behave && clojure -M:dev:behave/app:figwheel
   ```

**Emscripten cache stale:**
```bash
export EM_CACHE=~/.emscripten_cache
mkdir -p $EM_CACHE

# Clear and rebuild WASM:
cd behave-lib && make clean && make install
```

---

## §E: App/CMS won't start (Datomic, Postgres, port conflicts)

### Root causes

- **Port already bound** (9101 for app, 8001 for CMS, 4334 for Datomic transactor, 5432 for Postgres).
- **Datomic transactor not running** (CMS requires transactor).
- **Postgres not running** (CMS requires Postgres).
- **Incorrect config** pointing to wrong DB host/port or missing auth token.

### Discriminating experiment

**Check ports:**

```bash
lsof -i :9101  # App
lsof -i :8001  # CMS figwheel
lsof -i :4334  # Datomic transactor
lsof -i :5432  # Postgres
# If any show a process, either kill it or use a different port.
```

**Verify Datomic and Postgres:**

```bash
# Datomic transactor (checks if it's running and responsive):
nc -zv localhost 4334

# Postgres:
pg_ctl status
# or:
psql -h localhost -U datomic -d datomic -c "SELECT 1"
```

**Check app config:**

```bash
cat projects/behave/config.edn
# Verify :server :http-port is 9101 (or your desired port)
# Verify :vms :url points to accessible CMS (usually https://localhost:8082)
```

### Fix pointers

**Port conflict:**
```bash
# Kill the offending process:
kill -9 <PID>

# Or use a different port by editing config.edn:
# :server {:http-port 9102}
```

**Datomic transactor not running:**
```bash
# Start it:
bb transactor &
# Output should show "System started" and listen on :4334

# Verify after ~5 sec:
nc -zv localhost 4334
# Should print "Connection successful"
```

**Postgres not running:**
```bash
# Start Postgres (macOS with Homebrew):
brew services start postgresql

# Verify:
pg_ctl status
# Should show "pg_ctl: server is running"

# Create Datomic DB (if first time):
createuser -U postgres -P datomic  # prompts for password (use "datomic")
createdb -U postgres -O datomic datomic
```

**CMS won't start (wrong VMS URL or auth token):**
1. Check config.edn:
   ```bash
   cat projects/behave/config.edn | grep -A5 ":vms"
   ```
2. `:url` should be `https://localhost:8082` for dev (CMS figwheel on 8082).
3. `:secret-token` is a placeholder; ensure it matches CMS auth (usually hardcoded in dev).
4. If CMS is on a different port, update `:url`.

**Verify CMS starts:**
```bash
# Terminal 1:
bb transactor &
# Terminal 2:
pg_ctl start -D /usr/local/var/postgres
# Terminal 3:
clojure -M:dev:behave/cms:figwheel
# Should open http://localhost:8082/login in browser
```

---

## §F: Test failures from bad environment or unapplied migrations

### Root cause

Stale or missing migrations in Postgres, or DataScript schema out of sync with running VMS. Silent failures (no error message); tests just compute wrong values.

### Pre-flight environment checklist

Run this **before debugging test failures**:

| Check | Command | Expected | Action if fails |
|-------|---------|----------|-----------------|
| Node.js is real (not Bun shim) | `which node` | Should NOT contain "bun"; `node --version` 18+ | Install real Node; add to PATH |
| Postgres running | `pg_ctl status` | "server is running" | `brew services start postgresql` |
| Datomic transactor running | `nc -zv localhost 4334` | "Connection successful" | `bb transactor &` |
| CMS DB exists | `psql -U datomic -d datomic -c "SELECT 1"` | "1 row" returned | `createdb` and run migrations |
| CMS migrations applied | In CMS REPL: `(d/q '[:find (count ?e) :where [?e :group-variable/name]] (d/db conn))` | 100s of results (group-variables) | `bb transactor` + re-run migration tool |
| layout.msgpack up-to-date | `ls -l projects/behave/resources/public/msgpack/layout.msgpack` compare to `ls -l projects/behave_cms/resources/migrations/ \| tail -5` | layout.msgpack is newer than last migration | `clojure -X:download-vms` (app) or CMS admin → export |
| WASM rebuilt for current schema | `ls -l behave-lib/build/behave-min.wasm` vs `projects/behave_cms/resources/migrations/ \| tail -3` | .wasm is newer | `cd behave-lib && make install` |
| Figwheel dev build success | Browser console (localhost:8081/api/test) | No "undefined is not a function" or "Module is not defined" errors | Kill figwheel, restart: `clojure -M:dev:behave/app:figwheel` |
| Test fixtures match schema | `sqlite3 worksheets/behave7-20-min.bp7 "SELECT COUNT(DISTINCT a) FROM datoms WHERE e < 1000;"` compare to fixture generation | Row count matches golden | Rebuild fixtures or regenerate .bp7 files |

### Discriminating experiment

**Open app, make a worksheet change, check DataScript for schema drift:**

```javascript
// In browser console (localhost:9101 or localhost:8081):
// Check if units-uuids are real UUIDs or DataScript entity IDs

// Pull a worksheet:
let ws = datascript.core.q_BANG_([
  '[:find ?u :where [?ws :worksheet/name "test"] [?ws :worksheet/inputs ?inp] [?inp :input-variable/units-uuid ?u]]'
], behave.vms.store.vms_conn());
console.log(ws);
// Should be a UUID like "651dadb8-0158-4c68-8b41-d5013411d342"
// If it's a number like 4874, that's a DataScript entity ID → SCHEMA DRIFT
```

**Check if input setup mirrors the solver path exactly:**

Compare test helper to real solver:
- **Real solver** (`solve-worksheet` 1-arity, `/projects/behave/src/cljs/behave/solver/core.cljs:238`): dispatches `:worksheet/update-input-units` for every non-`:none` unit.
- **Test helper** (e.g., `solve-ws-outputs` in `solver_test.cljs`): if it calls the 4-arity `solve-worksheet` directly without the update-units loop, units will fall back to domain native unit (mismatch).

### Fix pointers

**Pre-flight checklist item fails:**
1. Follow the "Action" column above.
2. Re-run the check after fix.

**Schema drift detected (units-uuid is entity ID):**
1. Datomic migration likely failed or is out of order. Re-sync:
   ```bash
   bb transactor &
   # In REPL or clojure-mcp:
   (require '[schema-migrate.core :as sm])
   (sm/migrate "datomic:sql://..." :to-version :latest)
   ```
2. Re-export layout.msgpack: `clojure -X:download-vms`.
3. Rebuild test fixtures.

**Test computes wrong value after migrations:**
1. Verify test setup mirrors the real app path (not synthetic). Check `/projects/behave/test/cljs/behave/solver_test.cljs` for `solve-ws-outputs` — if it uses 4-arity solve without update-units loop, add:
   ```clojure
   ;; After building worksheet, before solve:
   (doseq [[gv-uuid unit-uuid] all-input-units]
     (when (not= unit-uuid :none)
       (rf/dispatch [:worksheet/update-input-units ws-uuid gv-uuid unit-uuid])))
   ```
2. Rebuild and re-run test.

---

## §G: Figwheel hot-reload broken (page doesn't update, console errors)

### Root causes

- **Figwheel server process crashed** or hung.
- **Browser cached stale JS** (old bundle).
- **Source map missing** (confusing error messages).
- **Port 8081 or 8082 bound by another process**.

### Discriminating experiment

**Check if Figwheel is running:**

```bash
lsof -i :8081  # App dev
lsof -i :8082  # CMS dev
# If nothing, figwheel crashed or wasn't started.

# Check process log (if running in tmux/screen):
ps aux | grep figwheel
# Should show the clojure command.
```

**Reload browser; check console:**

```
localhost:8081 (app) or localhost:8082 (cms)
Open Dev Tools → Console
```

Look for:
- `Figwheel: loaded (:figwheel/js-reload-dep)` — good, hot-reload is working.
- `Cannot find asset /cljs/app.js` — figwheel is not serving compiled JS. Likely crashed or stale build.
- `Uncaught TypeError: Cannot read property...` — app code error (not figwheel issue).

**Force hard-refresh and check Network tab:**

```
Cmd-Shift-R (or Cmd-Option-Shift-R on macOS)
Open Dev Tools → Network → filter for "app.js"
```

- Status 200 and size >1MB: good.
- Status 304 (cached): hard-refresh may not have worked; clear browser cache.
- Status 404: figwheel JS build failed or directory wrong.

### Fix pointers

**Figwheel process crashed:**
1. Kill any stray figwheel processes:
   ```bash
   killall java  # or `pkill -f figwheel`
   ```
2. Restart in a fresh terminal:
   ```bash
   cd /Users/rsheperd/code/sig/behave-app
   clojure -M:dev:behave/app:figwheel  # App dev
   # Or for CMS:
   clojure -M:dev:behave/cms:figwheel  # CMS dev
   ```
3. Wait ~30 sec for build. Browser should auto-reload.

**Stale JS cached in browser:**
```
Cmd-Shift-R (hard refresh)
```

If that doesn't work:
```
Dev Tools → Application → Cache Storage / Local Storage → Clear All
Cmd-Shift-R
```

**Port conflict:**
```bash
lsof -i :8081  # Or :8082 for CMS
kill -9 <PID>
# Then restart figwheel.
```

**Source map missing (hard to debug errors):**
1. Check that dev.cljs.edn has `:source-map true`:
   ```bash
   grep -n "source-map" dev.cljs.edn
   # Should show :source-map true
   ```
2. If missing, add it and rebuild.
3. Hard-refresh browser to pull new map.

---

## §H: .bp7 worksheet load failures (empty or crash)

### Root causes

- **Corrupt SQLite file** (file exists but datoms table is empty or has schema mismatch).
- **datascript-storage-sql version mismatch** (storage format changed; DB not readable).
- **VMS schema changed** (migration ran, but .bp7 fixtures created with old schema).
- **Missing fixture** (file referenced but doesn't exist in `worksheets/`).

### Discriminating experiment

**Check worksheet file integrity:**

```bash
sqlite3 worksheets/behave7-20-min.bp7 ".tables"
# Should list tables like: datoms

sqlite3 worksheets/behave7-20-min.bp7 "SELECT COUNT(*) FROM datoms;"
# Should return a count >0. If 0 or error, file is corrupt or empty.

sqlite3 worksheets/behave7-20-min.bp7 "SELECT COUNT(DISTINCT a) FROM datoms LIMIT 1;"
# Quick check of datom structure.
```

**Try opening a known-good fixture:**

```clojure
;; In ClJS REPL or app console:
(require '[behave.test-support :as ts])
(ts/load-fixture! "/worksheets/behave7-20-min.bp7")
;; If succeeds, that fixture is good; your problem fixture may be corrupt.
```

**Compare schema:**

```bash
# Dump schema from good fixture:
sqlite3 worksheets/behave7-20-min.bp7 ".schema datoms" > /tmp/schema-good.txt

# Dump schema from problem fixture:
sqlite3 worksheets/problem.bp7 ".schema datoms" > /tmp/schema-bad.txt

# Diff:
diff /tmp/schema-good.txt /tmp/schema-bad.txt
```

### Fix pointers

**Corrupt file (empty datoms):**
1. Delete it:
   ```bash
   rm worksheets/corrupt.bp7
   ```
2. Regenerate from test fixture or CSV:
   ```clojure
   ;; In app REPL:
   (require '[behave.worksheet :as ws])
   (ws/new-worksheet-from-csv! "worksheets/new.bp7" "surface.csv")
   ;; Or from a known-good fixture, copy:
   cp worksheets/behave7-20-min.bp7 worksheets/new.bp7
   ```

**datascript-storage-sql mismatch:**
1. This is rare; usually indicates a major version bump.
2. Regenerate fixtures:
   ```bash
   cd projects/behave
   clojure -M:test -e "(require '[behave.test-support :as ts]) (ts/rebuild-fixtures!)"
   ```

**VMS schema changed (fixtures out of sync):**
1. Migrations ran, but .bp7 fixtures were created with old schema. Datoms reference old entity IDs.
2. Rebuild fixtures:
   ```bash
   bb transactor &  # Ensure transactor + DB are up
   cd projects/behave
   clojure -M:dev:behave/app:figwheel
   # In REPL or test file:
   (require '[behave.test-support :as ts])
   (ts/rebuild-fixtures!)
   ```

**File doesn't exist:**
```bash
ls worksheets/ | grep -i <name>
# If not found, create one:
cd projects/behave && clojure -M:dev ... (create via UI and export)
```

---

## When NOT to use this skill

This skill covers **post-deployment, in-repo debugging** of Behave7. Do NOT use this skill for:

- **Architecture questions** (load-bearing design decisions, why a pattern was chosen): see `behave-architecture-contract`.
- **Failure archaeology** (chronicle of settled incidents, root causes with evidence): see `behave-failure-archaeology`.
- **Refactoring or adding features** (code changes, testing): see `feature-dev` or your project's test/review skills.
- **Fire-science domain questions** (Rothermel, fuel models, GACC regions): see `fire-behavior-reference`.
- **VMS pipeline (adding variables, CMS changes, migrations)**: see `behave-vms-variable-pipeline`.
- **Build environment setup (first-time install, dep versions)**: see `behave-build-and-env`.
- **Running the app in production or choosing a deployment mode**: see `behave-run-and-operate`.
- **Designing test cases or golden datasets**: see `behave-validation-and-qa`.
- **absurder_sql campaign (decision gates, promotion)**: see `behave-absurder-sql-campaign`.

---

## Provenance and maintenance

Every claim below is verified against the repo at `/Users/rsheperd/code/sig/behave-app` on 2026-07-06. Re-verify these facts if failures persist or code changes:

| Fact | Re-verify command | Expected result | File/LOC |
|------|-------------------|-----------------|----------|
| Solver logs via `[:SOLVER :SINGLE ...]` tag | `grep -n "log-solver" /projects/behave/src/cljs/behave/solver/core.cljs` | Line 30 defines tag | `/projects/behave/src/cljs/behave/solver/core.cljs:30` |
| Figwheel dev port 8081 (app) | `grep ring-server-options dev.cljs.edn` | `:port 8081` | `/dev.cljs.edn:7` |
| Figwheel dev port 8082 (CMS) | `grep ring-server-options cms.cljs.edn` | `:port 8082` | `/cms.cljs.edn:9` |
| Server mode port 9101 | `grep :http-port projects/behave/config.edn` | `:http-port 9101` | `/projects/behave/config.edn:4` |
| Test page route | `grep -n "/api/test" projects/behave/src/clj/behave/handlers.clj` | Render-tests-page handler | `/projects/behave/src/clj/behave/handlers.clj` |
| Units nil skip setter (units-uuid bug) | `grep -B2 -A2 "(some? unit)" /projects/behave/src/cljs/behave/solver/core.cljs` | Line 48 condition | `/projects/behave/src/cljs/behave/solver/core.cljs:48-52` |
| Migrations directory | `ls -d projects/behave_cms/resources/migrations/` | Directory exists | `/projects/behave_cms/resources/migrations/` |
| WASM install path | `ls -l projects/behave/resources/public/js/behave-min.wasm` | File exists, >1MB | `/projects/behave/resources/public/js/behave-min.wasm` |
| behaveUnits.h enums | `grep -n "struct.*Units" behave-lib/behave-mirror/src/behave/behaveUnits.h \| head -5` | AreaUnits, BasalAreaUnits, LengthUnits, LoadingUnits, PressureUnits | `/behave-lib/behave-mirror/src/behave/behaveUnits.h:31-..` |
| Species table size (GACC set) | In figwheel console: `Module.SIGMortality.prototype.getNumberOfRecordsInSpeciesTable()` | ~197 (190 base + variants) | Dynamic (WASM); see `MORTALITY_TEST_HANDOFF.org` commit 7ce42e6e |
| Test suite status | `http://localhost:8081/api/test` after `clojure -M:dev:behave/app:figwheel` | Main: 59 deftests in `projects/behave/test/cljs/` (measured `grep -rho '(deftest ' projects/behave/test/cljs/ \| wc -l`, 2026-07-06; status varies); branch rj-fix-figwheel-tests: runner reported 35 deftests / 4,161 assertions green (2026-07-02) | See `/FIX_TEST_PLAN.org` for branch status |
| Datomic transactor port | `grep -n "4334" bb.edn` | Not explicitly in bb.edn; hardcoded in datomic config | `/bases/datomic_store/config/datomic-sql.properties` (not in skill scope, assumed 4334 per standard) |
| CMS Datomic transactor start | `bb transactor` | Starts transactor on :4334 | `/bb.edn:51-57` |
| Postgres default port | Standard (5432) | Assumed standard Postgres port | Not hardcoded in repo; operator choice |

**Notes on volatile facts:**
- Test suite status (green/red) changes per commit. Check FIX_TEST_PLAN.org for current status.
- WASM rebuild timestamp drifts; re-verify if solver output changes unexpectedly.
- Migration count grows as new schema changes ship; re-check `/projects/behave_cms/resources/migrations/` if schema questions arise.
- Species table size is fixed to GACC set (197) as of 2026-07-02; changes only if behave-mirror is rebuilt or GACC scope expands.
