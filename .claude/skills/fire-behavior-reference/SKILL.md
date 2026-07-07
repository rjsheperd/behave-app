---
name: fire-behavior-reference
description: Fire-science domain pack for Behave7's extended Rothermel model, six-module solver chain, crown-fire initiation, mortality equations, units system, and fuel-model concept — FOR THIS CODEBASE ONLY, verified against C++ classes and CLJS bindings.
---

# Fire-Behavior Reference: Behave7's Rothermel Model Implementation

## Scope and audience

This skill documents fire-behavior modeling **AS IMPLEMENTED IN THIS CODEBASE**, not fire science in general. Target: Opus-level agents needing to debug, modify, or integrate with Behave7's solver (`projects/behave/src/cljs/behave/solver/core.cljs`), the WASM C++ binding (`behave-lib/behave-mirror`), or the Variable Management System (VMS) that exposes fire inputs and outputs.

**Date-stamped: 2026-07-06.** Volatile facts (versions, branches, file locations) re-verifiable via commands in "Provenance and maintenance" section.

---

## The Six-Module Solver Chain

Behave7 implements the **extended Rothermel model** via six C++ modules orchestrated sequentially in ClojureScript. Each module has a WASM-compiled class (e.g., `SIGSurface`, `SIGCrown`) with a CLJS auto-generated binding namespace.

### Module execution order in `solver/core.cljs::solve-worksheet`

```
Surface (SIGSurface)
  ↓ (outputs → inputs via links)
Crown (SIGCrown)
  ↓
Contain (SIGContainAdapter)
  ↓
Mortality (SIGMortality)
  ↓
Spot (SIGSpot)  [runs if Surface OR Crown is active]
  ↓
Ignite (SIGIgnite)  [runs if Surface is active]
```

**Output linking rule:** A downstream module receives a previous module's output *only* if the input slot is empty (BHP1-1356, line 166 in `solver/core.cljs`). This prevents user-supplied inputs from being overwritten.

| Module | C++ Class | CLJS Namespace | Key run function | Inputs | Outputs |
|--------|-----------|---|---|---|---|
| **Surface** | `SIGSurface` | `behave.lib.surface` | `doSurfaceRun` | Fuel model, fuel moisture (1h/10h/100h/live-herb/live-woody), wind, slope | Rate of spread (ROS), fireline intensity (FLI), flame length, scorch height |
| **Crown** | `SIGCrown` | `behave.lib.crown` | `doCrownRun` [method-selected: `doCrownRunRothermel` or `doCrownRunScottAndReinhardt`] | Surface outputs (ROS, FLI), canopy base height, canopy height, canopy bulk density, canopy cover | Fire type (Surface/Torching/ConditionalCrownFire/Crowning), crown ROS, crown FLI, crown flame length |
| **Contain** | `SIGContainAdapter` | `behave.lib.contain` | `doContainRun` | Surface/Crown ROS/FLI, resource arrival times, production rates, L:W ratio, max size/time | Containment status, time to contain, perimeter at contain |
| **Mortality** | `SIGMortality` | `behave.lib.mortality` | `calculateMortalityAllDirections` | Species code (4-char FIA), GACC region, equation type, flame length OR scorch height, tree DBH, crown ratio, crown damage %, beetle damage, bole char height | Probability of mortality (%), scorch height, crown damage class |
| **Spot** | `SIGSpot` | `behave.lib.spot` | `calculateAll` | Fire type, flame length, location (ridge/valley/midslope), terrain, tree species, DBH | Spotting distance (firebrand travel) |
| **Ignite** | `SIGIgnite` | `behave.lib.ignite` | `calculateFirebrandIgnitionProbability` | Firebrand size, fuel bed composition, fuel moisture | Ignition probability (%) |

---

## Surface Module: Extended Rothermel Surface Fire Model

The **Surface** module calculates surface fire rate of spread (ROS) and fireline intensity using the **extended Rothermel surface fire model** (Rothermel 1972, as extended by Albini et al. in BEHAVE).

### Inputs to Surface

- **Fuel model** (`setFuelModelNumber`): Integer 1–224. Fuel models encode load (dead/live by size class), SAVR (surface area-to-volume ratio), depth, and extinction moisture for dead fuel.
  - Models 1–13: Standard NFDRS models (grass, shrub, timber).
  - Models 14–40+: Chaparral variants.
  - Models 101+: Custom or variant models (set via `setCustomFuelModel` in `fuel_models.cljs`).
  
- **Fuel moisture** (five classes, in fraction or percent):
  - **1-hour dead fuel** (`setMoistureOneHour`): 0–60% (driest twigs <1/4").
  - **10-hour dead fuel** (`setMoistureTenHour`): 0–60% (twigs 1/4"–1").
  - **100-hour dead fuel** (`setMoistureHundredHour`): 0–60% (twigs 1"–3").
  - **Live herbaceous** (`setMoistureLiveHerbaceous`): 0–100% (green understory plants).
  - **Live woody** (`setMoistureLiveWoody`): 0–100% (green branches, foliage).
  
  **Critical:** Fuel moisture drives the **reaction intensity** (heat released per unit area), which governs ROS and FLI. Drier fuels = faster spread.

- **Wind speed** (`setMidflameWindSpeed`): Measured at midflame height (typically 10–20 ft above ground), in ft/min, mi/h, km/h, or m/min. Wind alignment and direction mode configurable via `windUpslopeAlignmentMode` and `windAndSpreadOrientationMode` enums.

- **Slope** (`setSlope`): Degrees or percent. Slope increases ROS uphill exponentially.

- **Aspen/Chaparral variants** (optional): For chaparral fuel types, additional inputs (fuel depth, curing level) override standard fuel model data.

### Outputs from Surface

- **Rate of spread (ROS)** in direction of max spread: `getSpreadRateOfMaxSpread` (ft/min, ch/h, m/min, km/h).
- **Fireline intensity (FLI)** (Btu/ft/s or kW/m): `getFirelineIntensity` = ROS × heat per unit area. **Critical input to Crown module.**
- **Flame length** (ft, m): Calculated from FLI. `calculateFlameLength(fli_in, fli_units, fl_units)`.
- **Scorch height** (ft, m): Calculated from FLI and mid-flame wind. `calculateScorchHeight(fli_in, fli_units, wind_speed, wind_units, temp, temp_units, scorch_units)`.
- **Directional outputs**: `getBackingSpreadRate`, `getFlankingSpreadRate`, `getHeadSpreadRate` (fire spread in opposite, perpendicular, and aligned-to-wind directions).

---

## Crown Module: Crown Fire Initiation and Type Determination

The **Crown** module determines whether a surface fire can sustain crown fire and calculates crown-fire characteristics. Two crown-fire initiation methods are supported:

### Crown-Fire Initiation Methods

#### 1. Rothermel Crown Fire (scalar, threshold-based)

**Method:** `doCrownRunRothermel()`

Canopy fires initiate when surface FLI exceeds a **critical fire intensity threshold**, computed from:
- Canopy base height (CBH): Lowest canopy branch height (ft, m).
- Canopy bulk density (CBD): Oven-dry foliage mass per unit canopy volume (lb/ft³, kg/m³).
- Crown ratio: Crown length as fraction of tree height.

**Critical intensity:** I_c ∝ CBD × (CBH)^n. Higher CBD or lower CBH → lower ignition threshold → fires crown more easily.

**Crown fire type** determined by fire intensity:
- Surface fire: FLI < critical intensity.
- Torching: FLI crosses threshold; individual trees ignite.
- Conditional crown fire: Crowning starts but does not sustain rate of spread.
- Crowning: Sustained active crown fire, ROS = crown ROS.

#### 2. Scott & Reinhardt Crown Fire (empirical, probabilistic)

**Method:** `doCrownRunScottAndReinhardt()`

Empirical equations (Scott & Reinhardt 2001, PNW-GTR-534) model crown fire initiation probability as a function of:
- Surface FLI or flame length.
- Canopy base height.
- Crown fraction burned (CBH / canopy height).
- Wind speed at midflame.

**Outputs:** Crown fire type (probability-weighted) and crown ROS. Considered more empirically robust than Rothermel for mixed-conifer forests.

### Crown Module Inputs

- **Surface outputs** (auto-linked): FLI, ROS, flame length from Surface module.
- **Canopy geometry:**
  - `setCanopyBaseHeight()` (ft, m): Distance from ground to lowest crown foliage.
  - `setCanopyHeight()` (ft, m): Total tree height.
  - `setCanopyBulkDensity()` (lb/ft³, kg/m³): Dry foliage load per canopy volume.
  - `setCanopyCover()` (fraction, %): Proportion of area beneath canopy.
- **Crown method selection** (enum `crown-fire-calculation-method`):
  - `:rothermel` → `doCrownRunRothermel()`
  - `:scott-and-reinhardt` → `doCrownRunScottAndReinhardt()`

### Crown Module Outputs

- **Fire type** (`getFireType()` → `fire-type` enum):
  - `Surface`: Subsurface fire only.
  - `Torching`: Surface fire with sporadic torching.
  - `ConditionalCrownFire`: Crowning begins but doesn't sustain.
  - `Crowning`: Active crown fire with independent ROS.
  
- **Crown ROS** (`getCrownFireSpreadRate()`): If crowning, rate of crown fire spread (may exceed surface ROS). Otherwise undefined/zero.
- **Crown FLI** (`getCrownFirelineIntensity()`): Intensity of crown fire.
- **Crown flame length** (`getCrownFlameLength()`).
- **Transition metrics:**
  - `getCriticalOpenWindSpeed()`: Wind speed at crown fire initiation.
  - `getCrownCriticalSurfaceFirelineIntensity()`: FLI threshold for crowning.

---

## Contain Module: Fire Suppression Resource Containment

The **Contain** module models fire spread and resource-based suppression. It computes **time to contain** and final perimeter given:
- Surface or crown fire spread characteristics (ROS, FLI).
- Suppression resources (arrival times, production rates).
- Tactical parameters (attack distance, L:W ratio, max size/time).

### Contain Inputs

Resource parameters:
- `addResource(arrival_time, arrival_unit, duration, duration_unit, production_rate, production_rate_unit, description)`: Add a suppression resource (crew, engine, helicopter) arriving at time T with production rate P.
  - Arrival time (minutes, hours): When resource is on scene.
  - Duration: How long resource works.
  - Production rate (ft/min, m/min, ch/h, km/h): Rate at which resource advances the fire line (containment perimeter).

Tactical:
- `setLwRatio(lwRatio)`: Fire perimeter length-to-width ratio (ratio, dimensionless). Typical values 1–4; higher = more elongated (directed toward flanks).
- `setAttackDistance(distance, units)` (ft, m): Distance from fire origin to initial resource position.
- `setMaxFireSize(size, units)` (acres, hectares): Upper limit on fire growth (fire ends if exceeded).
- `setMaxFireTime(time)`: Upper limit on simulation time (minutes).
- `setReportRate(speed, units)`: Fire spread rate at report time (for initial conditions).
- `setReportSize(size, units)`: Fire size at initial report.

### Contain Outputs

- **Containment status** (`getContainStatus()` → `contain-status` enum):
  - `Contained`: Fire reached steady state within resource capacity.
  - `Uncontained`: Resources insufficient.
  - `Overrun`: Fire overcame a resource.
  - `Exhausted`: Resource ran out of time/capability.
  - `SizeLimitExceeded`: Fire exceeded `maxFireSize`.
  - `TimeLimitExceeded`: Simulation ran over `maxFireTime`.

- **Time to contain** (`getTimeToContainment()`): Minutes from fire start to full containment (if achieved).
- **Containment perimeter** (`getFinalPerimeter(units)`), **fire area** (`getFinalFireSize(units)`).

---

## Mortality Module: Post-Fire Tree Mortality

The **Mortality** module calculates the **probability of tree mortality** given fire scorch and crown damage. It combines empirical **species-specific equations** (Van Mantgem & Schwilk 2009, others) with inputs describing fire damage and tree characteristics.

### Mortality Species and GACC Regions

Species are identified by **FIA species code** (4-character string, e.g., `PIPO` = Ponderosa Pine, `ABAM` = Silver Fir). The module uses a **species master table** compiled for each GACC (Geographic Area Coordination Center) region.

**GACC regions** (9 regions covering USFS fire management):
- `Alaska`, `California`, `EasternArea`, `GreatBasin`, `NorthernRockies`, `Northwest`, `RockeyMountain`, `SouthernArea`, `Southwest`.

**Critical:** A species is only "found" if it exists in the species master table for the set GACC region. Variant codes (>4 characters, e.g., `ABGRI`, `ABBAB`) are region-specific subspecies or varieties; they resolve only under their matching GACC. The test handoff (MORTALITY_TEST_HANDOFF.org) notes ~7.4k test rows fail because the hard-coded region doesn't match the variant's provenance.

### Mortality Equations (Three Types)

1. **Crown scorch** (`equation-type`: `:crown_scorch`):
   - Input: Scorch height (ft, m) or flame length (ft, m) [via enum `flame-length-or-scorch-height-switch`].
   - Output: Probability of mortality (0–100%) based on crown scorch volume.

2. **Crown damage class** (`equation-type`: `:crown_damage`):
   - Input: Crown damage % (0–100), crown ratio (fraction), beetle damage (enum: no/yes), cambium kill rating (0–4).
   - Output: Mortality probability (multi-factor: damage × tree vigor).
   - **Note:** C++ WASM uses "inert" model for CRCABE (crown_damage) type — crown damage from fire damage is deferred in the current implementation (see variables_mapping.org tag `#fix`).

3. **Bole char** (`equation-type`: `:bole_char`):
   - Input: Bole char height (ft, m), bark thickness (in, cm), DBH (diameter at breast height).
   - Output: Cambium kill probability → mortality risk.

### Mortality Inputs

- `setGACCRegion(region)` (enum `gacc`): **Required**. Sets the species table context. Species lookup fails if region doesn't match the species.
- `setSpeciesCode(code)` (string, e.g., `"PIPO"`)
- `setEquationType(eq_type)` (enum `equation-type`: `crown_scorch`, `crown_damage`, `bole_char`)
- `setFlameLengthOrScorchHeightSwitch(switch_enum)` (enum: `flame_length` or `scorch_height`)
- `setFlameLengthOrScorchHeightValue(value, units)` (ft, m, depending on switch)
- `setDBH(diameter, units)` (in, cm)
- `setTreeHeight(height, units)` (ft, m)
- `setCrownRatio(ratio, units)` (fraction or %)
- `setCrownDamage(damage_pct)` (%, 0–100)
- `setCambiumKillRating(rating)` (0–4, where 4 = complete kill)
- `setBeetleDamage(damage_enum)` (enum: `no`, `yes`)
- `setBoleCharHeight(height, units)` (ft, m) [bole char type only]
- `setTreeDensityPerUnitArea(density, units)` (trees/acre, trees/ha)

### Mortality Outputs

- **Probability of mortality** (`getProbabilityOfMortality(units)`, percent or fraction): 0–100%, the primary output.
- **Scorch height** (if computed): `getScorchHeight()` (ft, m).
- **Crown damage class** (`getCrownDamage()`): Integer 0–4 representing crown scorch severity.
- **Species lookup results:**
  - `getCommonNameFromSpeciesCode(code)` (string): Common name of species.
  - `getMortalityEquationNumberFromSpeciesCode(code)` (integer): Which equation within species table.
  - `checkIsInGACCRegionFromSpeciesCode(code, region)` (boolean): Does this species exist in the region? **Used to validate region/species pairing.**

---

## Spot Module: Spotting Distance Calculations

The **Spot** module calculates **firebrand spotting distance** — how far glowing embers travel downwind and ignite spot fires. It models three firebrand sources:

### Spotting Sources

1. **Surface fire spotting** (`calculateSpottingDistanceFromSurfaceFire()`):
   - Flame length as firebrand launch height.
   - Wind speed carries embers downwind.
   - Distance ∝ flame length × wind.

2. **Torching tree spotting** (`calculateSpottingDistanceFromTorchingTrees()`):
   - Torch height and flame length.
   - Wind carries from crown.

3. **Burning pile spotting** (`calculateSpottingDistanceFromBurningPile()`):
   - Pile flame height, pile size.
   - Typically used for slash burns.

### Spot Inputs

- `setFireType(fire_type)` (enum `fire-type`: Surface/Torching/ConditionalCrownFire/Crowning)
- `setLocation(location)` (enum `spot-fire-location`: MidslopeWindward/ValleyBottom/MidslopeLeeward/RidgeTop)
- `setFlameLength(length, units)` (ft, m)
- `setDBH(dbh, units)` (in, cm) [for tree spotting]
- `setTreeSpecies(species)` (enum `spot-tree-species`, limited set: Engelmann spruce, Douglas fir, etc.)
- `setRidgeToValleyDistance(distance, units)` (ft, m)
- `setDownwindCoverHeight(height, units)` (ft, m) [vegetation that catches firebrands]
- `setDownwindCanopyMode(mode)` (enum `spot-down-wind-canopy-mode`: CLOSED, OPEN)

### Spot Outputs

- **Spotting distance** (`getSpottingDistanceFromSurfaceFire()`, etc.) (ft, m, miles, km): Maximum downwind distance firebrands travel.

---

## Ignite Module: Firebrand Ignition Probability

The **Ignite** module computes the probability that a firebrand ignites fuel, given **firebrand characteristics** (size, temperature) and **fuel bed state** (moisture, type).

### Ignite Inputs

- `setFirebrandDiameter(diameter, units)` (in, mm)
- `setFirebrandTemperature(temperature, units)` (°F, °C)
- `setFuelBedType(type)` (enum `ignition-fuel-bed-type`: e.g., PonderosaPineLitter, DouglasFirDuff, PeatMoss)
- `setFuelMoisture(moisture, units)` (%, fraction)
- `setFuelTemperature(temperature, units)` (°F, °C)

### Ignite Outputs

- **Ignition probability** (`getIgnitionProbability()`, 0–100%): Likelihood firebrand kindles fuel.

---

## Units System: CLJS ↔ C++ Bridge

Behave7 implements a **dual-unit system**: each quantity has a numeric value and a unit enum. The CLJS `behave.lib.units` namespace bridges user-selected units (English/metric) to WASM C++ base units (always in SI or NFDRS-default feet).

### Unit Architecture

**CLJS side** (`projects/behave/src/cljs/behave/lib/units.cljs`):

1. **Unit catalog** (lines 9–26): 17 dimensions (AreaUnits, SpeedUnits, TemperatureUnits, etc.) — each maps a C++ class name to conversion functions.
   ```
   - AreaUnits → {to-fn: toBaseUnits, from-fn: fromBaseUnits}
   - SpeedUnits → …
   ```

2. **Unit symbol lookup** (lines 48–103): Flat maps of short-hand symbols (e.g., `"ft/min"`, `"m/s"`, `"ch/h"`) to unit specs:
   ```clojure
   {:short "ch/h" :system "english" :enum enum/speed-units :dimension :speed :unit "ChainsPerHour"}
   ```
   The `:unit` field is the C++ enum name (e.g., `ChainsPerHour`) and `:enum` is the CLJS enum map (generated from C++).

3. **Conversion functions** (`convert`, line 133):
   ```clojure
   (convert value from to decimals)  ;; e.g., (convert 100 "ft/min" "ch/h" 2) → 0.27
   ```
   Strategy: value in user `from` units → call C++ `toBaseUnits(from_enum)` → C++ `fromBaseUnits(to_enum)` → user `to` units. **Result is always a double; no enum is returned.**

**C++ side** (`behave-lib/behave-mirror/src/behave/…Units.h`, e.g., `SpeedUnits.h`):

- Each unit dimension is a struct with static methods `toBaseUnits(enum)` and `fromBaseUnits(enum)`.
- Base units vary by dimension:
  - Speed: ft/min
  - Length: ft
  - Area: sq ft
  - Temperature: °F (internally Kelvin)
  - etc.
- Emscripten WebIDL binding exposes these via `Module.SpeedUnits.prototype.toBaseUnits`, etc.

### Units Bug Hazards

1. **Silent nil unit arity drop** (SOLVER_TEST_HANDOFF.org):
   - `apply-single-cpp-fn` (solver/core.cljs line 34) checks `(count params)`. If a setter expects 2 args (value + unit) but unit is `nil`, the function is called with only value → C++ sees garbage/default for unit → wrong conversion or skipped setter.
   - **Fix:** Always ensure unit-uuid is resolved before calling C++ setters; wrap in assertions.

2. **Unit-uuid fallback chains** (solver/core.cljs line 138):
   - Worksheet unit for an output = `[cached-unit OR native-unit OR :none]` (in priority order).
   - If native unit is a DataScript entity ref (not a UUID string), fallback silently returns a ref ID (e.g., `4874`) instead of a unit enum → downstream code treats it as a unit and fails.

3. **Percent ambiguity**: Two distinct "%" entities exist in VMS (uuids `651dadb7-66f5…` and `69249141-e52c…`), one for fractions and one for slope. Always disambiguate via dimension.

4. **M/H** (meters per hour): Marked `FIXME` in units.cljs line 87; metric speed system has `m/h` but it's not fully validated against C++.

### How to Verify Units in a Solver Run

Solver logs all unit conversions to the browser console via `:SOLVER` log entries (solver/core.cljs line 30, `log-solver` helper). Watch for:
- `[:SOLVER :SINGLE-VAR gv-id value unit]`: Logged unit should not be `nil`.
- `[:SOLVER :MULTI-UNITS gv-uuid unit]`: Each param's unit (extracted from repeat-group).
- If unit is `nil`, check VMS unit assignment for that group-variable.

---

## Fuel Models Concept

A **fuel model** is a parameterized description of surface fuel: dead fuel loads by timelag class (1h, 10h, 100h), live herb/woody loads, SAVR (surface area-to-volume ratio), fuel bed depth, and heat of combustion (dead and live). Fuel models encode fire behavior for a vegetation type.

### Fuel Model Numbering

- **1–13**: Standard NFDRS fuel models (short grass, timber litter, chaparral, etc.).
- **14–40+**: Chaparral variants (age, type specific).
- **101+**: Custom (user-defined or sidecar fuel models not in standard set).

### Setting a Fuel Model

```clojure
;; Get load for fuel model 2, 1-hour dead fuel, in tons/acre
(surface/getFuelLoadOneHour module 2 (enums/loading-units "TonnesPerAcre"))

;; Set custom fuel model 101
(fuel-models/setCustomFuelModel
  module 101                                          ;; fuel model number
  "CUSTOM"                                            ;; code
  "My Custom Fuel"                                    ;; name
  10 (enums/length-units "Feet")                      ;; fuel bed depth
  50 (enums/moisture-units "Percent")                 ;; extinction moisture (dead)
  8000 (enums/heat-combustion-units "BtusPerPound")   ;; heat of combustion (dead)
  8000 (enums/heat-combustion-units "BtusPerPound")   ;; heat of combustion (live)
  2 (enums/loading-units "TonnesPerAcre")             ;; 1-hour load
  1.5 (enums/loading-units "TonnesPerAcre")           ;; 10-hour load
  1 (enums/loading-units "TonnesPerAcre")             ;; 100-hour load
  0.5 (enums/loading-units "TonnesPerAcre")           ;; live herb load
  0.2 (enums/loading-units "TonnesPerAcre")           ;; live woody load
  2000 (enums/surface-area-to-volume-units "SquareFeetOverCubicFeet")   ;; 1-hour SAVR
  2000 (enums/surface-area-to-volume-units "SquareFeetOverCubicFeet")   ;; live herb SAVR
  1500 (enums/surface-area-to-volume-units "SquareFeetOverCubicFeet")   ;; live woody SAVR
  true)                                               ;; is dynamic (user-defined)
```

### Fuel Model Parameters and Units

All parameters go through the units system (toBaseUnits/fromBaseUnits). **Load** is typically stored in base units (lb/ft²) internally; **SAVR** in ft²/ft³.

---

## Glossary: Key Terms and Code Locations

| Term | Meaning | Where in Code |
|------|---------|---------------|
| **Rothermel model** | Extended surface fire model (Rothermel 1972); models ROS via fuel load, moisture, wind, slope | `behave-lib/behave-mirror/src/behave/surface.h` |
| **Extended Rothermel** | Rothermel + crown fire + containment + mortality modules | `projects/behave/src/cljs/behave/solver/core.cljs` lines 251–297 (module init) |
| **ROS (Rate of Spread)** | Fire spread speed (ft/min, ch/h, m/min, km/h) | `surface/getSpreadRateOfMaxSpread()`, `crown/getCrownFireSpreadRate()` |
| **FLI (Fireline Intensity)** | Heat energy release per unit fireline length per unit time (Btu/ft/s, kW/m); I = ROS × HPA | `surface/getFirelineIntensity()`, `crown/getCrownFirelineIntensity()` |
| **Flame length** | Visible flame height above fuel surface (ft, m); calculated from FLI | `surface/calculateFlameLength()` |
| **Scorch height** | Height of crown scorch caused by fire; inputs to mortality equations | `surface/calculateScorchHeight()`, `mortality/getScorchHeight()` |
| **CBH (Canopy Base Height)** | Height of lowest hanging foliage (ft, m); determines crown fire ignition threshold | `crown/setCanopyBaseHeight()` |
| **CBD (Canopy Bulk Density)** | Dry foliage load per canopy volume (lb/ft³, kg/m³); high CBD = easier crowning | `crown/setCanopyBulkDensity()` |
| **Crown ratio** | Crown length as fraction of tree height; input to crown fire & mortality | `crown/setCanopyHeight()`, `mortality/setCrownRatio()` |
| **Fire type** | Classification: Surface, Torching, ConditionalCrownFire, Crowning | `crown/getFireType()` → `fire-type` enum (enums.cljs line 159) |
| **Fuel model** | Parameterized description of surface fuel (loads, SAVR, depth, extinction moisture) | `behave.lib.fuel-models` namespace, `fuel-models.cljs` |
| **SAVR** | Surface area-to-volume ratio (ft²/ft³); small particles → high SAVR → fast ignition | `fuel-models.cljs` getters (e.g., `getSavrOneHour`) |
| **Moisture of extinction** | Fuel moisture above which fire cannot spread; varies by fuel model | `fuel-models.cljs getMoistureOfExtinctionDead()` |
| **GACC (Geographic Area Coordination Center)** | USFS region; mortality species table is GACC-specific (e.g., "Southern Area", "Northwest") | `enums.cljs` line 296, enum `gacc` |
| **FIA species code** | 4-char code identifying tree species (e.g., PIPO = Ponderosa Pine, ABAM = Silver Fir) | `mortality/setSpeciesCode()`, species master table in C++ |
| **Species master table** | GACC-specific lookup table of tree species; mortality equations keyed by species | `behave.lib.species-master-table`, `mortality/init()` line 7 |
| **Equation type** | Classification of mortality input: crown_scorch, crown_damage, bole_char | `enums.cljs` line 123, enum `equation-type` |
| **Crown damage class** | 0–4 categorical severity of crown burn (for crown_damage equation type) | `mortality/getCrownDamage()` |
| **Spotting distance** | Distance firebrand (glowing ember) travels downwind before landing (ft, m, miles) | `spot/getSpottingDistanceFromSurfaceFire()`, etc. |
| **Firebrand** | Glowing coal/ember lifted by fire and transported by wind | Spot & Ignite modules |
| **L:W ratio** | Fire perimeter length-to-width ratio (dimensionless); models fire ellipse eccentricity | `contain/setLwRatio()` |
| **Resource production rate** | Rate at which suppression resource advances containment line (ft/min, ch/h, etc.) | `contain/addResource()` |
| **Units UUID** | Unique identifier (UUID string) for a unit in the VMS; links inputs/outputs to unit enum | `solver/core.cljs` line 111, `unit-uuid` parameter |
| **Worksheet** | User's fire scenario; persisted as SQLite `.bp7` file; contains inputs, computed outputs, parametric runs | `.bp7` fixtures in `worksheets/` |
| **Output linking** | Mechanism by which downstream module receives upstream output if its input slot is empty | `solver/core.cljs` line 157, `apply-output-links()` |
| **Hatchet** | Tool that generates CLJS/WebIDL bindings from C++ headers via ANTLR; auto-generates `behave/lib/*.cljs` | `behave-lib/` build process |
| **WASM (WebAssembly)** | Compiled C++ code (`behave-mirror`) running in browser as binary module | `projects/behave/resources/public/js/`, `Module` object |

---

## When NOT to use this skill

This skill covers fire-behavior domain theory AND implementation. Use sibling skills for:

- **Architecture and design decisions:** behave-architecture-contract (Why does the solver chain inputs/outputs? What invariants exist?)
- **Building/environment/compilation:** behave-build-and-env (Node shim, Emscripten, WASM bootstrap, Nix setup, externs)
- **Running and configuring:** behave-run-and-operate (Server mode, dev server, config.edn, research-mode gates, .bp7 loading)
- **VMS pipeline (C++ → Hatchet → CMS → layout.msgpack):** behave-vms-variable-pipeline (How do I add a new fire input? What are the 8 steps?)
- **Testing and validation:** behave-validation-and-qa (Golden test data, test tier runbook, how to write a test)
- **Debugging failures:** behave-debugging-playbook (Symptom → triage table; discriminating experiments)
- **Past incidents and root causes:** behave-failure-archaeology (Unit arity bug, WASM bootstrap timing, migration sync, species table)
- **Absurder_sql campaign:** behave-absurder-sql-campaign (Unmerged Rust/SQLite branch; decision gates)
- **Documentation and style:** behave-docs-and-writing (Org-mode house style, ticket playbook, help-content authoring)

---

## Provenance and Maintenance

**As of 2026-07-06**, re-verify these facts via:

| Fact | Verification command | Expected output |
|------|---------------------|---|
| Six modules in solver, in order: Surface → Crown → Contain → Mortality → Spot → Ignite | `grep -A 60 "^(defn solve-worksheet" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/solver/core.cljs \| grep -E "(defn \|run-fn\|cond->)"` | Lines show surface-module, crown-module, contain-module, mortality-module, spot-module, ignite-module defined and cond→ applied in order |
| Solver calls `doSurfaceRun`, `doCrownRun`, etc. on each module | `grep "run-fn" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/solver/core.cljs` | `:run-fn surface/doSurfaceRun`, `:run-fn crown/doCrownRun`, etc. |
| Crown module has Rothermel and Scott-and-Reinhardt methods | `grep -E "doCrownRunRothermel\|doCrownRunScottAndReinhardt" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/crown.cljs` | Two function definitions present |
| Units are bridged via toBaseUnits/fromBaseUnits | `grep -E "to-fn\|from-fn" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/units.cljs` | Lines show function access via prototype (lines 34–35) |
| Fuel models are auto-generated by Hatchet | `head -5 /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/fuel_models.cljs` | Comment `; Auto-generated by hatchet 🪓` at line 3 |
| 10-hour dead fuel moisture setter: `setMoistureTenHour` (not `setMoistureOneHour`) | `grep -E "setMoistureOneHour\|setMoistureTenHour" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/surface.cljs` | Two separate function definitions; 1-hour uses `setMoistureOneHour`, 10-hour uses `setMoistureTenHour` |
| GACC enum has 9 regions: NorthernRockies (one 'h'), RockeyMountain (not Rocky) | `grep -A 10 "^(def gacc" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/enums.cljs` | Enum shows `"GACC::NorthernRockies"` and `"GACC::RockeyMountain"` |
| Mortality equation types: crown_scorch, crown_damage, bole_char | `grep -A 5 "^(def equation-type" /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/enums.cljs` | Three EquationType entries visible |
| Output linking implemented at line 166 (BHP1-1356) | `sed -n '165,167p' /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/solver/core.cljs` | Comment referencing BHP1-1356 and condition `(nil? (get-in acc ...))` |
| Units FIXMEs in units.cljs | `grep FIXME /Users/rsheperd/code/sig/behave-app/projects/behave/src/cljs/behave/lib/units.cljs` | Lines 45, 69, 71, 87, 94 show FIXMEs for Contain Fire Points, m/h, Tree Count/Density |

---

## References and Further Reading

**Peer-reviewed papers** (not housed in repo; external citations for domain authority):

1. **Rothermel, R. C. (1972).** "A mathematical model for predicting fire spread in wildland fuels." USDA Forest Service Research Paper INT-115. (Extended Rothermel surface fire model.)
2. **Scott, J. H., & Reinhardt, E. D. (2001).** "Assessing crown fire potential by linking models of surface and crown fire behavior." USDA Forest Service Research Paper PNW-GTR-534. (Crown fire initiation alternatives.)
3. **Van Mantgem, P. J., & Schwilk, D. W. (2009).** "Mutual dependence of aspens and fires in the northern Rocky Mountains." Forest Ecology and Management, 258(10), 2265–2272. (Post-fire mortality.)

**In-repo domain handoffs:**

- `/Users/rsheperd/code/sig/behave-app/MORTALITY_TEST_HANDOFF.org`: Species coverage (GACC regions, FIA codes, variant codes), species table issues, -100 sentinel.
- `/Users/rsheperd/code/sig/behave-app/SOLVER_TEST_HANDOFF.org`: Units arity drop, unit-uuid fallback chains, silent setter skipping.
- `/Users/rsheperd/code/sig/behave-app/FIX_TEST_PLAN.org`: Full test campaign context (green date, standing reds).

**C++ source (behave-mirror):**

- `behave-lib/behave-mirror/src/behave/surface.h`: Surface fire model class signature.
- `behave-lib/behave-mirror/src/behave/crown.h`: Crown fire classes.
- `behave-lib/behave-mirror/src/behave/mortality_inputs.h`: Mortality parameter structures.

**CLJS generated bindings:**

- `projects/behave/src/cljs/behave/lib/surface.cljs` → `SIGSurface` methods.
- `projects/behave/src/cljs/behave/lib/crown.cljs` → `SIGCrown` methods.
- `projects/behave/src/cljs/behave/lib/mortality.cljs` → `SIGMortality` methods.
- `projects/behave/src/cljs/behave/lib/enums.cljs` → Unit enums, species, fire-type, GACC, equation-type, etc.

---

**End of fire-behavior-reference skill.**
