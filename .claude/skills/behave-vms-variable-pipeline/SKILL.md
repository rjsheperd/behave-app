---
name: behave-vms-variable-pipeline
description: End-to-end propagation pipeline when fire-science surface area changes; C++ → WASM → Hatchet CLJS → cms-exports → CMS → layout.msgpack → fixtures. Add-a-variable checklist, variables_mapping.org tag taxonomy, manual vs automated steps, drift hazards.
---

# Behave7 VMS Variable Pipeline

**As of 2026-07-06**

## Overview

The Behave7 variable pipeline is the mechanism by which changes to the underlying fire-science C++ implementation propagate through the entire system: from C++ solver code in behave-mirror, through WebAssembly compilation and JavaScript bindings, into the ClojureScript application, and finally into the Variable Management System (VMS)—the Datomic-backed content database that defines the app's structure, module layout, input/output variables, and help content.

This skill documents:
1. The six-stage propagation pipeline
2. An **add-a-variable checklist** for adding new variables or functions
3. The **variables_mapping.org taxonomy** and what each tag means
4. Known drift hazards and where they have historically struck
5. Which steps are automated and which require manual intervention

---

## Glossary

- **behave-mirror**: A git submodule (branch `rj-rust-port`) at `behave-lib/behave-mirror/` containing the C++ source code implementing the extended Rothermel fire-behavior model. Classes include SIGSurface, SIGCrown, SIGContainAdapter, SIGMortality, SIGSpot, SIGIgnite.
- **Hatchet**: An ANTLR-based code-generation tool (external repository, not in this repo) that reads C++ class definitions and generates WebIDL bindings, ClojureScript function wrappers, and EDN entity definitions.
- **WASM**: WebAssembly; the behave-mirror C++ is compiled to WASM via Emscripten, producing `behave-min.wasm` and `behave-min.js`.
- **cms-exports**: EDN files in the repository root (`cms-exports/*.edn`) that define C++ class/function metadata for import into the VMS.
- **VMS (Variable Management System)**: A Datomic + PostgreSQL database (`projects/behave_cms/`) that stores the application data model: Modules, Submodules, Groups, GroupVariables, Variables, Units, and their relationships.
- **layout.msgpack**: A serialized msgpack file (`projects/behave/resources/public/layout.msgpack`) that is downloaded by the client at startup and deserialized into a DataScript database, providing all VMS metadata to the Behave7 app.
- **GroupVariable**: A VMS entity linking a Variable to an input/output Group within a Module. Each GroupVariable has a UUID, an optional `:group-variable/cpp-function` link to a C++ function, and metadata like min/max, conditionals, visibility.
- **Variable**: A VMS entity representing a single named quantity (e.g., "Wind Speed"). Each Variable has a domain (e.g., SpeedUnits) and a set of allowed units.

---

## Stage 1: C++ Change in behave-mirror

**File location**: `behave-lib/behave-mirror/src/behave/*.cpp` and `behave-lib/behave-mirror/src/behave/*.h`

**What happens**: A fire-science developer modifies the C++ implementation—e.g., adding a getter, changing parameters, adding a new class method, or fixing equations.

**Verification**:
```bash
cd /Users/rsheperd/code/sig/behave-app
git submodule status | grep behave-mirror
# Expected: rj-rust-port branch

# Inspect the change:
cd behave-lib/behave-mirror
git log --oneline -5
git diff HEAD~1 src/behave/surface.cpp  # Example
```

**Manual or Automated?** Manual. Developers make edits using standard C++ workflows.

**Drift hazard**: None yet; the change hasn't entered the Behave7 system.

---

## Stage 2: WASM Rebuild

**File locations**:
- `behave-lib/Makefile` — orchestrates the build
- `behave-lib/CMakeLists.txt` — CMake configuration
- `behave-lib/include/idl/behave.idl` — WebIDL interface definition (generated from C++ headers via Hatchet)
- `behave-lib/flake.nix` — Nix development environment
- Output: `projects/behave/resources/public/js/behave-min.{wasm,js}`

**What happens**:

1. **Bind step** (`make bind`): 
   - Reads `include/idl/behave.idl` (generated elsewhere by Hatchet)
   - Runs `webidl_binder` to create `include/cpp/emscripten/glue.cpp` and `include/js/glue.js`
   - These files bridge C++ and JavaScript

2. **Compile step** (`make compile`):
   - `emcmake cmake -B build` — configures for Emscripten
   - `cmake --build build` — compiles all C++ + glue code
   - Produces `build/behave-min.wasm` and `build/behave-min.js`

3. **Install step** (`make install`):
   - Copies `behave-min.*` to `projects/behave/resources/public/js/`

**Verification**:
```bash
cd /Users/rsheperd/code/sig/behave-app/behave-lib

# From Nix environment:
nix develop

# Or without Nix, ensure emscripten, cmake, clang are in PATH:
export EM_CACHE=$PWD/.em_cache
export WEBIDL="$(brew --prefix emscripten)/libexec/tools/webidl_binder.py"  # macOS

# Run the build:
make install

# Verify output:
ls -lh ../projects/behave/resources/public/js/behave-min.*
# Expected: recent timestamps
```

**Manual or Automated?** Semi-automated. The `make install` target runs all three steps, but:
- Must be triggered manually (no CI gate runs this)
- The EM_CACHE environment variable must be set correctly (see "Build Traps" below)
- Requires emscripten to be in PATH (the dev machine may have Bun's `node` shim instead of real node)

**Drift hazard — EM_CACHE pollution**:
If `EM_CACHE` is not set or points to a stale location, Emscripten will cache old object files and the rebuild will silently use stale intermediate binaries. Solution: `rm -rf behave-lib/.em_cache` before rebuilding.

**Drift hazard — Node shim**:
The development machine class this repo targets may have Bun's `node` shim (which doesn't work for npm packages). Some build steps (e.g., when Hatchet runs) need a real node. Solution: Use Nix or ensure `which node` points to a real Node.js binary.

---

## Stage 3: Hatchet Regeneration of ClojureScript Bindings

**File locations**:
- Input: `behave-lib/build/behave-min.js` (the WASM output from Stage 2)
- Output: `projects/behave/src/cljs/behave/lib/*.cljs` (surface.cljs, crown.cljs, contain.cljs, mortality.cljs, spot.cljs, ignite.cljs, enums.cljs, units.cljs, etc.)

**What happens**:

Hatchet parses the compiled WASM module and generates:
1. ClojureScript wrapper functions for each C++ class method
2. Enum definitions (e.g., `behaveUnits.h` enums become keyword maps)
3. EDN metadata files (exported to `cms-exports/`)

**Why it's external**: Hatchet is a separate tool (maintained in the firelab/hatchet repository). It reads C++ headers and WASM bindings to produce Clojure/ClojureScript code. This repo does **not** contain Hatchet.

**Verification**:
```bash
# These files should be recently modified after Stage 2:
ls -lt /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/ | head -20
# Look for recent timestamps on: surface.cljs, crown.cljs, enums.cljs, etc.

# Check that files contain CLJS function definitions:
grep -E "defn|defmulti" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/surface.cljs | head -10
```

**Manual or Automated?** Manual. An engineer must:
1. Run Hatchet manually (outside this repo)
2. Copy the generated CLJS files into `projects/behave/src/cljs/behave/lib/`
3. Copy the generated EDN files into `cms-exports/`

**Example Hatchet invocation** (simplified; exact command depends on Hatchet setup):
```bash
cd ~/work/code/hatchet  # External hatchet repo
java -jar hatchet.jar \
  --behave-mirror /path/to/behave-app/behave-lib/behave-mirror \
  --output-cljs /path/to/behave-app/projects/behave/src/cljs/behave/lib \
  --output-edn /path/to/behave-app/cms-exports
```

**Drift hazard — Stale CLJS files**:
If a developer modifies a C++ function signature but forgets to regenerate the CLJS wrappers, the app will call the old function with wrong arity or parameter types. Tests may pass (if they mock the function) but production runs will fail.

Detection: Look for mismatches between C++ function signatures and CLJS `defn` arities in `projects/behave/src/cljs/behave/lib/*.cljs`.

**Drift hazard — Enums out of sync**:
If a new enum is added to `behaveUnits.h` but `enums.cljs` is not regenerated, the solver will not be able to look up the enum value, causing solver runs to fail with "unit not found" errors.

Detection: Compare `behave-lib/behave-mirror/src/behave/behaveUnits.h` enum member count with `enums.cljs` function count.

---

## Stage 4: cms-exports/*.edn Import

**File locations**:
- Input: `cms-exports/SIGSurface.edn`, `cms-exports/SIGCrown.edn`, `cms-exports/SIGMortality.edn`, `cms-exports/SIGContainAdapter.edn`, `cms-exports/SIGSpot.edn`, `cms-exports/SIGIgnite.edn`, `cms-exports/SIGBehaveRun.edn`, `cms-exports/SIGMoistureScenarios.edn`, `cms-exports/SIGFineDeadFuelMoistureTool.edn`, `cms-exports/SIGSlopeTool.edn`, `cms-exports/VaporPressureDeficitCalculator.edn`, `cms-exports/relativeHumidity.edn`, `cms-exports/safeSeparationDistanceCalculator.edn`, `cms-exports/unit-enums.edn`, `cms-exports/dimensions.edn`, `cms-exports/vars_min_max.csv`
- Import mechanism: `development/cms_import.clj` (manual invocation) + CMS migrations

**What happens**:

Each `cms-exports/SIG*.edn` file contains a nested map of C++ namespaces, classes, and their functions with parameter metadata. Example structure:
```clojure
{:global
 {:SIGSurface
  [["SIGSurface"
    {:type "SIGSurface"
     :id "SIGSurface"
     :parameters [...]}]
   ["doSurfaceRun"
    {:type nil
     :id "doSurfaceRun"
     :parameters [...]}]]}}
```

These are imported into the VMS (Datomic) via the function `add-export-file-to-conn` in `cms_import.clj`. This function:
1. Reads the EDN file
2. Looks up each C++ namespace in the VMS (or creates it if missing)
3. Creates `:cpp.class/name` and `:cpp.function/name` entities
4. Links functions to their parameters

**Verification**:
```bash
# Check that cms-exports files exist and are non-empty:
ls -lh /Users/rsheperd/code/sig/behave-app/cms-exports/*.edn | head -10

# Inspect the structure of one file:
head -100 /Users/rsheperd/code/sig/behave-app/cms-exports/SIGSurface.edn

# Count lines (typical SIGSurface is 1000+ lines):
wc -l /Users/rsheperd/code/sig/behave-app/cms-exports/SIG*.edn
```

**Manual or Automated?** Manual. An engineer must:
1. After Hatchet regenerates `cms-exports/*.edn`, commit them to the repo
2. Create a CMS migration (e.g., `2025_07_01_import_surface_changes.clj`) that calls `add-export-file-to-conn`
3. Run the migration by starting the CMS server

**Example migration** (from `projects/behave_cms/resources/migrations/`):
```clojure
(ns migrations.example-import
  (:require [behave-cms.store :refer [default-conn]]
            [datomic.api :as d]
            [cms-import :refer [add-export-file-to-conn]]))

(defn payload-fn [db]
  (let [conn (default-conn)]
    (add-export-file-to-conn "./cms-exports/SIGSurface.edn" conn)
    []))  ;; Return empty tx-data; the side-effect is the import
```

**Drift hazard — Stale EDN files**:
If `cms-exports/SIGSurface.edn` is out of sync with the C++ source (e.g., a function was removed but the EDN still lists it), the CMS import will create stale entities. When the solver tries to link a GroupVariable to the removed function, it will find the old entity instead of failing early.

Detection: After every Hatchet run, commit `cms-exports/` and review the diff. If function names or signatures changed, expect changes in the EDN.

---

## Stage 5: CMS Migration + layout.msgpack Export

**File locations**:
- CMS migrations: `projects/behave_cms/resources/migrations/2025_*.clj` (90+ files)
- Migration runner: `components/schema_migrate/src/schema_migrate/runner.clj`
- CMS server: `projects/behave_cms/src/clj/behave_cms/server.clj`
- VMS export endpoint: exposed by the CMS server at `/sync?auth-token=TOKEN`
- layout.msgpack download: `projects/behave/src/clj/behave/download_vms.clj`

**What happens**:

1. **CMS migrations run**:
   - When the CMS server starts, `schema_migrate.runner/apply-pending-migrations!` executes all unapplied `.clj` files in the migrations directory (in alphabetical order)
   - Each migration's `payload-fn` or `payload` variable is evaluated and transacted into Datomic
   - Migration IDs are recorded in the `:bp/migration-id` attribute to prevent re-running

2. **VMS is exported to msgpack**:
   - Endpoint: `POST /sync?auth-token=SECRET_TOKEN`
   - The CMS serializes the entire VMS graph (Modules, Groups, Variables, Units, etc.) to msgpack
   - Result: a binary msgpack file suitable for download

3. **layout.msgpack is downloaded to the client**:
   - During build (CI): `clojure -X:download-vms :url VMS_URL :auth-token TOKEN` (from `projects/behave/src/clj/behave/download_vms.clj`)
   - Saves to `projects/behave/resources/public/layout.msgpack`
   - Later, client-side startup loads this file via HTTP GET and deserializes it into DataScript

**Verification**:
```bash
# Check that CMS migrations exist:
ls -1 /Users/rsheperd/code/sig/behave-app/projects/behave_cms/resources/migrations/ | wc -l
# Expected: ~110+ files

# Check layout.msgpack size and timestamp:
ls -lh /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/layout.msgpack
# Expected: 1.3–1.5 MB, recent timestamp

# Simulate a local CMS migration run (requires CMS infrastructure):
# (Difficult without a running Datomic/Postgres; see "When NOT to use" section)
```

**Manual or Automated?** Semi-automated:
- Migrations are automated once the `.clj` files exist (they run on CMS startup)
- Export to msgpack is automated (triggered during build via `clojure -X:download-vms`)
- But migration `.clj` files must be manually created by an engineer

**How to create a migration**:
1. Copy `projects/behave_cms/resources/migrations/template.clj` to a new file named `2025_07_01_your_change_name.clj`
2. Edit the `:migrate/ignore?` metadata if needed (set to `true` to skip a migration)
3. Define either:
   - `payload` — a data structure (vector of Datomic transaction maps)
   - `payload-fn` — a function that takes the database and returns transaction data
4. Commit and push; the migration runs automatically when CMS starts

**Drift hazard — Missing migration**:
If the VMS data changes (e.g., a function is added or a parameter type changes) but no migration is written, the old data persists in Datomic. When the app loads layout.msgpack, it will have stale metadata. Example: A new function parameter was added in C++, Hatchet regenerated the EDN, but the EDN was not imported via a migration—the CMS still has the old parameter list.

Detection: Compare the count of functions/parameters in the latest `cms-exports/SIGSurface.edn` with queries against the CMS Datomic database.

**Drift hazard — Stale layout.msgpack**:
If migrations are applied to the CMS but `layout.msgpack` is not re-exported and re-synced, the client will load an outdated structure. Example: A GroupVariable was made hidden via a migration, but layout.msgpack still includes it as visible.

Detection: After deploying a CMS migration, manually trigger `clojure -X:download-vms` and verify that `layout.msgpack` file size and timestamp are recent.

---

## Stage 6: Test Fixture Updates

**File locations**:
- Test layout.msgpack: `projects/behave/resources/public/layout.msgpack` (served by the dev server)
- Test fixtures (worksheets): `worksheets/*.bp7` (SQLite databases)
- Test VMS code: `projects/behave/test/cljs/behave/test_runner.cljs`

**What happens**:

1. **Test runner loads layout.msgpack**:
   - `projects/behave/test/cljs/behave/test_runner.cljs` calls `(load-vms! "test")` on startup
   - This fetches `/layout.msgpack?v=test` from the dev server
   - The msgpack is deserialized into a DataScript instance used by all tests

2. **Tests use the VMS to validate solver outputs**:
   - Example: `projects/behave/test/cljs/behave/solver_test.cljs` looks up GroupVariables and their linked C++ functions
   - If layout.msgpack is stale, the test will fail to find a function or variable, causing assertion errors

3. **Test worksheets (.bp7 files) are SQLite databases**:
   - These are stored in `worksheets/` and checked into git
   - They contain pre-configured inputs and expected outputs for regression testing
   - If the VMS schema changes (e.g., a variable's units domain changed), an old .bp7 will have invalid unit UUIDs

**Verification**:
```bash
# Check that the test layout.msgpack is non-empty:
ls -lh /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/layout.msgpack

# Run the test suite to see if VMS-related tests pass:
cd /Users/rsheperd/code/sig/behave-app/projects/behave
clojure -M:dev:behave/app:figwheel  # Start dev server on 8081
# Then navigate to http://localhost:8081/api/test in browser

# Check that worksheets exist and are recent:
ls -lh /Users/rsheperd/code/sig/behave-app/worksheets/*.bp7 | head -10
```

**Manual or Automated?** Manual. An engineer must:
1. After a VMS change (e.g., variables renamed, units changed), identify affected test fixtures
2. Regenerate or update `.bp7` files by:
   - Opening them in the Behave7 app
   - Re-saving them (to ensure unit UUIDs and variable references are up to date)
   - Or delete stale `.bp7` files and create new ones via the app UI
3. Commit the updated `.bp7` files to git

**Drift hazard — Stale .bp7 files**:
If a Variable's units domain changed (e.g., "Wind Speed" units changed from English-only to English+Metric), old .bp7 files will have unit UUID references that no longer exist in the VMS. Tests that load these files will fail with "unit not found" errors.

Detection: After a VMS schema change, run tests and look for errors like `"unit-uuid not found in vms"` or unit conversion failures in solver tests.

---

## ADD-A-VARIABLE CHECKLIST

Use this checklist whenever adding a new Variable, GroupVariable, or solver function to Behave7. Each step includes a verification command.

### 0. Define the C++ function (in behave-mirror)

**Example**: Add a getter to `SIGSurface`:
```cpp
// In behave-lib/include/cpp/sig-adapters/SIGSurface.h
double getNewOutput(LengthUnits::LengthUnitsEnum);

// In behave-lib/include/cpp/sig-adapters/SIGSurface.cpp
double SIGSurface::getNewOutput(LengthUnits::LengthUnitsEnum units) {
  return surface_.getNewOutput(units);
}
```

**Verify**:
```bash
cd /Users/rsheperd/code/sig/behave-app/behave-lib/behave-mirror
git diff HEAD -- src/behave/*.cpp src/behave/*.h ../include/cpp/sig-adapters/*.cpp ../include/cpp/sig-adapters/*.h | head -50
# Should show your new function
```

### 1. Rebuild WASM

**Command**:
```bash
cd /Users/rsheperd/code/sig/behave-app/behave-lib
export EM_CACHE=$PWD/.em_cache
export WEBIDL="$(brew --prefix emscripten)/libexec/tools/webidl_binder.py"  # or use Nix
make clean install
```

**Verify**:
```bash
ls -lh /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/js/behave-min.wasm
# Should be recent timestamp (just now)
stat /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/js/behave-min.js
# Should show modification time within the last minute
```

### 2. Regenerate ClojureScript bindings via Hatchet

**Command** (in external Hatchet repo):
```bash
cd ~/work/code/hatchet  # Or wherever Hatchet is checked out
./run-hatchet.sh \
  --behave-lib /Users/rsheperd/code/sig/behave-app/behave-lib \
  --output-cljs /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib \
  --output-edn /Users/rsheperd/code/sig/behave-app/cms-exports
```

**Verify**:
```bash
cd /Users/rsheperd/code/sig/behave-app
# Check that CLJS files have recent timestamps:
stat projects/behave/src/cljs/behave/lib/surface.cljs | grep Modify
# Should be just now

# Check that the new function appears in the generated code:
grep "getNewOutput" projects/behave/src/cljs/behave/lib/surface.cljs
# Should find a defn or similar wrapper

# Check that cms-exports files were regenerated:
stat cms-exports/SIGSurface.edn | grep Modify
# Should be just now
```

### 3. Commit changes to the repo

**Command**:
```bash
cd /Users/rsheperd/code/sig/behave-app
git add projects/behave/src/cljs/behave/lib/*.cljs
git add projects/behave/resources/public/js/behave-min.*
git add cms-exports/*.edn
git commit -m "BHP1-XXXX Regenerate bindings for new getNewOutput function"
```

**Verify**:
```bash
git log -1 --stat
# Should show changes to the above files
```

### 4. Create a CMS migration to link the new function

**Command**:
```bash
# Copy the template:
cp /Users/rsheperd/code/sig/behave-app/projects/behave_cms/resources/migrations/template.clj \
   /Users/rsheperd/code/sig/behave-app/projects/behave_cms/resources/migrations/2025_07_06_add_new_output_variable.clj

# Edit the file:
vim /Users/rsheperd/code/sig/behave-app/projects/behave_cms/resources/migrations/2025_07_06_add_new_output_variable.clj
```

**Example migration**:
```clojure
(ns migrations.2025-07-06-add-new-output-variable
  (:require [behave-cms.store :refer [default-conn]]
            [datomic.api :as d]
            [cms-import :refer [add-export-file-to-conn]]))

(defn payload-fn [db]
  (let [conn (default-conn)]
    ;; Import the updated SIGSurface.edn which now includes getNewOutput
    (add-export-file-to-conn "./cms-exports/SIGSurface.edn" conn))
  ;; Return empty; the side-effect is the import
  [])
```

**Verify**:
```bash
ls -1 /Users/rsheperd/code/sig/behave-app/projects/behave_cms/resources/migrations/ | tail -1
# Should show your new file: 2025_07_06_add_new_output_variable.clj
```

### 5. Create a GroupVariable in the VMS (via CMS UI or migration)

This step depends on whether the variable is an **input** (user-provided) or **output** (solver-computed).

**Option A: Via CMS UI** (if CMS server is running):
1. Start CMS: `cd /Users/rsheperd/code/sig/behave-app && bb dev:behave_cms`
2. Navigate to `http://localhost:8082` (CMS admin)
3. Create a GroupVariable linked to the new function
4. Set visibility, conditionals, help text, etc.

**Option B: Via migration** (recommended for production changes):
```clojure
;; In the migration file, also include:
{:db/id     [:db/ident :app/surface]  ;; Look up the Surface Module
 :module/group-variable
 [{:group-variable/uuid "YOUR-NEW-UUID-HERE"
   :group-variable/name "New Output"
   :group-variable/cpp-function  [:db/ident :cpp.function/getNewOutput]
   :group-variable/variable [:variable/name "New Output"]}]}
```

**Verify**:
```bash
# Start CMS and check that the GroupVariable appears in the UI:
cd /Users/rsheperd/code/sig/behave-app
bb dev:behave_cms &
# Wait for startup
curl -s http://localhost:8001/api/modules | jq '.[] | select(.name=="Surface") | .groups[] | .group_variables[] | select(.name=="New Output")'
# Should return the new GroupVariable JSON
```

### 6. Test locally

**Command**:
```bash
cd /Users/rsheperd/code/sig/behave-app

# Rebuild CMS and export layout.msgpack locally (requires running CMS):
bb dev:behave_cms &
sleep 5
# (Migrations run on startup)

# Download the updated layout.msgpack:
clojure -X:download-vms :url "http://localhost:8001" :auth-token "dev"
# (This assumes local CMS has a dev token; check config)

# Start dev server and test:
clojure -M:dev:behave/app:figwheel
# Navigate to http://localhost:8081
# Create a worksheet, set inputs, verify the new output appears and calculates correctly
```

**Verify**:
```bash
# Check that layout.msgpack was updated:
stat /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/layout.msgpack | grep Modify

# Run the test suite:
# (In browser) Navigate to http://localhost:8081/api/test
# Look for any test failures related to missing functions or variables
```

### 7. Re-sync test fixtures

If the VMS schema changed in a way that affects existing worksheets (e.g., variable units domain changed), update or regenerate test `.bp7` files:

**Command**:
```bash
cd /Users/rsheperd/code/sig/behave-app

# Open an affected .bp7 in the app and re-save it:
# (Manual step: open app, File → Open, select worksheets/BHP1-XXXX.bp7, File → Save)
# OR delete stale .bp7 files if they're no longer needed:
rm worksheets/stale-test.bp7

# Commit:
git add worksheets/
git commit -m "BHP1-XXXX Update test fixtures for new output variable"
```

**Verify**:
```bash
git log -1 --stat
# Should show modified .bp7 files
```

### 8. Commit the CMS migration and merged changes

**Command**:
```bash
cd /Users/rsheperd/code/sig/behave-app
git add projects/behave_cms/resources/migrations/2025_07_06_add_new_output_variable.clj
git add projects/behave/resources/public/layout.msgpack
git commit -m "BHP1-XXXX Add new output variable to VMS via migration"
git push origin your-branch
```

**Verify**:
```bash
git log --oneline -5
# Should show your commits in order
```

### 9. On merge to main: trigger build and release

**Command** (automatic via CI):
- Merge your PR to `main`
- CI runs: `clj-kondo` linter
- Manual tag push triggers `jar-builder.yml` workflow:
  ```bash
  git tag v7.1.5
  git push origin v7.1.5
  ```
- Build workflow runs:
  ```bash
  clojure -X:download-vms :url "$VMS_URL" :auth-token "$VMS_AUTH_TOKEN"
  ```
- This downloads the latest layout.msgpack from production VMS

**Verify**:
```bash
# Check CI logs for download-vms step:
gh run list --limit 1
gh run view <RUN_ID> --log
# Should show "Completed downloading from VMS!"
```

---

## variables_mapping.org Tag Taxonomy

**File**: `/Users/rsheperd/code/sig/behave-app/variables_mapping.org` (57K, created as behave6→behave7 porting guide)

This document maps each variable in the IA (Information Architecture) to C++ code blocks, and uses tags to indicate what work remains. The tags are metadata used by developers to plan work.

### Tag Counts (as of 2026-07-06)

| Tag | Count | Meaning |
|-----|-------|---------|
| `#make-getter` | 30 | Variable exists in C++, but no public getter function. Need to write a `double getX(Units)` wrapper. |
| `#expose` | 32 | Getter/setter exists in a different C++ class, need to bubble it up through SIG adapters (e.g., `SurfaceFuelbedIntermediates::foo()` → `SurfaceFire::foo()` → `SIGSurface::foo()`). |
| `#expose-or-new-solver` | 22 | Function exists in a different MODULE (e.g., Spot class method). Blocker: architectural decision not yet made—should we instantiate the Spot module inside Surface's constructor, or have the solver instantiate Spot separately? (See "Architecture Decision" section below.) |
| `#behave6` | 1 | Code exists in behave6 repository (old system), not yet ported to behave-mirror. Need to read the reference implementation and port to C++. |
| `#make-setter` | 7 | Variable exists in C++, but no public setter (e.g., a private member that must be mutated; need a `setX(value, units)` method). |
| `#ws-variable` | 3 | Variable should NOT be exposed to the solver at all—instead, it should be stored in the worksheet data and used to toggle inputs/outputs or passed as metadata (e.g., a user-provided "fire name"). |
| `#fix` | 1 | Code block is wrong, buggy, or incomplete. Needs investigation and repair. See notes in the org file. |

### Open Architecture Decision: #expose-or-new-solver (22 items)

**Question**: When a variable from one module (e.g., Spot) needs to be accessible as an output of another module (e.g., Surface), should we:

**Option A (current state)**: Instantiate the downstream module inside the upstream module's constructor.
- **Pros**: Simpler solver code; solver doesn't need to know about all modules.
- **Cons**: Tight coupling; if Spot's constructor has side effects or expensive initialization, Surface pays the cost.

**Option B (alternative)**: Instantiate all modules in the solver, and have them share references.
- **Pros**: Decoupled; easier to test modules in isolation.
- **Cons**: Solver orchestration logic becomes more complex.

**Status**: This decision is blocking progress on 22 variables (e.g., spotting distance from active crown fire). Until the team chooses an approach, these variables remain in the `#expose-or-new-solver` tag.

**Resolution path**: Write an ADR (Architecture Decision Record) and update the migration to reflect the choice. See "Proof and Analysis Toolkit" skill for running a spike to gather evidence.

---

## Manual vs Automated: Drift Hazards Summary

| Step | What | Manual or Automated? | Drift Hazard | Detection |
|------|------|----------------------|-------------|-----------|
| 1: C++ change | Edit behave-mirror | Manual | None yet; change hasn't entered system | Code review |
| 2: WASM rebuild | `make install` | Manual (must be triggered; no CI gate) | EM_CACHE stale, node shim issue, stale .em_cache | Rebuild fails or produces unchanged binaries |
| 3: Hatchet regen | Generate CLJS + EDN | Manual (external tool) | Stale CLJS wrappers (arity mismatch), enum mismatches | Tests fail with "function not found" or "unit not found" |
| 4: cms-exports import | Run CMS migration | Semi-auto (migration auto-runs on CMS startup, but must be written) | Stale EDN, missing migration | VMS has old function count or parameter list |
| 5: CMS export | `clojure -X:download-vms` | Automated (CI) for release builds; manual for dev | layout.msgpack not re-exported after VMS change | Client loads old structure; GroupVariable has stale UUID |
| 6: Test fixture update | Regenerate .bp7 | Manual | Stale .bp7 (invalid unit UUIDs, renamed variables) | Tests fail with "unit not found" or "variable not found" |

### Recommendations for Minimizing Drift

1. **After every C++ change**, tag the commit with the issue ID (e.g., `BHP1-1234-new-surface-output`).
2. **Immediately after WASM rebuild**, run the CLJS test suite locally (`clojure -M:dev:behave/app:figwheel` → `http://localhost:8081/api/test`). Failures here indicate Hatchet regeneration is needed.
3. **When committing cms-exports changes**, review the diff to ensure function counts and parameter types match your C++ changes.
4. **When creating a CMS migration**, add a comment referencing the C++ change or the Hatchet output that prompted it.
5. **After every VMS migration**, manually export layout.msgpack locally and verify it loads in the app without errors.
6. **Before merge to main**, run all tests in CI (currently only `clj-kondo` linter runs; propose adding functional tests to the gate).

---

## Key Files and Modules

### C++ / WASM Layer

| File / Directory | Purpose |
|------------------|---------|
| `behave-lib/behave-mirror/` | C++ source (submodule, branch `rj-rust-port`) |
| `behave-lib/Makefile` | WASM build orchestration |
| `behave-lib/CMakeLists.txt` | CMake configuration for Emscripten |
| `behave-lib/flake.nix` | Nix development environment |
| `projects/behave/resources/public/js/behave-min.wasm/.js` | Compiled WASM output |

### Hatchet Output Layer

| File / Directory | Purpose |
|------------------|---------|
| `projects/behave/src/cljs/behave/lib/surface.cljs` | Generated CLJS wrappers for SIGSurface class |
| `projects/behave/src/cljs/behave/lib/crown.cljs` | Generated CLJS wrappers for SIGCrown |
| `projects/behave/src/cljs/behave/lib/contain.cljs` | Generated CLJS wrappers for SIGContainAdapter |
| `projects/behave/src/cljs/behave/lib/mortality.cljs` | Generated CLJS wrappers for SIGMortality |
| `projects/behave/src/cljs/behave/lib/spot.cljs` | Generated CLJS wrappers for SIGSpot |
| `projects/behave/src/cljs/behave/lib/ignite.cljs` | Generated CLJS wrappers for SIGIgnite |
| `projects/behave/src/cljs/behave/lib/enums.cljs` | Generated keyword maps for C++ enums (units, fuel models, etc.) |
| `projects/behave/src/cljs/behave/lib/units.cljs` | Unit conversion lookup tables |
| `cms-exports/SIGSurface.edn` | EDN metadata for SIGSurface functions/parameters |
| `cms-exports/SIGCrown.edn` | EDN metadata for SIGCrown |
| `cms-exports/SIGMortality.edn` | EDN metadata for SIGMortality |
| `cms-exports/SIGContainAdapter.edn` | EDN metadata for SIGContainAdapter |
| `cms-exports/SIGSpot.edn` | EDN metadata for SIGSpot |
| `cms-exports/SIGIgnite.edn` | EDN metadata for SIGIgnite |
| `cms-exports/unit-enums.edn` | Unit enum definitions (e.g., `:speed-units/meters-per-minute`) |
| `cms-exports/dimensions.edn` | Unit dimension metadata (e.g., length, speed, density) |
| `cms-exports/vars_min_max.csv` | Min/max bounds for solver variables |

### VMS Layer

| File / Directory | Purpose |
|------------------|---------|
| `projects/behave_cms/src/clj/behave_cms/server.clj` | CMS server entry point |
| `projects/behave_cms/resources/migrations/` | 110+ migration files that populate Datomic |
| `components/schema_migrate/src/schema_migrate/runner.clj` | Migration discovery and execution engine |
| `development/cms_import.clj` | Functions to import EDN files into Datomic |
| `projects/behave/src/clj/behave/download_vms.clj` | Downloads layout.msgpack from CMS |

### App Layer

| File / Directory | Purpose |
|------------------|---------|
| `projects/behave/src/cljs/behave/solver/core.cljs` | Solver orchestration; calls C++ functions via CLJS wrappers |
| `projects/behave/src/cljs/behave/solver/queries.cljs` | VMS queries (lookup group-variable → function mapping) |
| `projects/behave/src/cljs/behave/vms/store.cljs` | Loads layout.msgpack; initializes VMS DataScript instance |
| `projects/behave/resources/public/layout.msgpack` | Deployed VMS snapshot |
| `projects/behave/test/cljs/behave/test_runner.cljs` | Test runner that loads layout.msgpack |
| `worksheets/*.bp7` | Test fixtures (SQLite databases with pre-configured inputs/outputs) |

---

## When NOT to use this skill

This skill covers end-to-end variable propagation when fire-science **surface area changes**—i.e., when the C++ solver interface or VMS data model changes.

**Use a different skill for:**

- **Running the app or CMS**: See `behave-run-and-operate` for launching dev/server/desktop modes, config options, and worksheets.
- **Solver behavior, performance, or correctness issues**: See `behave-debugging-playbook` for symptom-driven triage and `behave-validation-and-qa` for test protocols and golden data.
- **Building the app from scratch**: See `behave-build-and-env` for environment setup, prerequisite installation, and known build traps.
- **Solver algorithm details or fire-science domain knowledge**: See `fire-behavior-reference` for Rothermel model, units systems, GACC regions, and mortality equations.
- **Understanding app architecture and design decisions**: See `behave-architecture-contract` for invariants, module boundaries, and why-decisions.
- **Investigating past incidents or root causes**: See `behave-failure-archaeology` for chronicled issues, settlement evidence, and lessons learned.
- **Test runs and validation protocols**: See `behave-validation-and-qa` for evidence standards, test tier commands, known standing reds, and how to add new tests.
- **CMS-only changes** (adding help content, tweaking UI, reordering fields without changing solver behavior): Use `behave-run-and-operate` or the CMS UI directly.
- **Migrating data or rolling back changes**: See `behave-proof-and-analysis-toolkit` for migration dry-runs and data integrity recipes.

---

## Provenance and maintenance

**Last verified**: 2026-07-06

**Recent corrections** (2026-07-06):
- Fixed C++ adapter paths in ADD-A-VARIABLE checklist: corrected `behave-lib/behave-mirror/include/cpp/sig-adapters/` to `behave-lib/include/cpp/sig-adapters/` (the adapter layer lives in behave-lib, not behave-mirror)
- Added missing cms-exports files to Stage 4 documentation: `relativeHumidity.edn` and `safeSeparationDistanceCalculator.edn`
- Corrected `#ws-variable` tag count from 10 to 3 in variables_mapping.org tag taxonomy (verified by grep)

### Re-verification commands (run monthly or after major changes)

1. **Verify submodule branch**:
   ```bash
   git submodule status | grep behave-mirror
   # Expected: +<SHA> behave-mirror/behave-mirror (heads/rj-rust-port)
   ```

2. **Verify WASM output exists**:
   ```bash
   ls -lh /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/js/behave-min.{wasm,js}
   # Expected: files exist, size > 500 KB
   ```

3. **Verify CLJS files are recent and numerous**:
   ```bash
   ls -1 /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/*.cljs | wc -l
   # Expected: 17+ files
   ```

4. **Verify cms-exports structure**:
   ```bash
   ls -1 /Users/rsheperd/code/sig/behave-app/cms-exports/*.edn | wc -l
   # Expected: 11+ files (SIGSurface, SIGCrown, etc.)
   ```

5. **Verify migration count**:
   ```bash
   ls -1 /Users/rsheperd/code/sig/behave-app/projects/behave_cms/resources/migrations/*.clj | wc -l
   # Expected: 110+ files; if significantly fewer, migrations may have been consolidated
   ```

6. **Verify layout.msgpack exists and is non-empty**:
   ```bash
   ls -lh /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/layout.msgpack
   # Expected: > 1 MB, timestamp recent (within last week)
   ```

7. **Verify variables_mapping.org tag counts** (spot-check):
   ```bash
   for tag in "#make-getter" "#expose" "#expose-or-new-solver"; do
     echo -n "$tag: "
     grep "$tag" /Users/rsheperd/code/sig/behave-app/variables_mapping.org | wc -l
   done
   # Expected: #make-getter ≥ 29, #expose ≥ 8, #expose-or-new-solver ≥ 21
   ```

8. **Verify solver orchestration still references six modules**:
   ```bash
   grep -E "surface-module|crown-module|contain-module|mortality-module|spot-module|ignite-module" \
     /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/solver/core.cljs | wc -l
   # Expected: 6+ lines (one per module definition)
   ```

---

## Cross-references

- **For solver behavior and correctness**: `behave-validation-and-qa`, `behave-debugging-playbook`
- **For build environment and prerequisites**: `behave-build-and-env`
- **For running the app and CMS**: `behave-run-and-operate`
- **For fire-science details**: `fire-behavior-reference`
- **For architecture and design**: `behave-architecture-contract`
- **For past incidents and root causes**: `behave-failure-archaeology`
- **For data validation and analysis**: `behave-proof-and-analysis-toolkit`
