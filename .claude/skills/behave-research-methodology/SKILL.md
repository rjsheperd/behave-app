---
name: behave-research-methodology
description: Discipline for turning hunches into accepted results — evidence bars, hypothesis prediction, adversarial refutation, spike lifecycle, and retirement protocols.
---

# Behave Research Methodology

This skill documents the discipline for accepting or rejecting ideas in this repository, with worked examples from the investigation history. It covers:

1. **The evidence bar** — how one mechanism must explain *all* observations (including negatives)
2. **Hypothesis-predicts-numbers-before-running** — predictions written before the experiment
3. **Idea lifecycle** — spike → harness → benchmarks → promotion or documented retirement
4. **Provenance of good ideas** — where they historically came from in this repo
5. **When to stop** — criteria for abandoning a direction and the artifact to leave behind

The methodology lives in the handoff docs and test playbooks rooted in the project root (`MORTALITY_TEST_HANDOFF.org`, `SOLVER_TEST_HANDOFF.org`, `FIX_TEST_PLAN.org`); this skill is the *meta-discipline*. See also `behave-proof-and-analysis-toolkit` for experimental recipes and `behave-change-control` for promotion gates.

---

## Glossary of Key Terms

Readers new to this repo should understand:

- **GACC**: Geographic Area Coordination Center — a regional forest-fire coordination unit; this repo's fire-science models are GACC-region-specific (e.g., SouthernArea, Northwest). Enum values stored in WASM species table.
- **WASM**: WebAssembly — the C++ fire-behavior model (behave-lib) compiled to binary; runs in the browser. Generated wrappers in `behave/lib/*.cljs` call WASM methods.
- **VMS**: Variable Management System — a Datomic-backed CMS (port 8001) that stores fire-science model structure (modules, variables, units, equations). Exported as `layout.msgpack` and loaded into the client's DataScript at startup.
- **Hatchet**: Code-generation tool (ANTLR-based) that reads C++ headers and auto-generates ClojureScript wrapper functions (e.g., `behave/lib/surface.cljs`). Output lives in `behave/lib/*.cljs` and `cms-exports/*.edn`.
- **CSV (golden data)**: Reference datasets (`mortality.csv`, `surface.csv`, etc. in `behave-lib/test/csv/`) containing test inputs and expected outputs, used to validate solver correctness.
- **FOFEM**: Fire and Fuels Extension to the Forest Vegetation Simulator — an external reference model used to validate Behave7 outputs. Used in the mortality investigation to establish the golden dataset scope.
- **behave-mirror**: C++ submodule (branch `rj-rust-port`) containing the original Rothermel fire-behavior equations. Compiled to WASM, fed by VMS via layout.msgpack.
- **Fixtures**: `.bp7` SQLite worksheet files (stored in `worksheets/` dir) containing sample input/output runs used in tests.
- **Solver**: The orchestration logic in `projects/behave/src/cljs/behave/solver/core.cljs` that chains six fire-science modules (Surface → Crown → Contain → Mortality → Spot → Ignite) and applies output linking.

---

## I. The Evidence Bar — One Mechanism for All Observations

**Principle**: A theory must account for *every* failing row, every passing row, and every negative result. If you find yourself explaining three different problems with three different causes, you have incomplete evidence. The discipline is to force unification until one coherent story explains the data, or accept multiple independent root causes with disjoint evidence.

**Example from this repo**: The mortality test failure investigation (as of 2026-07-02, documented in `MORTALITY_TEST_HANDOFF.org`).

### The Setup

The test iterates 15,052 rows of `mortality.csv` (fire-behavior golden data), comparing computed tree mortality % against a reference column, with tolerance ±4.0 absolute.

**Initial observation**: Two classes of failures:
- ~7.4k rows: observed = -100 (the "species not found" sentinel)
- ~3.4k rows: resolves but value is wrong (e.g., expected 95 / observed 28.3)

**Naive hypothesis**: "It's a species-coverage problem and a value-accuracy problem." This was *wrong* because it implied two independent bugs, and the evidence did not unify.

### Three Competing Hypotheses

Before running the investigation, the handoff documented three testable predictions:

1. **H1** (variant codes are GACC-region-specific): Iterate all 10 GACC regions on a -100 row (e.g., `ABGRI` / CRNSCH); one region should yield non-(-100).
   - **Falsifiable by**: If no region yields non-(-100), H1 is disproven.

2. **H2** (WASM species table is a build subset): Call `getNumberOfRecordsInSpeciesTable()` and compare against the 525 CSV codes.
   - **Falsifiable by**: If the table contains all 525 codes, H2 is disproven.

3. **H3** (CSV codes are invalid/renamed): Diff the CSV species set against `getSpeciesCodeAtSpeciesTableIndex(i)` over all table indices.
   - **Falsifiable by**: If a 1-to-1 correspondence exists, H3 is disproven.

### Adversarial Refutation

The investigation ran each test:

**H1 disproven**: Tested ABAM (a base 4-char code) under hardcoded SouthernArea. It computes fine (observed ~65) *even though* `checkIsInGACCRegionFromSpeciesCode(ABAM, SouthernArea)` returns false. So region-gating is not the mechanism. (Later confirmed via live WASM: base codes always resolve; variant codes always return -1 → -100, regardless of region.)

**H2 confirmed**: `getNumberOfRecordsInSpeciesTable()` = 197 records (190 distinct 4-char FIA codes). The CSV references 525 distinct codes; 339 are absent. Direct spot-checks: `ABGRI2`, `ABISPP`, `ABLO` → index -1 → -100; `ABGR`, `POTR12` → resolve.

**H3 confirmed as secondary**: The 339 absent codes include 157 that are *aliases* of a 4-char base (identical mortality/bark equations) and 183 with *no* base code in the table.

### Root Cause A (Unified Explanation)

**The compiled WASM species table is the reduced GACC-scope set** (197 records). mortality.csv is over-broad: it references the OLD full table (526 codes). The fix is *not* to add a 4-char-fallback hack (wrong for 183/339); instead, **regenerate mortality.csv from the model's known golden set**, which is the C++ reference in `behave-mirror/src/testMortality/resultsProbMort.csv` (3,717 CRNSCH + 108 BOLCHR rows, all with species in the 197-record table).

**Result**: All 7,119 -100 rows are eliminated by this single mechanism.

### Root Cause B (Independent Second Mechanism)

After fixing Root Cause A, a separate investigation revealed:

**The crown_damage (CRCABE) equation is inert in the WASM**. Every one of 3,832 CRCABE rows returns a constant ~1.5-3% mortality regardless of crownDamage %, CambiumKillRating, or beetle flags — *only* DBH moves it. Direct WASM testing (calling `setters` with and without values) confirmed the input path is ignored.

**Decisive evidence**: The upstream C++ reference (`resultsProbMort.csv`) contains 3,717 CRNSCH + 108 BOLCHR, but *zero* CRCABE rows. So the golden data never validated crown_damage. This is a separate issue from species coverage, with its own root cause (model bug in C++), and its own data scope.

**Result**: All 3,832 CRCABE rows are dropped because the model does not yet validate them.

### Lesson: One Mechanism per Failure Class

The discipline forced the investigation to **split the failure set into two disjoint classes** (7,119 rows with mechanism A; 3,832 rows with mechanism B), each explained by one root cause with independent evidence. This is *better* than forcing one theory to cover both (which would have been wrong).

---

## II. Hypothesis-Predicts-Numbers-Before-Running

**Principle**: Write down the *exact* prediction and the form to test it *before* you run the experiment. This prevents anchoring bias ("I got 19.67, so I'll believe the model if it's within 20").

### The Setup (from SOLVER_TEST_HANDOFF.org)

The `surface-worksheet` test computes spread rate. The old synthetic path gave ~19.9 ch/h (off by 4x from the golden 19.68 ch/h). The new worksheet-backed path gave 82.44 ch/h (even worse).

**Before running the investigation**, the handoff listed three hypotheses *in priority order* with exact predictions:

| Hypothesis | Prediction | Test form |
|-----------|-----------|-----------|
| **H1**: `solve-ws-outputs` omits the `update-input-units` dispatch loop | Calling `solve-worksheet` without the loop leaves values unnormalized; fixing by running the loop should move observed **toward 19.68** | Add the loop to `solve-ws-outputs`, re-run, compare to 19.68 |
| **H2**: Value+unit → WASM setter transform is broken | The WASM setter receives either the wrong unit enum or an unconverted value; tracing should show divergence from direct calls | Instrument `apply-single-cpp-fn`, log `[fn-name value unit]` for wind/slope/moisture, compare to direct-call baseline |
| **H3**: Wrong group-variable chosen for an input | Wind has multiple setter aliases (10_meter vs midflame vs wind_speed); a mismatch in native units could amplify the value | Check that wind and slope group-variables have dimensions matching the input (e.g., wind native unit is mi/h, not some other unit) |

### The Experiment

Priority was tested first. The investigation:
1. Checked that hypothesis H1 was *actually* being violated (re-ran the path manually, confirmed units loop was skipped).
2. Fixed H1 by making `solve-ws-outputs` mirror the real 1-arity path.
3. Re-ran `surface-worksheet`; observed dropped to **19.67758433** vs golden **19.677584** — *prediction confirmed, hypothesis accepted*.
4. (H2 and H3 were never needed because H1 explained the full gap.)

### Bonus: Quantitative Predictions

The investigation also noted **intermediate values that constrain the search**:

- Direct WASM call, wind mode TwentyFoot (correct): 19.68 ch/h.
- Direct WASM call, wind mode DirectMidflame (wind NOT reduced by WAF): 41.82 ch/h.
- Worksheet solve (buggy): 82.44 ch/h (higher than both).

This tells you the bug is not just a wind-unit mismatch (would give ~41); it's an *amplification* (the effective midflame wind is > 5 mph). That constraint pointed immediately to the update-input-units path.

### Template for Your Experiment

Before running an experiment, fill in this form:

```
Hypothesis: [One sentence]
Prediction: If true, then [observed value] should be [specific number or range]
  (contrasted with: if false, then [expected range])
Test: [exact command / function call / assertion]
```

Example:
```
Hypothesis: Moisture is stored as percent (0.2) but labeled as a fraction (enum 1).
Prediction: If true, then surface spread should be over-high (WASM reads 0.2% = bone-dry).
  Contrasted with: if correct, observed ~19.68 ch/h.
Test: Solve worksheet with moisture 20% (0.2 fraction); assert observed > 50 ch/h.
```

---

## III. Idea Lifecycle — From Spike to Promotion or Retirement

**Principle**: Ideas follow a path: spike → harness → benchmarks → decision (promote via PR + change control or document retirement). Each phase has artifacts and decision gates.

### Phase 1: Spike

**What**: Exploratory code, diagrams, proof-of-concept, or investigation work. Confined to `development/` (long-lived spikes) or `components/` (component-scale spikes). *Not* merged to main.

**Examples**:
- `development/spike-malli-dot/`: Schema visualization spike. Generated architecture/CMS/units/worksheet diagrams (`.png` files). Outcome: provided architecture documentation artifacts; was never a candidate for production code (it's exploratory tools).
- `components/absurder_sql/`: Rust/SQLite DataScript replacement candidate. Branch `rj-ds-rust` (53 commits beyond main, unmerged as of 2026-07-06). Has its own shadow-cljs builds (`:test-kaocha`, `:test`, `:browser`, `:datascript`), Rust crate, and test harness.

**Artifacts**:
- Exploratory code in `development/` or component source in `components/`.
- README or internal comments explaining the spike goal.
- Prototype tests (not yet in the suite's critical path).

**Decision gate**: Is this worth a test harness? If yes, promote to Phase 2; if exploratory-only, archive or retire here.

### Phase 2: Dedicated Test Harness

**What**: A focused test suite that validates the spike's core claim, independent of the larger test suite.

**Examples**:
- `behave.mortality-test` (2 deftests, 15,052+ assertions via CSV rows): validates WASM mortality against golden data.
- `behave.crown-test`, `behave.surface-test`, `behave.contain-test`: module-level harnesses, each tied to a CSV golden set (`crown.csv`, `surface.csv`, `contain.csv`).
- `components/absurder_sql/bin/kaocha`: headless test runner (Chrome via CDP, funnel port 44220, debug port 9222).

**Artifacts**:
- Test file(s) in `projects/behave/test/cljs/behave/*_test.cljs` (or equivalent for the spike's domain).
- Golden data file(s) in `behave-lib/test/csv/*.csv` (or a fixture equivalent).
- Handoff document (e.g., `MORTALITY_TEST_HANDOFF.org`) explaining the test, known issues, and attack plan.

**Decision gate**: Does the harness *fail in ways that are understood and actionable*? (Not "it fails sometimes"; but "it fails on row class X because of reason Y.") If yes, proceed to Phase 3; if the failures are opaque, return to spike.

### Phase 3: Benchmarks and Refinement

**What**: Performance comparisons, scale tests, or repeated runs to validate the spike is robust.

**Examples**:
- absurder_sql: comparative perf vs DataScript on worksheet load/subscribe time.
- Mortality test: run against all 15,052 rows (or the regenerated 3,798 rows after scope reduction) to confirm consistency.

**Artifacts**:
- Benchmark results (commit message, performance tables, or separate `.clj`/`.sh` script).
- Decision log (e.g., "VMS load time improved by 12% with absurder_sql on large worksheets; promotion candidate").

**Decision gate**: Are the results reproducible and aligned with the spike's goal? If yes, proceed to Phase 4a (promotion) or 4b (retirement); if unclear, loop back to Phase 2 (refine test harness).

### Phase 4a: Promotion (via PR + Change Control)

**What**: Merge the spike into main via a PR that follows the change-control protocol (see `behave-change-control` for the 4 non-negotiables and incident rationale).

**Artifacts**:
- PR title: `[BHP1-####] <imperative verb> <what>` (e.g., `[BHP1-1532] Fix graph axes limits`).
- PR body: Purpose, testing steps, linked handoff doc.
- Commit message (title only): `BHP1-#### <plain-language fix>` (e.g., `BHP1-1532 Fix graph axes limits`).
- Updated test suite runs green (no new standing reds).
- Handoff or ticket playbook updated if the spike is ongoing.

**Post-merge artifacts**:
- The spike code is now production (inlined into `bases/` or main `src/` as appropriate).
- The test harness is integrated into the CLJS suite (or project-specific harness, for absurder_sql).
- The handoff doc remains in root for future archaeology.

**Example**: The mortality test investigation (MORTALITY_TEST_HANDOFF.org) led to:
- Regenerated `behave-lib/test/csv/mortality.csv` (from 15,052 → 3,798 rows).
- Commit: `94f6d56b Regenerate mortality.csv from FOFEM reference to match model scope`.
- Suite now green on mortality (0 failures).

### Phase 4b: Retirement (Documented)

**What**: The spike is not promoted; instead, the decision and rationale are recorded so future work does not repeat it.

**Artifacts**:
- Dated `.org` or `.md` file in project root or `.claude/skills/` with: symptom → evidence → why-abandoned → link to spike branch.
- Tag on the final spike commit or branch (e.g., `spike/malli-dot-architecture` with note: "exploratory; generated diagrams; not promoted; see arch/ for outcome").
- Entry in this skill's "Retirement Examples" section (below).

**Why this matters**: The repo contains evidence of *not* retiring formally:
- `features/` (Cucumber E2E) and `bases/datahike_store` exist in the codebase but are effectively abandoned (Cucumber has ~1 of 40 scenarios active; datahike_store is unused). No retirement record exists. This creates **archaeological debt**: future work assumes these are maintained, wastes time investigating them, or fears to delete them. *Always document retirement.*

**Example template**:

```org
* RETIRED (2026-07-15): spike/candidate-thing

Symptom: Feature X was supposed to improve performance by 20%.
Evidence: Benchmarks in branch rj-spike-thing showed 2% improvement (within noise).
  Profiling (tools/bench.clj) showed the bottleneck was elsewhere (see ticket #123).
Why abandoned: Not worth the maintenance cost; the real win is addressing the bottleneck.
Artifact: Branch rj-spike-thing (commit abc123e); diagrams in development/spike-thing/.
See also: [the real fix] (ticket #123).
```

---

## IV. Where Good Ideas Historically Came From

Understanding the *source* of successful ideas in this repo helps you recognize when a new idea is worth exploring.

### Source 1: Test Suite Pain

**Pattern**: A test fails; the investigation reveals a deeper invariant violation or design smell.

**Examples**:
- **mortality-test failures** (7,119 -100 rows): Revealed that the WASM species table is GACC-scoped, not full-species. This reshaped the golden data scope (`mortality.csv` regenerated from FOFEM reference).
- **surface-worksheet and crown-worksheet failures** (82.44 vs 19.68 ch/h, constant fireType): Revealed that the worksheet input-delivery path was skipping the units-normalization step. This surfaced an app-level smell: `apply-single-cpp-fn` silently skips setters when unit is nil (documented in `SOLVER_TEST_HANDOFF.org`, app-side smell #1).
- **contain-testing-simple failure**: The WASM addResource call was missing an `arrivalTimeUnit` argument, causing an argument shift. This revealed the wrapper generation was not validating arity.

**Action**: When a test fails, ask: "What invariant does this violate?" The answer often points to the next thing to build or fix.

### Source 2: Handoff Docs and Ticket Playbooks

**Pattern**: A retiring engineer or PM documents a known issue or architectural decision in a ticket playbook (`.org` file prefixed with ticket number, stored locally and not committed). The playbook becomes the basis for the next investigation.

**Examples**:
- `MORTALITY_TEST_HANDOFF.org` (TL;DR + leading hypotheses + suggested attack plan): Became the roadmap for the mortality investigation. The attack plan (step 1-4) was executed faithfully, the competing hypotheses were tested in order, and the conclusion updated the handoff with findings.
- `SOLVER_TEST_HANDOFF.org`: Documented the worksheet-solve bug with three competing hypotheses (H1, H2, H3 in priority order). H1 was tested first, confirmed, and promoted to a fix.
- `variables_mapping.org` (57K, root): Tags unfinished work (#make-getter, #expose-or-new-solver, etc.). This is a **research frontier** document (see skill #14) that drives prioritization.

**Action**: Read all handoff docs in the project root at session start. They are the "problem backlog" and the "how we think" document simultaneously.

### Source 3: Build/Environment Traps (Negative Evidence)

**Pattern**: A feature or test works locally but fails in CI, or vice versa. Investigation reveals a hidden dependency or assumption.

**Examples**:
- **WASM bootstrap timing** (761b0c22): The test host page instantiates the WASM `Module` *after* the bundle loads. But `enums.cljs` expected `window.runtimeInitialized` to be set during bundle load. Fix: move the Module instantiation before bundle load and remove the gate.
- **Node shim trap** (documented in `behave-build-and-env`): The dev machine's PATH `node` is a Bun shim, not real Node. `bb build-js` needs the real node and `@cljs-oss/module-deps` in the environment.
- **EM_CACHE** (Emscripten caching): WASM builds need `EM_CACHE` set; if not, Emscripten rebuilds from scratch every time.

**Action**: When a test fails inconsistently or in a new environment, assume a **latent dependency**. Document it explicitly in the environment skill (#7).

### Source 4: Code Review and Archaeology

**Pattern**: Merging a PR or reading a commit reveals a mistake or an opportunity.

**Examples**:
- **VMS migration drift** (BHP1-1594, ad62e0e7): Migrations are scattered across the codebase. Moving them all to `projects/behave_cms/resources/migrations/` revealed that the migration-load path was only checking one location. This prevented safe migrations.
- **Generated artifact staleness** (Hatchet wrapper / CMS exports): If `behave/lib/*.cljs` or `cms-exports/*.edn` files are hand-edited, they diverge from the C++ source. A "regenerate, don't patch" rule emerged.

**Action**: Treat archaeology (reading commit history, diffs, and past investigations) as a source of hypotheses for the next bug.

---

## V. When to Stop — Criteria for Abandoning a Direction

**Principle**: Stopping work is a *positive decision*, not a default. Document the stop explicitly with evidence.

### Stopping Criteria

| Criterion | What it means | Example |
|-----------|---------------|---------|
| **Completed**: Test passes, decision made, merged | The spike has promoted to production or been formally retired. | Mortality test: 3,798/3,798 rows pass; regenerated golden data via PR; closed. |
| **Blocked**: Evidence is unavailable or conflicting | The investigation cannot proceed without external data (e.g., FOFEM reference data, upstream Behave6 source) or without resolving a dependency. | Crown damage (CRCABE): path is inert; FOFEM reference has zero CRCABE rows; can't validate without model fix + new golden data. Documented as "follow-up" in MORTALITY_TEST_HANDOFF.org. |
| **Deferred**: The issue is real, but the cost is high relative to priority | Fix exists and is understood, but promotion is blocked by change-control, team capacity, or competing work. | Restoring 339 dropped variant species in mortality.csv: requires behave-mirror + FOFEM data-owner effort. Documented in handoff; separate tracking (ticket). |
| **Rejected**: The hypothesis is disproven or the cost exceeds the benefit | Evidence shows the idea doesn't work, or the return-on-investment is negative. | H1 (region-gated variants): Direct test showed base codes resolve under hardcoded SouthernArea even when region check is false. Hypothesis disproven; no further test on region theory. |
| **Known standing red**: The failure is understood and accepted | The issue is baked into the upstream reference or a consequence of a deliberate scope decision. | POTR12 (quaking aspen) discrepancy: 27 rows in upstream C++ reference where Behave differs from FOFEM by >=4. Baked into golden data; excluded from mortality.csv; documented as "known-red"; not a test failure. |

### The Retirement Artifact

When stopping (criteria: completed, blocked, deferred, rejected, or known-red), create a dated entry in the project root `.org` file or in this skill with:

1. **Symptom**: What was failing or wrong?
2. **Evidence**: What experiments were run? What did they show?
3. **Decision**: What is the stop reason (one of the criteria above)?
4. **Rationale**: Why stop here rather than continue?
5. **Next steps** (if any): What would it take to unblock / continue?
6. **Related**: Links to branches, tickets, or other investigations.

**Example from this repo**:

```org
* BLOCKED (2026-07-02): Mortality CRCABE rows (crown_damage equation inert)

Symptom: 3,832 CRCABE (crown damage) rows in mortality.csv compute constant ~1.5-3%
  regardless of input (crownDamage %, CambiumKillRating, beetle flags). Test fails.
Evidence: Direct WASM testing (setters with/without values, unit on/off) shows only DBH
  moves output; other inputs are ignored. Upstream C++ reference (resultsProbMort.csv)
  has zero CRCABE rows — equation never validated. Inert path is a model bug, not test
  artifact.
Decision: BLOCKED — no upstream golden data to validate against.
Rationale: The crown_damage path is not tested upstream (FOFEM_input.tre is CRNSCH+BOLCHR only).
  Restoring CRCABE rows requires: (1) behave-mirror fix (model), (2) new FOFEM goldens.
  Both are out-of-scope here.
Next steps:
  - File behave-mirror issue: "crown_damage path inert" (assign to fire-science owner).
  - Request FOFEM data: CRCABE golden values for the affected species+param combos.
  - Once data arrives: regenerate CRCABE rows in mortality.csv; re-enable test.
Related: MORTALITY_TEST_HANDOFF.org (root cause B); ticket BHP1-XXXX (fire-science follow-up).
```

### Anti-Pattern: Silent Abandonment

**What NOT to do**: Delete code, skip tests, or mark them `^:skip` without documenting why.

**Why**: Future work assumes these are maintained, wastes time investigating them, or re-implements them unknowingly. Example: Cucumber E2E tests (~1 of 40 active) and `datahike_store` component are effectively abandoned but have no retirement record. This creates **archaeological debt**.

**Action**: If you are tempted to skip a test or remove code, write the retirement artifact *first*. Then the skip/deletion is justified.

---

## VI. Adversarial Refutation — The Discipline

The evidence bar is only as strong as your *refutation* of alternatives. This section formalizes the practice.

### Template for Systematic Hypothesis Testing

Before testing, list all plausible hypotheses. For each, write:

1. **Hypothesis statement** (one sentence)
2. **Prediction** (if true, then X should be Y)
3. **Test** (exact command / assertion)
4. **Falsification criterion** (if not-Y, hypothesis is false)
5. **Priority** (test this first if multiple hypotheses)

### Example: Worksheet Input Units (SOLVER_TEST_HANDOFF.org)

| H | Hypothesis | Prediction | Test | Falsification | Priority |
|---|-----------|-----------|------|---------------|----------|
| 1 | `solve-ws-outputs` omits units loop | Skipping loop → unnormalized values → spread moves away from 19.68 | Add loop; re-run; compare | If spread ≈ 19.68 after fix, H1 accepted | 1st |
| 2 | Value+unit → WASM setter broken | Setter receives wrong unit enum or unconverted value → divergence in logs | Instrument `apply-single-cpp-fn`; log value+unit; compare to direct call | If logs match direct call, H2 false | 2nd |
| 3 | Wrong group-var chosen | Wind setter maps to wrong native-unit gv → amplification | Check gv native units (mi/h, deg, etc.) match inputs | If units match, H3 false | 3rd |

**Execution**: Test H1 first. If it explains the gap, stop (H2 and H3 are not needed). If H1 fails its falsification criterion, test H2. And so on.

### Outcome Recording

After testing, update the priority list with results:

```
H1: ACCEPTED (added units loop, spread now 19.67758433 vs golden 19.677584)
H2: NOT TESTED (H1 fully explained the gap)
H3: NOT TESTED (H1 fully explained the gap)
```

This prevents **hypothesis hunting** (testing hypotheses until one sticks). The pre-written list forces you to decide *before* you have skin in the outcome.

---

## VII. When to Use This Skill

Use this skill when:

- You are investigating a test failure or unexpected observation and need to structure the investigation (evidence bar, competing hypotheses, adversarial refutation).
- You are designing a spike or proof-of-concept and need to plan the lifecycle (harness → benchmarks → promotion).
- You are reviewing a PR or merged code and want to verify it meets the research bar (hypothesis prediction, adversarial testing).
- You are reading a handoff doc or ticket playbook and want to understand the methodology behind it.
- You need to decide when to stop work (completed, blocked, deferred, rejected, known-red) and document the stopping criterion.
- You are building a test suite and want to structure golden data and test tiers (see `behave-validation-and-qa` for the inventory and `behave-proof-and-analysis-toolkit` for recipes).

---

## VIII. When NOT to Use This Skill

**Do NOT use this skill for**:

- **Fast debugging on a known-familiar issue**: If you already know the cause (e.g., "I broke line 42 in solver_test.cljs"), just fix it. No handoff or hypothesis needed.
  → *Instead*: Use `behave-debugging-playbook` for triage on unfamiliar symptoms.

- **Routine code changes with no research component**: Typo fixes, style updates, or refactoring stable code.
  → *Instead*: Use `behave-change-control` for the PR process.

- **Domain theory or fire-science questions**: "What is crown scorch?" or "How does the Rothermel model work?"
  → *Instead*: Use `fire-behavior-reference`.

- **Decisions about promoting changes to production**: The methodology assumes you *already have* a passing test and a candidate fix. The promotion decision (PR, branch, commit message) is separate.
  → *Instead*: Use `behave-change-control` for the gates and `behave-docs-and-writing` for the commit voice.

- **Building the test harness itself** (fixtures, CSV golden data, assertion structure).
  → *Instead*: Use `behave-proof-and-analysis-toolkit` for recipes and `behave-validation-and-qa` for the test inventory.

- **VMS or build pipeline issues**: "How do I regenerate the VMS after a C++ change?" or "What's the EM_CACHE variable?"
  → *Instead*: Use `behave-vms-variable-pipeline` (#6) or `behave-build-and-env` (#7).

---

## IX. Cross-References

This skill is one of a 15-skill library (as of 2026-07-06):

| # | Skill | When to use |
|---|-------|-------------|
| 1 | **behave-change-control** | You have a fix and need to know the promotion gates (PR, commit, branch conventions) and non-negotiables |
| 2 | **behave-debugging-playbook** | You see a symptom and need to triage (failing test, wrong output, startup crash) |
| 3 | **behave-failure-archaeology** | You want to read the chronicle of past investigations and avoid re-fighting old battles |
| 4 | **behave-architecture-contract** | You're planning a change and need to know load-bearing decisions and invariants |
| 5 | **fire-behavior-reference** | You need to understand the fire science (Rothermel, modules, units, GACC codes) *as reflected in this code* |
| 6 | **behave-vms-variable-pipeline** | You're adding a new variable and need the end-to-end data-flow (C++ → WASM → CLJS → VMS → fixtures) |
| 7 | **behave-build-and-env** | You're setting up a dev environment or fixing a build issue (prerequisites, traps, EM_CACHE, node shim) |
| 8 | **behave-run-and-operate** | You need to run behave (figwheel, server, desktop, CMS) or deploy it |
| 9 | **behave-diagnostics-and-tooling** | You need to measure/profile (solver logs, test console, WASM debugger, perf) |
| 10 | **behave-validation-and-qa** | You need the test inventory, test tiers with exact commands, known standing reds, how to add tests |
| 11 | **behave-docs-and-writing** | You're writing docs (README.org, architecture, ticket playbooks, help content) |
| 12 | **behave-absurder-sql-campaign** | You're landing the Rust/SQLite DataScript replacement from rj-ds-rust branch |
| 13 | **behave-proof-and-analysis-toolkit** | You're designing an experiment: recipes for golden testing, bisection, migration dry-runs, storage benchmarks |
| 14 | **behave-research-frontier** | You want to understand open problems (absurder_sql as standalone engine, solver perf/scale, front-end perf) |
| 15 | **behave-research-methodology** | *← You are here* — turning hunches into accepted results (evidence bars, hypotheses, spike lifecycle, retirement) |

**Closely related**:
- **behave-proof-and-analysis-toolkit** (#13): Provides *recipes* (differential testing, layer bisection, migration dry-run, storage benchmarking) to collect evidence for the methodology here.
- **behave-change-control** (#1): Provides *gates* and *non-negotiables* for promoting a fix once the research is done.
- **behave-failure-archaeology** (#3): Maintains the *chronicle* of past investigations; cite it when deciding to retire a spike (e.g., "See archaeology entry for CRCABE inertness").
- **behave-debugging-playbook** (#2): Provides *triage* when you don't yet know the root cause; use this skill when investigation is ready to begin.
- **behave-validation-and-qa** (#10): Defines test *inventory* and *tiers*; cross-reference for the "evidence tier" discussion (unit, integration, golden, etc.)

---

## X. Provenance and Maintenance

All facts below are verified against the repo as of **2026-07-06**. IMPORTANT: Some investigations described in this skill (mortality CSV regeneration, solver test fixes) are on the unmerged branch `rj-fix-figwheel-tests` and are not yet on main. Those facts are marked `UNVERIFIED (branch state)` below.

| Fact | Verification | Status |
|------|--------------|--------|
| Handoff docs in root: FIX_TEST_PLAN.org, MORTALITY_TEST_HANDOFF.org, SOLVER_TEST_HANDOFF.org | `ls /Users/rsheperd/code/sig/behave-app/*.org` (verify paths exist) | VERIFIED (main) |
| WASM species table size: 197 records / 190 distinct base codes | MORTALITY_TEST_HANDOFF.org line 171: `getNumberOfRecordsInSpeciesTable= 197` (reported from in-page WASM inspection) | VERIFIED (WASM state, not code) |
| Mortality test: regeneration completed, 3,798 rows; 27 known-red | `git show 94f6d56b` (commit title + regeneration procedure at lines 285-289) | UNVERIFIED (on branch rj-fix-figwheel-tests; main still has 15,052 rows) |
| Solver test green: 35 deftests / 4,161 assertions / 0 failures / 0 errors | FIX_TEST_PLAN.org line 38 and SOLVER_TEST_HANDOFF.org line 6 ("RESOLVED") | UNVERIFIED (branch state; see git log rj-fix-figwheel-tests) |
| Root cause A (species coverage): CSV regenerated from FOFEM reference | MORTALITY_TEST_HANDOFF.org lines 285-289 (regeneration procedure); commit 94f6d56b | UNVERIFIED (branch state) |
| Root cause B (CRCABE inert): FOFEM reference has zero CRCABE rows | MORTALITY_TEST_HANDOFF.org line 233 | VERIFIED (historical reference data, immutable) |
| absurder_sql spike branch: rj-ds-rust, 30+ commits unmerged | `git log --oneline main..origin/rj-ds-rust 2>/dev/null \| wc -l` | VERIFIED (branch exists, count ~30+) |
| Test harnesses: mortality, surface, crown, contain in projects/behave/test/cljs/behave/ | `ls projects/behave/test/cljs/behave/*_test.cljs` | VERIFIED (main) |
| Golden CSV files: mortality.csv, surface.csv, crown.csv, contain.csv in behave-lib/test/csv/ | `ls behave-lib/test/csv/*.csv` | VERIFIED (main) |
| WASM bootstrap fix (761b0c22): Module instantiation before bundle load | `git show 761b0c22 --stat` | VERIFIED (branch state, on rj-fix-figwheel-tests only) |
| Units-uuid arity drop fix (4b7b2dbb): mortality-worksheet on real ws-input fixture | `git show 4b7b2dbb --stat` (in git log, on rj-fix-figwheel-tests branch) | VERIFIED (branch state, on rj-fix-figwheel-tests only) |

### Re-Verification Commands

Run these quarterly or after major changes to confirm facts remain true:

```bash
# Check mortality test row counts (on main)
wc -l behave-lib/test/csv/mortality.csv

# Verify handoff docs exist (on main)
ls *.org | grep -E "(HANDOFF|TEST_PLAN)"

# Count absurder_sql commits beyond main
git log --oneline main..rj-ds-rust 2>/dev/null | wc -l

# Verify test file locations (on main)
find projects/behave/test/cljs/behave -name "*_test.cljs" | wc -l

# Verify golden CSV files (on main)
ls behave-lib/test/csv/*.csv | wc -l

# Check which investigation branches exist
git branch -r | grep -E "(rj-fix|rj-ds-rust|rj-spike)"

# Verify key commits referenced in handoff docs
git log --oneline --all | grep -E "(94f6d56b|4b7b2dbb|761b0c22|63c6cfe6)"
```

---

## XI. Examples: Full Investigations from This Repo

### Investigation A: Mortality Species Coverage (2026-07-02)

**Status**: DOCUMENTED (investigation complete; promotion on branch `rj-fix-figwheel-tests`, not yet on main)

**Handoff**: MORTALITY_TEST_HANDOFF.org

**Hypotheses**: H1 (region-gated), H2 (table subset), H3 (invalid codes)

**Refutation**: H1 disproven by direct WASM test; H2 + H3 confirmed via table inspection (197 record species table confirmed via in-page WASM inspection)

**Resolution**: Regenerated `mortality.csv` from FOFEM reference (15,052 → 3,798 rows), commit 94f6d56b

**Branch state**: Test passes on `rj-fix-figwheel-tests` (investigator reports 3,798/3,798 rows pass); awaiting PR to main

**Key insight**: One unified mechanism (WASM table is GACC-scoped, not full-species) explains all 7,119 -100 rows; separate independent root cause (CRCABE path inert) explains 3,832 wrong-value rows.

### Investigation B: Worksheet Input Units (2026-07-02)

**Status**: DOCUMENTED (investigation complete; promotion on branch `rj-fix-figwheel-tests`, not yet on main)

**Handoff**: SOLVER_TEST_HANDOFF.org

**Hypotheses**: H1 (omitted units loop, priority 1), H2 (setter transform broken, priority 2), H3 (wrong gv, priority 3)

**Refutation**: H1 accepted (loop added, spread moved to golden value 19.67758433); H2 and H3 not tested (H1 sufficient)

**Resolution**: Added `update-input-units` dispatch to `solve-ws-outputs`, commit 4b7b2dbb; rewrote mortality-worksheet test onto real worksheet fixture (commit 63c6cfe6)

**Branch state**: Suite reports 35 deftests / 4,161 assertions / 0 failures / 0 errors on `rj-fix-figwheel-tests`; main has 59 deftests in `projects/behave/test/cljs/` (measured via grep, 2026-07-06; count varies between branches)

**Key insight**: Adversarial refutation prevented hypothesis hunting; H1 was tested first (by priority), confirmed, and closed the investigation without needing H2 or H3.

### Investigation C: Crown Damage Equation (2026-07-02)

**Status**: BLOCKED (insufficient golden data)

**Handoff**: MORTALITY_TEST_HANDOFF.org lines 193-204 (root cause B)

**Root cause**: CRCABE equation path is inert in WASM; only DBH affects output (setters for crownDamage, CambiumKillRating, beetle flags are ignored)

**Evidence**: Direct WASM testing (value+unit parameter sweeps) + upstream reference (behave-mirror/src/testMortality/FOFEM_input.tre) has zero CRCABE rows

**Stop reason**: BLOCKED — no upstream golden data to validate against; model fix required upstream

**Follow-up**: Requires behave-mirror model fix (C++ crown_damage path) + FOFEM data-owner to provide reference CRCABE golden values (separate ticket)

**Lesson**: Stopping is a positive decision. This investigation shows correct handling of "blocked" status: explicitly documented with evidence and next steps, not left as a standing red

---

## XII. Key Takeaways

1. **Evidence bar**: One mechanism must explain all observations (including negatives). If you have three problems, force unification or accept multiple independent root causes with disjoint evidence.

2. **Prediction before experiment**: Write down exact predictions and falsification criteria *before* running the test. This prevents anchoring bias.

3. **Spike lifecycle**: spike → harness → benchmarks → (promote via PR or document retirement). Each phase has artifacts and gates.

4. **Sources of ideas**: test pain, handoff docs, build traps, archaeology.

5. **Stopping criteria**: completed, blocked, deferred, rejected, known-red. Always document the stop.

6. **Adversarial refutation**: List all hypotheses, prioritize by cost-to-test, and execute in order. Stop when one is accepted (the others are not tested).

7. **Retirement**: If you stop or abandon a spike, create a dated entry with symptom → evidence → decision → rationale. This prevents archaeological debt.

---

**Last updated**: 2026-07-06  
**Skill author**: Claude (via orchestrated session)  
**Based on**: MORTALITY_TEST_HANDOFF.org, SOLVER_TEST_HANDOFF.org, FIX_TEST_PLAN.org, git history
