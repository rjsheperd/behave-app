---
name: behave-architecture-contract
description: Load-bearing architectural decisions, design rationale, invariants that must hold, and known weak points in the Behave7 Polylith monorepo. Read first when planning changes to storage, runtime, solver pipeline, or generated artifacts.
---

# Behave7 Architecture Contract

**Date: 2026-07-06** — volatile facts checked against deployed code.

This skill documents the non-negotiable architectural decisions that shape Behave7's structure, the rationale behind each choice, the invariants that must hold (and what breaks if they don't), and the known weak points that constrain future work.

**Audience**: Opus-level agents and engineers working on Behave7 internals. Assumes fluency with Clojure/ClojureScript and monorepo patterns. Fire-science domain terms (VMS, GACC, WASM, Rothermel) and architectural terminology are defined inline.

### Quick Glossary: Architectural Terms

Before diving in, here are the key architectural concepts used throughout:

- **Polylith**: An architectural pattern for organizing large monorepos into reusable components, bases, and projects with clear boundaries and dependency contracts.
- **re-frame**: A Clojure/ClojureScript framework for managing UI state using a unidirectional data flow (dispatch → event handlers → subscriptions → view).
- **DataScript**: An in-process Datalog database engine (like Datomic but lightweight, without external DB requirements). Used client-side for worksheet data.
- **Datomic**: A distributed database system with Datalog query language and immutable event log semantics. Used server-side for CMS authority data.
- **re-posh**: A re-frame plugin binding for DataScript, automating subscription queries when the DB changes.
- **SPA** (Single Page Application): A web app that runs in the browser, loading HTML once and updating the DOM dynamically (as opposed to server-side page rendering).
- **VMS** (Variable Management System): Behave7's hierarchical data model for fire-behavior inputs/outputs (Application → Module → Submodule → Group → GroupVariable → Variable).
- **GACC** (Geographic Area Coordination Center): A regional fire management coordination center; Behave7 tracks 190 GACC codes for species coverage.
- **WASM** (WebAssembly): Binary code format that runs in browsers; Behave7's solver (C++ Rothermel implementation) is compiled to WASM.
- **Rothermel**: The foundational empirical model for surface fire spread rate, used throughout Behave's fire-behavior engine.

---

## 1. The Polylith-Style 3-Tier Layout

Behave7 is a **Polylith-structured monorepo** with three tiers of code organization:

### Tier 1: Components (`components/`)

**Definition**: Reusable utility libraries, each with a public interface.

**Active production components** (used in behave app or CMS):
- `logging` — Triangulum-based distributed logging interface
- `transport` — Serialization bridge (msgpack, EDN, JSON, Transit, clj interop)
- `server` — Ring HTTP middleware and lifecycle management
- `config` — Configuration EDN file loading and env var overrides
- `schema-migrate` — Datomic schema query and migration utilities
- `file-utils` — Image resizing, zip file handling, platform-specific paths (app-data-dir)
- `jcef` — Java Chromium Embedded Framework wrapper (desktop JCEF mode only)
- `markdown2hiccup` — DITA/Markdown to Hiccup HTML conversion

**Inactive but present** (13 components with no usage in src/):
- `data-utils`, `string-utils`, `number-utils`, `browser-utils`, `dom-utils`, `async-utils`, `date-utils`, `csv-parser`, `version-utils`, `map-utils`, `datom-compressor`, `ds-schema-utils`
- These exist on the classpath but are not used. They may be premature extractions, abandoned refactoring, or reserved for future use.

**Half-migrated, interface incomplete**:
- `dita` (400 LOC, DITA XML generation, interface.clj empty)
- `cucumber` (100+ LOC, Selenium test runner, interface.clj empty)

**Dead stub**:
- `storage` (both interface.clj and core.clj are empty)

**Active spike**:
- `absurder_sql` — Rust + ClojureScript DataScript replacement; 30+ commits in branch `rj-ds-rust`, not yet integrated into main build.

**Key violation**: Components follow Polylith's `:interface` namespace contract (export public fns via `components/<name>/src/<name>/interface.clj`) in theory, but ~13 components have no usage, and 2 have empty interfaces. In a strict Polylith workspace, these would be caught by the `poly check` tool, which this repo does not use.

### Tier 2: Bases (`bases/`)

**Definition**: Infrastructure and middleware layers. Each base is a reusable platform that projects can depend on.

**Core bases**:

| Base | Purpose | Exports |
|------|---------|---------|
| `behave_schema` | Datomic/DataScript unified schema definitions | Schema specs + Datomic schema map for Application→Module→Submodule→Group→GroupVariable→Variable + Worksheet, plus separate tool/subtool/help-page schemas |
| `datom_store` | Client-side ephemeral + persistent storage | DataScript with SQLite backend via `datascript-storage-sql`; d/create-conn, d/restore-conn, d/transact, etc. |
| `datomic_store` | CMS authority database (Datomic peer + Postgres) | Datomic peer API + entity ID remapping logic (ds->datomic-eids / datomic->ds-eids atoms); used by behave_cms only |
| `datahike_store` | Unused fallback storage | Datahike API; positioned as middle ground between Datascript and Datomic but not integrated into any project |
| `behave_components` | Shared UI component library | Reagent components (buttons, inputs, tables, modals, etc.) used by both behave app and CMS |
| `behave_routing` | Bidi route definitions | Routes for worksheet/:ws-uuid/modules/:module/:io/:submodule, settings/:page, tools/:page hierarchies |
| `behave-docs` | MadCap Flare help authoring source | Git submodule at `bases/behave-docs`; help content compiled via `bb import-help` → Datomic → client queries |

### Tier 3: Projects (`projects/`)

**Definition**: Deliverable end-user applications.

| Project | Purpose | Main Entry |
|---------|---------|-----------|
| `behave` | Fire-behavior modeling client app (desktop or web) | `behave.core/-main` |
| `behave_cms` | CMS backend for VMS (Variable Management System) content management | `behave-cms.server/-main` |
| `behave_slim` | Lightweight version (scaffolded in deps.edn, no directory exists) | UNKNOWN |

### Critical Deviation from "Real" Polylith

**This repo is NOT a true Polylith workspace** because:

1. **No `workspace.edn` or `poly.edn` config file** — The `poly` tool (Polylith's command-line orchestrator) is not used. Instead:
   - Deps.edn `:dev` alias manually lists all component/base src and test directories
   - Component/base creation is manual (or via scripts that print instructions to add paths)
   - No `poly check` catches consistency violations, missing interfaces, or circular deps

2. **No poly tool enforcement** → The 13 unused components and 2 components with empty interfaces would be flagged as errors by `poly check` if this were a real workspace.

**Implication**: Behave7 uses Polylith's directory structure and naming conventions as guidance, but not its tooling or enforcement. This makes the architecture more flexible but less verifiable. Adding a component requires manual editing of deps.edn, not just directory creation.

---

## 2. Dual-Mode Runtime: JCEF Desktop vs. Server Mode

Behave7 ships as two different applications from the same code:

### Runtime Detection (from `projects/behave/src/clj/behave/core.clj`)

```clojure
(defn- conveyor?
  "True when running inside a Conveyor-packaged app (app.dir is set)."
  []
  (some? (System/getProperty "app.dir")))

(defn -main [& _args]
  (if (conveyor?)
    (do (server/init-config!)
        (server/enrich-config!)
        (start-cef!))
    (do (server/start-server!)
        @(promise))))  ;; Block forever
```

**JCEF Desktop Mode** (lines 70–113):
- Triggered when running inside a Conveyor-packaged app (System property `app.dir` is set)
- Lazy-requires `jcef.interface` so server mode never loads CEF natives (Java + Chromium binary bundle)
- Starts Jetty on localhost:PORT (default 8080, configured to 9101 in dev config.edn)
- Creates a custom HTTP request handler intercepting `http://localhost:PORT/*` before OS network stack
- Launches a JCEF window (Java Swing frame hosting Chromium)
- Database: SQLite at `~/.behave/db.sqlite` (platform-specific app-data-dir)
- Logging: `~/.behave/logs/`
- Use case: Desktop app for USFS field crews (Windows .exe, macOS .dmg, Debian .deb via Conveyor packaging)

**Server Mode** (lines 124–131):
- Triggered when app.dir is NOT set (e.g., running bare JAR, Docker container, dev environment)
- Starts Jetty on 0.0.0.0:PORT (default 8080, configured to 9101 in dev config.edn)
- No JCEF; browser access via `http://localhost:PORT` or remote HTTP
- Database: SQLite at path from config.edn (dev: `db.sqlite`, can be symlinked to shared store)
- Logging: path from config.edn
- Use case: Server deployment, containerization, dev REPL testing

**Key invariant**: The lazy-require on line 75 (`requiring-resolve 'jcef.interface/...`) ensures that if JCEF is missing (e.g., server-only Docker image), the app never tries to load CEF natives and fails at startup with a clear error rather than silently omitting desktop functionality. This supports deployments where JCEF is not available.

---

## 3. Three Storage Bases: Why Three?

Behave7 defines three storage/persistence bases to serve different architectural roles:

| Base | Technology | Used By | Purpose | Entity IDs | Scope |
|------|-----------|---------|---------|-----------|-------|
| `datom_store` | DataScript + SQLite (via datascript-storage-sql) | behave app (desktop/server) | Client-side worksheet data + user selections | Sequential, 1–N (DataScript default) | Ephemeral in-memory + persistent at ~/.behave/db.sqlite |
| `datomic_store` | Datomic peer + PostgreSQL backend | behave_cms only | Authoritative VMS data: modules, variables, equations, help content, users | Large (Datomic default: 17592186044416+), remapped via ds↔datomic-eids atoms | Persistent Postgres database |
| `datahike_store` | Datahike (persistent Datalog) | NONE (unused) | Planned fallback between Datascript and Datomic | Datahike default (sequential) | Would be persistent-beyond-process |

### Rationale

**Why DataScript for the client?**
- Lightweight, in-process, no external DB
- Identical Datalog query syntax as Datomic (portability)
- Browser-compatible for eventual CLJS in-memory state
- SQLite backend (datascript-storage-sql) for persistence between app restarts

**Why Datomic for CMS?**
- Transactional multi-version concurrency (important for shared CMS data)
- Immutable event log (audit trail for VMS changes)
- Datalog query language matches DataScript
- Authority: CMS is the single source of truth; client reads from it

**Why Datahike?**
- Never integrated into production code (only test files)
- Likely conceived as middle ground: persistent but simpler than Datomic
- Possibly reserved for future distributed/cloud scenarios
- **Recommendation**: Delete or document intended use, or integrate into absurder_sql spike

### Critical Detail: Entity ID Remapping

**Problem**: Datomic assigns large entity IDs (e.g., 13194139533312); DataScript expects small sequential IDs (1, 2, 3, …). When CMS exports VMS data (layout.msgpack), entity references must be translated.

**Solution** (from `bases/datomic_store/src/datomic_store/main.clj`):
```clojure
;; Atom maintains bidirectional mapping
(defonce ds->datomic-eids (atom {}))      ;; DataScript EID → Datomic EID
(defonce datomic->ds-eids (atom {}))      ;; Datomic EID → DataScript EID
```

When the client loads layout.msgpack, entity references are remapped from Datomic IDs to DataScript IDs. If this remapping fails (missing entry in the atom), subsequent `d/pull` queries fail silently with nil results.

**Invariant**: After any CMS migration that adds/changes entities, the remapping atoms must be recalculated and the msgpack re-exported. Failing to do so causes solver inputs to vanish.

---

## 4. VMS Data Model: The Hierarchy

The **Variable Management System (VMS)** is the data model for fire-behavior inputs/outputs. It defines the structure and semantics of all variables in Behave7.

### Hierarchy

```
Application
  └─ Module (Surface, Crown, Mortality, Contain, Spot, Ignite)
      └─ Submodule (e.g., "Fuel", "Weather", "Topography" under Surface)
          └─ Group (e.g., "Fuel Model Selection" under "Fuel" submodule)
              └─ GroupVariable (e.g., "Fuel Model" input, with UI control)
                  └─ Variable (e.g., enum values :fuel-model/10 :fuel-model/11 …)
```

**Example path**:
- Application: "BehavePlus"
- Module: "Surface"
- Submodule: "Fuel"
- Group: "Fuel Model"
- GroupVariable: UUID for the "Fuel Model" input control
- Variable: The set of available fuel model options (NFFL standard models 1–40, custom)

### Key Schema Entities

From `bases/behave_schema/src/behave/schema/`:

| Entity | Attributes | Role |
|--------|-----------|------|
| `:application/uuid`, `:module/uuid`, `:submodule/uuid` | Name, order, translation-key, help-key | Navigation hierarchy |
| `:group/uuid`, `:group/title` | Grouping label for related inputs | UI container |
| `:group-variable/uuid`, `:group-variable/order` | Order, translation-key, help-key, research? | Individual input/output control; may have `:group-variable/cpp-class` and `:group-variable/cpp-function` linking to C++ |
| `:variable/uuid`, `:variable/value`, `:variable/unit` | Enum value, unit; may have `:variable/domain` linking to fuel model/species | The actual data value |

### Separate Worksheet Schema

Worksheets (user runs/scenarios) are stored in a separate schema layer:

```
Worksheet
  └─ input-group (repeat-id for parametric runs)
      └─ input (value + unit for each GroupVariable)
  └─ output-groups (selected outputs to display)
      └─ Group (same as VMS Group)
```

Worksheets and VMS are separate but linked: a worksheet input refers to a GroupVariable UUID from the VMS.

**Key distinction**: VMS is defined in `behave_cms` (Datomic), while Worksheets are defined in `behave` (DataScript/SQLite). When app loads, it:
1. Fetches VMS layout.msgpack (serialized Datomic snapshot)
2. Loads it into a separate vms-conn (DataScript)
3. Queries vms-conn when rendering UI
4. Stores user worksheet inputs in the app's own DataScript (worksheet-conn)

---

## 5. Solver Orchestration and Output Linking Contract

The solver is the engine that computes fire-behavior outputs from inputs. It orchestrates six C++ modules in a strict sequence.

### The Six Modules (from `projects/behave/src/cljs/behave/solver/core.cljs` lines 254–297)

| Order | Module | C++ Class | Run Function | Purpose |
|-------|--------|-----------|--------------|---------|
| 1 | Surface | `SIGSurface` | `doSurfaceRun` | Rothermel surface fire: rate of spread, flame length, intensity |
| 2 | Crown | `SIGCrown` | `doCrownRun` | Crown fire initiation and active crown fire spread |
| 3 | Contain | `SIGContainAdapter` | `doContainRun` | Fire suppression effectiveness and final fire size |
| 4 | Mortality | `SIGMortality` | `calculateMortalityAllDirections` | Tree death probability by species and scorch height |
| 5 | Spot | `SIGSpot` | `calculateAll` | Spotting distance from firebrands (surface + crown only) |
| 6 | Ignite | `SIGIgnite` | `calculateFirebrandIgnitionProbability` | Firebrand ignition probability in fuel beds |

### Execution Flow

1. **Input setup** (line 244–246): Dispatcher ensures all input units are dispatched to re-frame, so subsequent queries have units available
2. **Parametric generation** (line 300): `generate-runs` creates Cartesian product of repeat-group inputs (parametric runs)
3. **Per-run loop** (line 301–334):
   - For each input row, initialize an empty `row` with inputs and all-outputs
   - **Module sequence** (lines 311–327):
     - If `:surface` in modules: `run-module surface-module`
     - If `:crown` in modules: `run-module crown-module`
     - … (conditionally run each module)
     - Special: Spot only runs if `:surface` AND `:crown` are both selected (line 323–324)
     - Ignite only runs if `:surface` selected (line 326–327)
   - Clean up source-link outputs (line 330)
   - Append row to accumulator

### Output Linking: The Propagation Contract

**Problem**: Module 2 (Crown) needs outputs from Module 1 (Surface). How do we pass them without manual wiring?

**Solution**: Output linking (lines 157–173 of `core.cljs`):

```clojure
(defn apply-output-links [prev-outputs inputs destination-links]
  ;; For each [src-uuid dst-uuid] pair in destination-links:
  ;;   If src-uuid was produced by previous module (prev-outputs contains it)
  ;;   AND dst-uuid input is EMPTY (nil)
  ;;   THEN insert the output value as the input value
  (reduce (fn [acc [src-uuid dst-uuid]]
            (if (prev-output-uuids src-uuid)
              (if (nil? (get-in acc [group-uuid 0 dst-uuid]))
                (assoc-in acc [...] output)  ;; Propagate
                acc)
              acc))
          inputs
          destination-links))
```

**Key constraint** (BHP1-1356 comment, line 165): "Only apply output link when the input is empty."

This means:
- If user explicitly sets a Crown input value, it takes priority over linked Surface output
- If Crown input is nil, the Surface output is automatically propagated
- Empty-input priority prevents accidental downstream pollution

**Invariant**: The `destination-links` mapping (from VMS) must accurately reflect which outputs feed which inputs. If a link is missing, solver produces independent results instead of coupled ones.

---

## 6. Generated-Artifact Boundary

Behave7 has a clear boundary between hand-written code and generated code. **Do not edit generated files.**

### Generated Artifacts

**WASM Bindings** (C++ → ClojureScript):
- **Location**: `projects/behave/src/cljs/behave/lib/surface.cljs`, `crown.cljs`, `mortality.cljs`, `spot.cljs`, `ignite.cljs`, `fuel_models.cljs`, `species_master_table.cljs`, `safe_separation_distance_calculator.cljs`, `moisture_scenarios.cljs`, `slope_tool.cljs`, `vapor_pressure_deficit_calculator.cljs`
- **Marker**: Top-of-file or early-file comment `;; Auto-generated by hatchet 🪓`
- **Note**: `contain.cljs` has no hatchet marker and is hand-written
- **Generation**: Hatchet tool (ANTLR-based C++ parser) scans C++ headers, generates ClojureScript wrapper functions that call `js/Module.SIGSurface.method(args)`
- **How to regenerate**: Run Hatchet (see `behave-lib/README.org` for procedure)
- **Why generated**: C++ method signatures are the source of truth; any changes to C++ must flow to ClojureScript bindings

**CMS-Exports EDN** (Datomic snapshot → EDN):
- **Location**: `cms-exports/SIGSurface.edn`, `SIGCrown.edn`, `SIGMortality.edn`, `SIGSpot.edn`, `SIGContainAdapter.edn`, `SIGIgnite.edn`, `SIGMoistureScenarios.edn`, `dimensions.edn`, `unit-enums.edn`
- **Marker**: No explicit marker, but files are dated and contain entity definitions from CMS (e.g., `:group-variable/uuid` entries)
- **Generation**: CMS export process (undocumented; appears to be manual or triggered via unknown CI step) or via `clojure -X:download-vms` during release
- **How to regenerate**: Connect to behave_cms Datomic, export entities to EDN (exact procedure not documented in this skill; see behave-vms-variable-pipeline skill)
- **Why generated**: CMS is the authority; these EDN files are snapshots used to populate the client's vms-conn

**Layout.msgpack** (Serialized VMS):
- **Location**: `projects/behave/resources/public/layout.msgpack`
- **Generation**: CMS export → msgpack serialization (via `clojure -X:download-vms` in CI/release)
- **Marker**: Binary format; no visible marker
- **How to regenerate**: In CI/release pipeline, `clojure -X:download-vms :url VMS_URL :auth-token TOKEN` fetches from CMS and writes to resources/public
- **Why generated**: Contains full VMS (modules, groups, variables, units, help) serialized efficiently for fast client load

### Hand-Written Code

Everything else is hand-written:
- `projects/behave/src/cljs/behave/events.cljs` — Re-frame event handlers
- `projects/behave/src/cljs/behave/solver/core.cljs` — Solver orchestration logic
- `projects/behave/src/cljs/behave/lib/units.cljs` — Unit conversions (manual, despite name)
- C++ files in `behave-lib/behave-mirror/` — Original Rothermel implementation

**Invariant**: Never patch generated files; regenerate them. If a WASM binding is broken, fix the C++ source and regenerate, don't edit the .cljs wrapper. If VMS data is wrong, fix it in CMS and regenerate layout.msgpack, don't hand-edit cms-exports/*.edn.

---

## 7. Key Invariants That Must Hold

If these invariants are violated, the app silently produces wrong results or crashes at runtime.

### 7.1 Schema Parity Between Storage Layers

**Invariant**: The Datomic schema in `behave_cms` (authority) must match the DataScript schema in the client.

**Why it matters**: When CMS exports layout.msgpack, it serializes Datomic entities. When the client deserializes, it expects DataScript to have the same schema attributes. If a CMS migration adds a new attribute (e.g., `:group-variable/new-field`), but the client's DataScript schema doesn't have it, queries silently omit the field.

**How to maintain**:
1. Schema changes are defined in `bases/behave_schema/src/behave/schema/*.cljc` (shared between JVM and CLJS)
2. Datomic schema is transacted in `projects/behave_cms/resources/migrations/*.clj`
3. After any Datomic schema migration, regenerate layout.msgpack and verify test fixtures load correctly

### 7.2 Entity ID Remapping Consistency

**Invariant**: The `ds->datomic-eids` and `datomic->ds-eids` atoms in `datomic_store` must stay in sync with actual entity IDs in Datomic.

**Why it matters**: When CMS exports layout.msgpack, it includes Datomic entity IDs (large numbers). The client remaps them to DataScript IDs (small sequential numbers) using these atoms. If an entity is missing from the atoms, references to it return nil.

**How to maintain**:
1. After CMS migrations add entities, recalculate the remapping atoms
2. Before releasing, verify that layout.msgpack deserializes without nil references
3. Check test fixtures (solver_test.cljs, etc.) for nil entity lookups

### 7.3 VMS Enum Values Must Match C++ Enum Definitions

**Invariant**: Enum values in `:variable/value` entities (e.g., fuel model IDs, unit codes) must match the corresponding C++ enum in `behave-lib/behave-mirror`.

**Example**: `behaveUnits.h` defines:
```cpp
enum class FirelineIntensityUnits { KW_PER_M, BTU_FT_S, ... };
```

The VMS must have `:variable/value` entries that match these enum member values. When the solver calls a C++ function with a unit, it passes the enum value; if the value doesn't match the C++ enum, the solver crashes or produces garbage.

**How to maintain**:
1. When C++ enums change (e.g., new fuel model added), update the VMS to include new enum values
2. Regenerate `projects/behave/src/cljs/behave/lib/enums.cljs` via Hatchet (which extracts enum values from C++ headers)
3. Verify that `projects/behave/src/cljs/behave/lib/units.cljs` includes conversions for all units in behaveUnits.h
4. Test the solver with the new enum values to ensure C++ accepts them

**Known issue** (from MEMORY.md): Mortality CRNSCH -100 status indicates species coverage gap (190 GACC codes supported, but 525 exist in some datasets; 339 variants unsupported).

### 7.4 layout.msgpack Must Match CMS State

**Invariant**: The layout.msgpack loaded by the client must be the current state of the CMS. If CMS is updated but layout.msgpack is not regenerated, the client has stale data.

**Why it matters**: Users enter inputs based on what they see in the UI (which is rendered from layout.msgpack). If layout.msgpack is stale, hidden or removed inputs are not available, causing confusion or errors.

**How to maintain**:
1. After CMS migrations, regenerate layout.msgpack via release pipeline (`clojure -X:download-vms`)
2. Do not manually edit layout.msgpack (it's binary msgpack, not human-editable)
3. Check git history to see when layout.msgpack was last updated; if stale (>1 week old), it's likely out of sync

### 7.5 Worksheet .bp7 Backward Compatibility

**Invariant**: Old .bp7 files (SQLite worksheet databases) must load in new versions of the app. The schema and entity IDs must be stable.

**Why it matters**: Users save their scenarios as .bp7 files. If a new app version changes the schema incompatibly (e.g., renames an input, changes entity IDs), old .bp7 files fail to load.

**How to maintain**:
1. When adding new inputs/outputs, don't remove or rename existing ones (add new entities, don't replace)
2. When renaming an entity (e.g., "Fire Perimeter" → "Fire Perimeter at First Resource Arrival Time"), provide a migration for existing worksheet data
3. Test loading old .bp7 fixtures (in `worksheets/` directory) with new app versions
4. Use `d/restore-conn` (not `d/create-conn`) to load existing .bp7 files; create-conn initializes a fresh DB

---

## 8. Known Weak Points and Design Tensions

These are areas where the architecture has gaps or conflicts that constrain future work.

### 8.1 Unused Components (13 of 25)

| Component | LOC | Issue |
|-----------|-----|-------|
| `data-utils` | ~15 fns | Never used; premature extraction? |
| `string-utils`, `number-utils` | ~20 fns | Duplicated in inline code |
| `browser-utils`, `dom-utils` | ~10 fns | Duplicated in behave_cms/utils.cljs |
| `async-utils`, `date-utils`, `csv-parser`, `version-utils`, `map-utils`, `datom-compressor`, `ds-schema-utils` | ~50 fns total | Unknown purpose |

**Risk**: These components are built and packaged but never called, increasing JAR size and maintenance burden. Grep for their namespaces in `projects/*/src` returns zero matches.

**Recommendation**: 
- Audit each component's purpose (check git history, JIRA)
- Either integrate them into active code or delete
- If reserved for future use, document the intent

### 8.2 Components with Empty Interfaces

**DITA** (`components/dita/`):
- ~400 LOC implementation in `core.clj`
- `interface.clj` is empty (no public fns exported)
- Used for DITA XML generation (help authoring)
- Status: Unclear if abandoned or incomplete

**Cucumber** (`components/cucumber/`):
- ~100 LOC Selenium WebDriver test runner
- `interface.clj` empty
- 3 feature files (`.feature`) in `features/` but ~40 scenarios are commented out
- Only 1 active scenario (surface_only.feature line 1–17)
- Status: Likely abandoned when browser-based Figwheel test runner proved more reliable

**Recommendation**:
- If DITA is still used for help authoring, complete its interface
- If Cucumber was replaced by Figwheel tests, either complete the interface or mark it deprecated

### 8.3 Dead Stub: storage Component

Both `core.clj` and `interface.clj` are empty placeholders. Likely intended for datahike or datascript layer but superseded by absurder_sql work.

**Recommendation**: Delete or document intended use.

### 8.4 Dual Version Source (Conveyor vs. resources/version.edn)

**Problem**: Version is hardcoded in two places:
- `projects/behave/conveyor.base.conf` line 29: `app { version = 7.1.4 }`
- `projects/behave/resources/version.edn`: `{:version "v7.1.4"}`

These must stay in sync or release packages are misaligned with client runtime version.

**Why it exists**: Conveyor needs version for packaging metadata; resources/version.edn is read at runtime.

**Current mitigation**: Release pipeline updates resources/version.edn via bump-version.yml, but conveyor.base.conf is not automatically updated.

**Risk**: If a developer manually runs a release without updating both files, version skew occurs. The packaged app reports v7.1.3 but internal version is v7.1.4.

**Recommendation**:
- Add a pre-release lint check that compares the two versions
- Or automate conveyor.base.conf update during release

### 8.5 No Functional CI Gate

**Current CI** (as of 2026-07-06):
- PR linting: clj-kondo on changed files only
- Tag push: builds JAR and packages via Conveyor (no solver validation)
- Release: manually triggered workflow (no tests run before release)

**Status**: Headless test runner was added on branch `rj-fix-figwheel-tests` (commit 206d7231, `bb test:ci` task, test-headless.cljs.edn), but is not yet merged to main or gated in CI/release pipelines.

**Risk**: A malformed solver change (e.g., output link corruption) could be released without detection.

**Recommendation**:
- Merge headless test runner branch to main
- Add `bb test:ci` to jar-builder.yml or release.yml as a pre-package step
- Ensure test suite gates on solver correctness (not just compilation)

### 8.6 layout.msgpack Has No Schema Versioning

**Problem**: layout.msgpack is binary msgpack with no embedded version field. When app versions change, there's no way to detect if msgpack is from an older VMS schema.

**Current mitigation**: Developers manually regenerate layout.msgpack during releases and run tests to catch breaking changes.

**Risk**: 
- User with old cached layout.msgpack upgrades app, app silently uses stale VMS data
- Solver inputs/outputs are missing or misnamed

**Recommendation**:
- Add `{:version "7.1.4"}` entry to layout.msgpack before serialization
- On app startup, check msgpack version and invalidate cache if mismatch
- Or, auto-regenerate msgpack from CMS on first load (slower but safer)

### 8.7 Hatchet WASM Binding Generation Is Manual

**Problem**: When C++ headers in `behave-lib/behave-mirror` change, WASM bindings must be regenerated via Hatchet. This is a manual step not automated in CI.

**Consequence**: A C++ change pushed to main without regenerating bindings silently fails at runtime (C++ method doesn't exist).

**Current mitigation**: Developer discipline (see SOLVER_TEST_HANDOFF.org for manual checklist).

**Recommendation**: Integrate Hatchet into CI or as a pre-commit hook.

### 8.8 CMS-Exports EDN Generation Process Is Undocumented

**Problem**: `cms-exports/*.edn` files are dated (some from Jul 2025, some from Jun 2024) but there's no documented procedure to regenerate them from CMS Datomic.

**Consequence**: Unclear whether these are stale snapshots or current; risky to depend on them if regeneration is not repeatable.

**Current status**: Files are checked into git, suggesting they're meant to be stable. But no script exports them.

**Recommendation**: Document the exact procedure (connect to CMS Datomic, query entities, export to EDN) and automate it in CI or as a Babashka task.

### 8.9 Worksheet SQLite Schema Not Documented

**Problem**: `.bp7` files are SQLite 3 databases with a schema not documented anywhere. Developers must inspect the DB or read `datom_store` code to understand the table structure.

**Consequence**: Hard to debug worksheet loading issues; unclear what happens to old .bp7 files after schema changes.

**Recommendation**: Document the .bp7 schema (table names, columns, relationships) in architecture docs or as an in-repo schema.sql file.

### 8.10 absurder_sql Integration Unclear

**Status**: Active Rust+ClojureScript DataScript replacement (branch `rj-ds-rust`, 30+ commits, 2025–2026 development) but not integrated into main build.

**Questions**:
- When will it be production-ready?
- Does it solve a performance bottleneck or is it exploratory?
- What's blocking integration (missing features, performance not yet validated)?

**Recommendation**: Clarify via design doc or mark as "research" until promotion criteria are met.

---

## 9. When NOT to Use This Skill

This skill documents architectural decisions and constraints. Other related skills cover adjacent concerns:

| When you need... | Use skill... | Not this one |
|------------------|-------------|-------------|
| Step-by-step reproduction and diagnosis for a specific bug | `behave-debugging-playbook` | ← |
| Chronicle of past incidents and root causes | `behave-failure-archaeology` | ← |
| Fire-science domain theory (Rothermel, GACC codes, units) | `fire-behavior-reference` | ← |
| VMS variable pipeline (C++ → WASM → Hatchet → CMS → layout.msgpack) | `behave-vms-variable-pipeline` | ← |
| Build environment setup and traps (node shim, EM_CACHE, externs) | `behave-build-and-env` | ← |
| Running dev/server/desktop/CMS, config axes, release operation | `behave-run-and-operate` | ← |
| Solver logging, test console capture, WASM debugging | `behave-diagnostics-and-tooling` | ← |
| Golden-data validation, test infrastructure, adding new tests | `behave-validation-and-qa` | ← |
| Docs standards, org-mode style, help-content authoring | `behave-docs-and-writing` | ← |
| absurder_sql campaign (phases, gates, decision logic) | `behave-absurder-sql-campaign` | ← |
| First-principles analysis: differential testing, layer bisection, migration dry-run | `behave-proof-and-analysis-toolkit` | ← |
| Open problems: absurder_sql standalone engine, solver perf, front-end perf | `behave-research-frontier` | ← |

**Seek this skill when**:
- Planning a change that touches storage, runtime modes, solver pipeline, or generated artifacts
- Adding a new component or base
- Integrating new modules or refactoring data flow
- Debugging unexpected nil values in queries or entity references

---

## 10. Provenance and Maintenance

All facts in this skill are verified against the deployed code as of **2026-07-06**.

### Re-verification Commands

Run these to detect drift and update the skill:

```bash
# Verify components directory structure
ls -1 /Users/rsheperd/code/sig/behave-app/components/ | wc -l
# Expected: ~25 directories

# Verify dual-mode runtime detection
grep -n "System.getProperty.*app.dir" /Users/rsheperd/code/sig/behave-app/projects/behave/src/clj/behave/core.clj
# Expected: line 66

# Verify three storage bases exist
ls -1 /Users/rsheperd/code/sig/behave-app/bases/ | grep -E "datom_store|datomic_store|datahike_store" | wc -l
# Expected: 3

# Verify datahike_store is unused in production code
grep -r "datahike_store" /Users/rsheperd/code/sig/behave-app/projects/*/src --include="*.clj" --include="*.cljs"
# Expected: (no matches; only test files have it)

# Verify solver module order
grep -n "run-module.*-module" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/solver/core.cljs | head -6
# Expected: 6 module invocations in order: surface, crown, contain, mortality, spot, ignite

# Verify WASM bindings are marked as generated
grep -l "Auto-generated by hatchet" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/*.cljs | wc -l
# Expected: 6+ files

# Verify cms-exports exists
ls -1 /Users/rsheperd/code/sig/behave-app/cms-exports/*.edn | wc -l
# Expected: 10+ EDN files (SIGSurface.edn, SIGCrown.edn, etc.)

# Verify version dual-source
grep "version = " /Users/rsheperd/code/sig/behave-app/projects/behave/conveyor.base.conf | head -1
cat /Users/rsheperd/code/sig/behave-app/projects/behave/resources/version.edn
# Expected: both show v7.1.4 (or next version)

# Verify no workspace.edn
ls -la /Users/rsheperd/code/sig/behave-app/workspace.edn 2>&1
# Expected: "No such file or directory"

# Verify VMS schema files exist
ls -1 /Users/rsheperd/code/sig/behave-app/bases/behave_schema/src/behave/schema/*.cljc | wc -l
# Expected: 25+ schema files

# Verify datomic_store remapping atoms are defined
grep -n "ds->datomic-eids\|datomic->ds-eids" /Users/rsheperd/code/sig/behave-app/bases/datomic_store/src/datomic_store/main.clj | head -2
# Expected: 2 matches (defonce atoms)

# Verify BHP1-1356 output-link priority comment
grep -n "BHP1-1356" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/solver/core.cljs
# Expected: 1 match at line ~165 commenting on empty-input priority

# Verify unused components
grep -c "^(defn\|^(def " /Users/rsheperd/code/sig/behave-app/components/data-utils/src/data_utils/interface.clj
# Expected: 15+ function definitions

# Verify empty interfaces
wc -l /Users/rsheperd/code/sig/behave-app/components/dita/src/dita/interface.clj
wc -l /Users/rsheperd/code/sig/behave-app/components/cucumber/src/cucumber/interface.clj
# Expected: both ~10 lines (docstring only, no code)

# Check if layout.msgpack exists and is recent
stat /Users/rsheperd/code/sig/behave-app/projects/behave/resources/public/layout.msgpack | grep Modify
# Expected: date within 1 month of deployment
```

### What Drifts Quickest

1. **Version number** — Updated in both conveyor.base.conf and resources/version.edn during releases; verify they stay in sync
2. **Component/base directory count** — New components added or old ones deleted; re-count periodically
3. **Headless test integration** — As of 2026-07-06, headless test runner exists on branch `rj-fix-figwheel-tests` but not merged to main; check if merged and integrated into CI/release
4. **absurder_sql status** — Active development on branch `rj-ds-rust`; check git log for recent commits and integration status
5. **CMS-exports file dates** — Should be regenerated with each CMS migration; compare git history timestamps
6. **WASM binding file count** — Currently 11 files with hatchet marker; update if new solver modules are added

---

**Document written**: 2026-07-06  
**Last updated**: 2026-07-06 (corrected default port, contain.cljs status, WASM binding list, file path, branch claims, added glossary)  
**Source repo**: https://github.com/firelab/behave-app (main branch)  
**Verified against**: commits c4c206ea (main, current), 206d7231 (branch rj-fix-figwheel-tests)
