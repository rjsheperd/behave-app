---
name: behave-docs-and-writing
description: Maintain architecture docs (README.org, arch/*.org), org-mode house style, ticket-playbook local convention, commit/PR voice, and help-content authoring pipeline (MadCap Flare + DITA XHTML → Datomic migrations).
---

# Behave7 Docs and Writing: Complete Runbook

Date stamp: **2026-07-06**

## One-paragraph context

This skill is your guide to authoring and maintaining Behave7's documentation of record: the root `README.org` (Rothermel fire-behavior simulator SPA + desktop app), four architecture docs under `arch/*.org` (Polylith, Clojure/Script, WebAssembly, schemas), a ticket-playbook convention where developers keep local-only `<TICKET-KEY>.org` files (not committed), consistent commit/PR voice across git history, and the help-content authoring pipeline that flows from the MadCap Flare CMS (`bases/behave-docs` submodule) → Flare's XHTML output → DITA/XHTML cleanup via Clojure component → Datomic migrations → layout.msgpack. This skill owns one home for every doc-lifecycle fact; sibling skills link here.

---

## Docs of record: README.org and arch/ directory

### README.org structure and stale parts (as of 2026-07-06)

**Location:** `/Users/rsheperd/code/sig/behave-app/README.org`

**Status:** Mostly current; one placeholder + one missing content confirmation.

**Structure and sections:**

| Section | Status | Notes |
|---------|--------|-------|
| Title + logo | Current | Behave7 branding + supported-by logos (USDA/USFS/SIG) |
| Table of Contents | Current | Links to Latest Release, Installation, Overview, Features, License, Architecture, Contact |
| Latest Release | Current | Link to GitHub releases page (auto-serves latest .jar) |
| Installation | Current | 3 steps: (1) JRE 21 (Eclipse Temurin or Corretto), (2) Chrome browser, (3) Download .JAR from releases, double-click to launch |
| Overview | Current | One-paragraph domain description (fire behavior prediction, rate of spread, spotting, scorch, mortality, etc.) |
| Features | Current | 4 bullet points (up-to-date fire equations, fresh UI, in-app docs, PDF export) |
| License | **STALE** | Placeholder text `<INSERT LICENSE>` (line 68); actual license is `LICENSE.txt` at repo root (public domain RMRS + BSD 2-Clause for external contributions) — see **FIX** below |
| Architecture | Current | Link to `arch/00_index.org` |
| Contact | Current | IIA HelpDesk link + email contacts (Faith Ann Heinsch, LaWen Hollingsworth) |

**Known issue:** The License section (line 68) contains the literal placeholder `<INSERT LICENSE>`. The correct license exists at `/Users/rsheperd/code/sig/behave-app/LICENSE.txt` and should be cited or inlined. This is a one-time fix (update README.org line 68 to point to or inline LICENSE.txt).

### Architecture docs: arch/*.org directory

All architecture docs use org-mode and are cross-linked via `[[file:...]]` syntax. **The project prefers README.org over README.md** — this is a house rule.

**Doc inventory (as of 2026-07-06):**

| File | Purpose | Status | Key topics |
|------|---------|--------|------------|
| `arch/00_index.org` | **Overview and architecture principles** | Current | Polylith pattern (components, bases, projects); ClojureScript + re-frame + re-posh + DataScript; WebAssembly (WASM) via Emscripten; VMS schema diagram; worksheet schema diagram; combined VMS+worksheet schema diagram |
| `arch/01_getting_started.org` | **Dev environment setup from scratch** | Likely current but **VERIFY** | Prerequisites (JDK 17, Babashka, Clojure, Postgres 12+); Emacs/CIDER setup (figwheel-main, port 8080); Datomic setup (download, Postgres init, transactor, console); UberJAR build steps (config.edn, bb build-js, bb uber) |
| `arch/02_building_wasm.org` | **C++ to WASM compilation and integration** | Not read; likely stale | Emscripten, behave-lib submodule, WASM generation, CLJS bindings, Hatchet tool |
| `arch/03_vms_guide.org` | **Variable Management System (VMS) deep dive** | Not read; likely describes schema, migrations, layout.msgpack, data flow | CMS at port 8001, Datomic at port 8000, VMS export format, variable linking |

**Cross-links between docs:** Each doc has a "Prev/Next" footer; docs are numbered 00–03. Linked via `[[file:]]` org-mode syntax (e.g., `[[file:01_getting_started.org][1. Getting Started]]`).

**Known staleness patterns (from brief; verify if working on relevant area):**

- `arch/01_getting_started.org` references `~development/user.cljs~` and figwheel port 8080; **VERIFY** that figwheel alias and port match root `deps.edn` and `projects/behave` config.edn (brief notes were found at port 8081).
- `arch/02_building_wasm.org` and `arch/03_vms_guide.org` may predate the `behave_components` submodule→inline migration (commit 29433863); if you find examples or paths referencing GitLab URLs, they are stale.
- No arch doc currently exists for **testing harness** (browser CLJS suite at localhost:8081/api/test, headless runner via test-headless.cljs.edn, kaocha+funnel+Chrome-CDP for absurder_sql) — that lives in skill `behave-validation-and-qa`.
- No single arch doc exists for **VMS pipeline** (C++ → WASM → Hatchet → cms-exports → CMS → layout.msgpack → fixtures); that is the subject of skill `behave-vms-variable-pipeline`.

**When to update arch/ docs:** Whenever a change to Polylith structure, build/env setup, or schema materialization lands. Before merging a PR that changes prerequisites, ports, or architectural patterns.

---

## Org-mode house style and conventions

This repo uses **org-mode (.org files) for all internal documentation**, not Markdown. This is a hard rule. Skills are Markdown (SKILL.md), but everything under `arch/`, `development/`, and `projects/behave/*.org` playbooks is org-mode.

### Org-mode file template and structure

**Minimum structure for a new org doc:**

```org
#+TITLE: <Title Here>

** Table of Contents
- [[#section-1][Section 1]]
- [[#section-2][Section 2]]

** Section 1
:PROPERTIES:
:CUSTOM_ID: section-1
:END:

Content here.

** Section 2
:PROPERTIES:
:CUSTOM_ID: section-2
:END:

More content.
```

**Key conventions:**

| Element | Rule | Example |
|---------|------|---------|
| Title | Use `#+TITLE:` (not `#` Markdown) | `#+TITLE: BHP1-1354 Show Direction Mode in Surface & Crown` |
| Headings | Use `**` (second level) for major sections | `** Prerequisites`, `** Architecture` |
| Subheadings | Use `***` (third level) for subsections | `*** Emacs/CIDER Setup` |
| Section anchors | Use `:PROPERTIES:` + `:CUSTOM_ID:` to enable internal links | For linking within or from other docs |
| Internal links | Use `[[file:...][Link Text]]` for cross-file, `[[#anchor][Link Text]]` for within-file | `[[file:01_getting_started.org][1. Getting Started]]` or `[[#section-1][Section 1]]` |
| Code blocks | Use `#+BEGIN_SRC <lang>` / `#+END_SRC` (not triple-backticks) | `#+BEGIN_SRC clojure` for Clojure code |
| Lists | Use `+` or `-` for unordered, `1.` for ordered | Org-mode renders consistently |
| Comments | Use `# Comment` at line start (not `#+COMMENT` which hides a section) | For in-file notes |
| HTML export options | Use `#+BEGIN_HTML` / `#+END_HTML` for raw HTML (e.g., GitHub badges, embedded iframes) | GitHub workflow badges, YouTube embeds (see README.org) |
| Tables | Use `|` pipes with `|-` for headers (Org-mode auto-formats) | Org-mode handles alignment |
| Bold/Italic | Use `*bold*`, `/italic/`, `~code~` (not Markdown `**`, `_`, or `` ` ``) | Follow org-mode markup |

### Line length and formatting

- **Max 80 characters per line** (100–120 acceptable for code blocks or URLs that cannot break).
- **Unix line endings** (LF, not CRLF).
- **Trailing newline** at EOF.
- **No trailing whitespace** on any line.
- **2-space indent** inside code blocks or nested lists.

### Comment conventions in org-mode files

```org
;; This is a docstring or explanation comment in a code block
#_ This is a commented-out form (Clojure only)
```

For **comments in prose sections**, use inline explanation or a separate paragraph; org-mode does not have a built-in "comment line" like `#+COMMENT` (that hides the entire section). Use `# Note:` at line start if you need an inline marker.

---

## Ticket-playbook convention: local-only `<TICKET-KEY>.org` files

### Purpose and lifecycle

Every developer keeps a **local-only playbook file** per Jira ticket under `projects/behave/<TICKET-KEY>.org` (e.g., `BHP1-1354.org`). This file is **NOT committed** (it is `.gitignore`'d or untracked) and serves as the developer's working research → design → development → review log for that ticket. It is a bridge between Jira issue description and PR.

### Expected structure (based on observed tickets: BHP1-1354, BHP1-1532, BHP1-1544, BHP1-1611)

```org
#+TITLE: BHP1-#### <Ticket Title>

* Research
;; Notes from understanding the ticket, existing code, related issues, etc.
;; Links to related issues, PRs, or code.

* Breakdown
;; Design notes: what needs to change, why, side effects, affected modules.
;; Pseudocode or structure outlines.
;; Gotchas discovered during research.

* Development
;; Actual changes made (often a summary, not full diffs).
;; Testing steps (in prose or pseudo-code).
;; Blockers or pivots.

* PR Description

** Title
[BHP1-####] <Imperative verb and outcome>

** Purpose
- Bullet point: what problem does this solve?
- Bullet point: why now?

** Related Issues
Closes BHP1-####

** Submission Checklist
- [ ] Included Jira issue in the PR title (e.g. `BHP1-### <title>`)
- [ ] Code passes linter rules (`clj-kondo --lint ...`)
- [ ] Feature(s) work when compiled (`clojure -M:compile-cljs` or similar)

** Testing
1. Step 1 to reproduce the fix.
2. Step 2 to confirm the expected outcome.
3. Step 3 if needed.

** Screenshots
(If applicable, placeholder or actual images)
```

### Key observations from observed tickets

1. **Title:** Always `BHP1-####` prefix + space + human-readable ticket title.
2. **PR Description section:** Copy-paste directly into GitHub PR template. Follow the structure exactly.
3. **Testing:** Numbered steps (1, 2, 3…) that a reviewer can execute end-to-end; include specific UI paths, config values, or file names where applicable.
4. **Submission Checklist:** Standard three checks (PR title format, linting, feature works); check off each as true before pushing.
5. **Screenshots:** Often left as `TODO: <description>` if the ticket is in-progress; filled in before PR review.
6. **Not committed:** The entire file stays local; the PR body (extracted from `* PR Description`) is what lands in GitHub.

### When to create or update

- **Create** at ticket start (research phase).
- **Update** as research → design → development → ready-for-review progresses.
- **Final version** matches the PR body exactly.
- **After merge:** Keep the file locally for reference; do not commit it.

### Verification (local file, not in repo history)

Since these files are not committed, there is no command to verify their existence in git history. Check them in the working tree:

```bash
ls -la projects/behave/*.org | grep BHP1
```

Expected: Multiple untracked or `.gitignore`'d `.org` files named `BHP1-*.org`.

---

## Commit and PR voice: observed patterns and hard rules

### Commit message format (from git log)

**Hard rule:** Commit messages are **plain-language titles only**, no body paragraph. Format:

```
BHP1-#### <Plain-English Summary of Change>
```

(Note: Some historical commits may include brackets `[BHP1-####]`, but this is not the current standard; use the format shown above.)

**Examples from history** (verified from `git log --oneline`):

- `BHP1-1532 Fix graph axes limits` (not "fix: graph axes" or "Graph axes: limits")
- `BHP1-1544 Preserve newlines in saved notes`
- `BHP1-1611 Shade result tables per direction`
- `BHP1-1571 Default single ranged input table to outputs on rows`
- `BHP1-1603 align result button left padding with add notes button`
- `[BHP1-1545] Fix Guided Workflow Module Order` (CapWords for proper nouns like "Guided Workflow"; some commits use brackets)

**Style checklist for commits:**

- Imperative mood (e.g., "Fix", "Add", "Preserve", "Shade", not "Fixed", "Added", "Fixes").
- Plain English, no Jira-ese or technical jargon in the subject.
- No body; title only.
- Lead with ticket number (BHP1-####) and space.
- Capitalize correctly (imperative verb + title case for proper nouns, lowercase for variables/functions).
- Under 80 characters if possible (100 acceptable).

### PR title and body format (from merged PRs)

**PR Title format (verified from GitHub merge history):**

```
[BHP1-####] <Imperative verb + outcome>
```

**Examples from history** (note: both formats with and without brackets appear in merged PRs):

- `[BHP1-1532] Fix graph axes limits` (with brackets)
- `BHP1-1544 Preserve newlines in saved notes` (without brackets)
- `[BHP1-1611] Shade result tables per direction` (with brackets)
- `BHP1-1545 sort worksheet header modules by results-order` (without brackets)

**PR Body format (extracted from observed `.org` playbooks):**

```markdown
## Purpose
- Bullet point explaining the problem or goal.
- Bullet point explaining why it matters.

## Testing
1. Step 1 (e.g., "Open a worksheet with Surface & Crown modules selected").
2. Step 2 (e.g., "Confirm 'Direction Mode' group now appears").
3. Step 3 (e.g., "Confirm 'Surface Fire' group does NOT appear").

## Submission Checklist
- [ ] Included Jira issue in PR title (e.g., `BHP1-### <title>`)
- [ ] Code passes linter rules (`clj-kondo --lint ...`)
- [ ] Feature(s) work when compiled (`clojure -M:compile-cljs` or similar)

(Optional: Screenshots, additional testing notes)
```

**Style checklist for PRs:**

- Title opens with `[BHP1-####]` or `BHP1-####` (both formats exist; preferred modern style uses brackets).
- Body uses numbered Testing steps (not bullet points).
- No AI-generated footer (e.g., "Co-Authored-By: Claude").
- Terse and direct; avoid over-explanation.
- When referencing related issues, use `Closes BHP1-####` or `Fixes BHP1-####` to auto-link.
- Screenshots or GIFs are encouraged; marked `TODO: <description>` if pending.

### When NOT to follow this voice

- **Merge commits** (auto-generated by GitHub): Follow GitHub's default format; manual editing not required.
- **Revert commits**: Prefix with `Revert "BHP1-#### <original msg>"` per Git convention.
- **Non-ticket commits**: Rare; use plain English imperative without ticket number if no Jira issue exists.

---

## Help-content authoring pipeline: MadCap Flare → DITA XHTML → Datomic

This pipeline is how in-app help docs (accessible inside the Behave7 UI) are authored, exported, cleaned, and synced with the CMS.

### Pipeline overview (end-to-end)

```
┌─────────────────────────────────────────────────────────────┐
│ 1. MadCap Flare (bases/behave-docs submodule)               │
│    - Proprietary WYSIWYG help authoring tool                │
│    - Stores project in Behave_Madcap_Project/               │
│    - Manual export to XHTML in XHTML_Output/                │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 2. Flare XHTML Output (bases/behave-docs/XHTML_Output/)     │
│    - HTML topics in Content/                                │
│    - Image resources in Resources/Images/                   │
│    - Variable snippets in Resources/Snippets/Variables/     │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 3. DITA XHTML Cleanup (components/dita + development/)      │
│    - development/help_import.clj orchestrates               │
│    - dita.xhtml-cleaner: removes style, class, id attrs    │
│    - Converts images: ../Resources/Images/*.png → /help/    │
│    - Convert images: .png/.jpg → .webp via shell convert    │
│    - Extracts :help-page/key from metadata                  │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 4. Datomic Migration & Transact (projects/behave_cms)       │
│    - Creates :help-page entities with keys and content      │
│    - Ties :help-page/key to UI lookups (dotted notation)   │
│    - Migration entries: help-import-add-<timestamp>         │
│    - Migration entries: help-import-remove-<timestamp>      │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ 5. Client Load (projects/behave DataScript startup)         │
│    - :help-page/key → lookup in DataScript                  │
│    - UI references :help-page/key via dotted convention     │
│    - In-app help icon opens help sidebar                    │
└─────────────────────────────────────────────────────────────┘
```

### Step 1: MadCap Flare authoring

**Tool:** MadCap Flare (proprietary; not managed in code, stored in submodule).

**File structure in repo:**

```
bases/behave-docs/
├── Behave_Madcap_Project/     (MadCap project; not detailed here)
├── XHTML_Output/              (Flare export target)
│   └── BehaveAppHelp/
│       ├── Content/           (HTML topics)
│       ├── Resources/
│       │   ├── Images/        (PNG, JPG, etc.)
│       │   └── Snippets/
│       │       └── Variables/ (Variable-help HTML snippets)
│       └── ...                (Flare metadata files)
├── README.md                  (Submodule README, not detailed)
└── deps.edn                   (Clojure deps for CMS integration)
```

**When Flare is updated:** A developer manually exports from Flare to `XHTML_Output/`. This is a manual step; no automation detected in the current build.

### Step 2: Flare XHTML output structure

**Topics (in-app help pages):**

- Location: `bases/behave-docs/XHTML_Output/BehaveAppHelp/Content/*.htm`
- Each `.htm` file is a Flare-generated topic.
- Contains a metadata element with class `metadata` (parsed to extract `:help-page/key`).
- Contains the help content as nested HTML divs, lists, links, images.

**Variables (context-sensitive help for input fields):**

- Location: `bases/behave-docs/XHTML_Output/BehaveAppHelp/Resources/Snippets/Variables/*.htm`
- Each `.htm` file documents one variable's help.
- Extracted separately and merged into the same `:help-page` entity pool.

**Images:**

- Location: `bases/behave-docs/XHTML_Output/BehaveAppHelp/Resources/Images/`
- Formats: PNG, JPG, JPEG (will be converted to WebP).
- Referenced in HTML as `../Resources/Images/example.png`.

### Step 3: DITA XHTML cleanup (Clojure component)

**Location:** `components/dita/` (interface in `src/dita/interface.clj`, logic in `src/dita/xhtml_cleaner.clj`).

**Main functions:**

| Function | Input | Output | Purpose |
|----------|-------|--------|---------|
| `clean-topic` (exported) | File path to a topic `.htm` | `{:key <help-key> :content <html>}` | Remove style/class/id, extract metadata key, return clean content |
| `clean-variables` (exported) | File path to variable snippet `.htm` | `[{:key ... :content ...} ...]` | Apply `clean-topic` logic per variable |
| `remove-stylesheet-and-class` (internal) | Hiccup node | Hiccup node or nil | Post-walk to remove unwanted tags and attributes |

**Cleanup transformations applied:**

| Transformation | Input | Output | Rationale |
|---|---|---|---|
| Remove `<link>`, `<meta>` tags | `<link>`, `<meta>` | (removed) | Flare metadata; not needed in app |
| Remove `<div id="...">` | Any div with `id` attribute | (removed) | Flare internal anchors |
| Remove prolog divs | `<div class="prolog">` | (removed) | Flare metadata section |
| Remove whitespace-only text nodes | `"\n  "` | (removed) | Flare formatting cruft |
| Remove empty `<img>` tags | `<img src="">` | (removed) | Broken image references |
| Remap `<tdiv>` → `<div>` | `<tdiv>` | `<div>` | Non-standard Flare tag |
| Remove inline `style` attributes | `style="..."` | (removed) | App CSS handles styling |
| Ensure `<a>` links open new tab | `<a href="...">` | `<a target="_blank">` | UX: keep app window open |
| Convert image URLs | `../Resources/Images/foo.png` | `/help/images/foo.webp` | App-relative path; WebP format |
| Convert heading levels | `<h4>` | `<h5>` (topics) or `<h6>` (vars) | Consistent hierarchy |

**Key conversion: Image format to WebP**

After cleanup, a shell script converts PNG/JPG to WebP (smaller, modern format):

```bash
bb import-help
# Inside bb.edn:
# 1. Clojure runs development/help_import.clj (orchestrates cleanup)
# 2. Copies Images/ to projects/behave/resources/public/help/images/
# 3. Shell: convert *.png/*.jpg → *.webp (via ImageMagick convert)
# 4. Shell: rm *.png *.jpg (cleans up originals)
```

### Step 4: Datomic migration and transact

**Orchestrator:** `development/help_import.clj` (public functions `import-help` and `rollback-import`).

**How it works:**

1. Connects to Datomic via `projects/behave_cms/resources/config.edn`.
2. Queries for English language entity (shortcode `en-US`).
3. Creates two migrations (for audit trail):
   - `help-import-remove-<timestamp>`: Retracts old help pages.
   - `help-import-add-<timestamp>`: Asserts new help pages.
4. For each cleaned topic/variable:
   - Creates a `:help-page` entity with `:bp/uuid`, `:help-page/key`, `:help-page/content`.
   - Links to the English language via `:language/_help-page`.
5. Transacts both migrations atomically.

**Help-page key convention:**

- Format: dotted notation (e.g., `:help-page/key "feature.name.subfeature"`).
- Extracted from Flare topic metadata (class `metadata`).
- Used by UI to look up help content at runtime.

### Step 5: Client-side lookup and rendering

**Location in code:** `projects/behave/src/cljs/behave/wizard/views.cljs` or similar (not detailed here; see skill `behave-run-and-operate` for UI config).

**Lookup:** When user clicks help icon for a UI element:
1. Query DataScript for `:help-page/key "<feature.path>"`.
2. Fetch `:help-page/content` (HTML string).
3. Render in sidebar or modal; sanitize HTML (prevent XSS).

**Key data attribute:** UI elements carry a data-help-key attribute (or re-frame subscription) linking them to the appropriate `:help-page/key`.

### Running the import pipeline: `bb import-help` command

**Prerequisite:** `behave_cms` must be running (port 8001) and `Datomic transactor` running (port 8000).

**Steps:**

```bash
# Terminal 1: Start Datomic transactor (if not running)
cd /Users/rsheperd/code/sig/behave-app
bb transactor

# Terminal 2: Start CMS server (if not running)
# (Depends on Datomic being up; connect string in projects/behave_cms/resources/config.edn)
clojure -M:dev:behave/cms:figwheel

# Terminal 3: Run import
cd /Users/rsheperd/code/sig/behave-app
bb import-help
```

**What happens:**

1. Clojure reads `development/help_import.clj` and calls `(import-help)`.
2. Connects to Datomic, queries for en-US language entity.
3. Iterates over `bases/behave-docs/XHTML_Output/BehaveAppHelp/Content/*.htm`:
   - Calls `(clean-topic file)` → `{:key <help-key> :content <html>}`.
   - Builds Datomic transaction.
4. Iterates over `bases/behave-docs/.../Resources/Snippets/Variables/*.htm`:
   - Calls `(clean-variables file)` → `[{:key ... :content ...} ...]`.
   - Adds to transaction.
5. Copies Images/ to `projects/behave/resources/public/help/images/`.
6. Shell conversion: PNG/JPG → WebP via ImageMagick `convert` command.
7. Removes original PNG/JPG files.
8. Transaction is committed to Datomic.
9. On next client reload, new help pages are available via DataScript `:help-page/key` lookup.

**Rollback (if needed):**

```bash
clojure -X:dev:import-help :rollback-import
# Or with a specific timestamp:
clojure -X:dev:import-help :rollback-import "2026-07-06-14-30-45"
```

---

## Docstring and comment style: Clojure conventions

This repo follows the Clojure community style guide (with project-specific refinements). See `/Users/rsheperd/code/rjsheperd/claude-dev/guides/CLOJURE_STYLE.md` and `/Users/rsheperd/code/rjsheperd/claude-dev/guides/clojure-style-guide.md` for full reference; highlights below.

### Docstring requirements

**Rule:** Every public var (`defn`, `defn-`, `def`, etc.) in `src/` or `enterprise/backend/src` must have a docstring. Private vars (marked `^:private`) should have docstrings too (best practice).

**Format:**

```clojure
(defn my-function
  "One-sentence summary of what the function returns or does.
   
   Longer explanation if needed (2-3 sentences max).
   
   `param1` — description of parameter 1.
   `param2` — description of parameter 2.
   
   Returns a map with keys `:foo` and `:bar`."
  [param1 param2]
  ...)

(defn ^:private internal-helper
  "Helper for [[my-function]]; not part of public API."
  [x]
  ...)
```

**Key conventions:**

- First line: complete sentence summarizing return value or side effect.
- Parameter descriptions: wrap param names in backticks (`` `param1` ``).
- Cross-references: use `[[var-name]]` syntax (renders as link in most Clojure tools).
- Markdown inside docstrings: supported (e.g., `**bold**`, `_italic_`, `` `code` ``).
- Line length: 80 chars max; continue to next line with 2-space indent.
- No backticks around var names in docstrings that are already `[[linked]]`.

### Comment conventions in Clojure

```clojure
;;;; Section heading (4 semicolons)
;; Top-level comment explaining the section or function (2 semicolons)
(defn foo [x]
  ;; Code fragment comment explaining a logic block (2 semicolons)
  (when (valid? x)
    x))

;; FIXME (RJ Sheperd 2026-07-06) -- Description of what needs fixing
;; TODO (Name YYYY-MM-DD) -- Description of what to do later
;; HACK (Name YYYY-MM-DD) -- Temporary workaround; explain why
;; REVIEW (Name YYYY-MM-DD) -- Code that needs review for correctness/perf
```

**Comment style checklist:**

| Element | Style | Example |
|---------|-------|---------|
| Section heading | 4 semicolons `;;;;` | `;;;; Public API` |
| Top-level doc | 2 semicolons `;;` | `;; Helper to validate input` |
| Inline/margin | 1 semicolon `;` | Code at end of line: `x ; result` |
| Commented-out code | `#_` (reader macro, preferred) | `#_(old-impl x y)` or `; (comment-form)` |
| TODO/FIXME/HACK/REVIEW | Format: `;;TYPE (Name YYYY-MM-DD) -- description` | `;; TODO (Alice 2026-01-15) -- refactor to use transducers` |

### Code organization conventions

- **Private vs. public:** Mark non-exported fns with `^:private` metadata: `(defn ^:private helper [x] ...)`.
- **Module organization:** Put private helpers after public API; use `declare` sparingly (prefer moving code).
- **Var size:** Keep functions under 20 lines; use small helper functions.
- **Line length:** 120 chars max (100 preferred for tight code).

### Example: Well-documented public function

```clojure
(defn evaluate-expression
  "Evaluate a Rothermel fire-behavior expression against a worksheet.
   
   Applies unit conversion and boundary checking based on the target variable's
   domain. Returns a result map with `:value`, `:units`, and `:valid?` keys.
   
   `ws` — worksheet entity (has `:worksheet/inputs`, `:worksheet/outputs`).
   `expr` — expression AST (tree of operators and variable references).
   `var-key` — the target variable's dotted key (e.g., `\"surface.rate-of-spread\"`).
   
   Throws `:invalid/domain-bounds` if result falls outside variable's min/max.
   
   See [[validate-worksheet]] for validation before eval."
  [ws expr var-key]
  (let [var-def (lookup-variable ws var-key)
        result (eval-expr expr ws)
        coerced (coerce-units result (:variable/native-unit var-def))]
    (if (in-bounds? coerced var-def)
      {:value (:value coerced) :units (:units coerced) :valid? true}
      (throw (ex-info "Value out of bounds" 
                      {:error :invalid/domain-bounds 
                       :bounds [(:variable/min var-def) 
                               (:variable/max var-def)]})))))
```

---

## When NOT to use this skill

This skill covers **internal project documentation** (README.org, arch/ docs, help-content pipeline, commit/PR voice, and ticket-playbook convention). It does **not** cover:

| Adjacent topic | See instead | Why |
|---|---|---|
| **Code reviews, lint rules, commit hooks** | `behave-change-control` (skill #1) | That skill owns the PR gate, CI checks, and branch protection rules |
| **Test authoring, test running, CI harness** | `behave-validation-and-qa` (skill #10) | Test inventory, run commands, golden data, and evidence bar live there |
| **VMS data pipeline: C++ → CMS → migrations** | `behave-vms-variable-pipeline` (skill #6) | That skill owns Hatchet, cms-exports, migrations, layout.msgpack sync |
| **Build env, node/Emscripten/externs setup** | `behave-build-and-env` (skill #7) | Env recreation, prerequisite versions, known traps (node shim, EM_CACHE) |
| **Running dev server, desktop app, CMS server** | `behave-run-and-operate` (skill #8) | Config files, ports, flags, .bp7 worksheet format, release workflow |
| **Domain theory: fire-behavior equations, Rothermel, units, GACC** | `fire-behavior-reference` (skill #5) | Fire science fundamentals and this repo's implementation |
| **Architecture patterns, design decisions, invariants** | `behave-architecture-contract` (skill #4) | Why Polylith, why DataScript, why WASM; known weak points |

---

## Provenance and maintenance

Every fact in this skill can be re-verified with one-line commands. Run these to confirm freshness.

**Last verified:** 2026-07-06
**Changes:** Updated commit message and PR title formats to reflect observed git history showing mixed bracket usage; corrected re-verification grep command to count individual occurrences.

| Fact | Re-verification command | Expected output |
|------|---|---|
| README.org exists and has license placeholder | `grep -n "<INSERT LICENSE>" /Users/rsheperd/code/sig/behave-app/README.org` | `68:<INSERT LICENSE>` |
| arch/ directory has 4 docs numbered 00–03 | `ls -1 /Users/rsheperd/code/sig/behave-app/arch/*.org \| wc -l` | `4` |
| Arch doc index file exists | `test -f /Users/rsheperd/code/sig/behave-app/arch/00_index.org && echo OK` | `OK` |
| Ticket playbooks are untracked | `cd /Users/rsheperd/code/sig/behave-app && git status projects/behave/BHP1-*.org` | Files listed as untracked (or absent if no current tickets) |
| Recent commit uses plain-English format (no body) | `git -C /Users/rsheperd/code/sig/behave-app log --oneline -5 \| head -1` | `BHP1-#### <Plain English>` (no extra lines) |
| PR title format in GitHub merged PRs | `gh pr list -R firelab/behave-app --state closed --limit 3 --json title` | Each title starts with `[BHP1-####]` or `BHP1-####` (mixed formats) |
| behave-docs submodule exists | `ls -d /Users/rsheperd/code/sig/behave-app/bases/behave-docs && echo OK` | `OK` |
| DITA component exists with cleaner functions | `grep -l "clean-topic\|clean-variables" /Users/rsheperd/code/sig/behave-app/components/dita/src/dita/*.clj` | `xhtml_cleaner.clj` |
| bb import-help task defined | `grep -c "import-help" /Users/rsheperd/code/sig/behave-app/bb.edn` | `≥ 1` (multiple references) |
| help_import.clj orchestrator exists | `test -f /Users/rsheperd/code/sig/behave-app/development/help_import.clj && echo OK` | `OK` |
| Clojure style guides linked in CLAUDE.md | `grep -o "CLOJURE_STYLE\|clojure-style-guide" ~/.claude/CLAUDE.md \| wc -l` | `≥ 2` |

---

## Depth over brevity: why this skill is comprehensive

This skill is designed as the **document of record** for all docs-and-writing practices in Behave7. Rationale:

1. **README.org staleness:** The license section is objectively broken (placeholder text). This skill names the problem, explains why, and links to the fix.
2. **Arch docs organization:** Four docs exist but their scopes overlap with other skills. This skill clarifies what each owns and points to siblings that own deeper topics.
3. **Ticket-playbook discovery:** The brief noted local `.org` files exist but are not documented. This skill reverse-engineered the observed structure from 4 real tickets and codified it.
4. **Help-content pipeline:** No single doc exists describing the end-to-end flow. This skill maps the pipeline (Flare → XHTML → Clojure cleanup → Datomic → DataScript) and provides the `bb import-help` runbook.
5. **Commit/PR voice:** Git history shows a consistent pattern; this skill makes it explicit so future agents (or humans) can replicate it.
6. **Docstring style:** The project links to two external Clojure style guides; this skill embeds the essential rules and adds project-specific conventions.

Every line earned its place. If you're authoring docs or help content, start here.
