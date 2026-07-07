---
name: behave-run-and-operate
description: Run and operate Behave7 in every deployment mode (figwheel dev, server, JCEF desktop, CMS), config catalog (every key in config.edn and variants), data/worksheet conventions (.bp7 SQLite files), and release end-to-end (tag → jar-builder vs manual release, version dual-source, signing).
---

# Behave7 Running and Operating Guide

**Glossary:** JCEF (Java Chromium Embedded Framework) — Desktop runtime embedding Chromium in a native window, used in Conveyor releases for standalone .dmg/.exe/.deb packages.

**Document date: 2026-07-06** — facts verified against repo source; volatile items (ports, versions, paths) are marked for re-verification.

## When NOT to use this skill

This skill covers **operating and configuring** a running Behave7 instance. For related needs, see:

- **Build and environment setup** → `behave-build-and-env` (prerequisites, Node shims, Emscripten, submodules, externs, WASM compilation)
- **Debugging and diagnostics** → `behave-diagnostics-and-tooling` (profiling, WASM debugging, solver logs, test console capture, headless test runner)
- **Testing and validation** → `behave-validation-and-qa` (test tiers, golden data, Cucumber, headless runner commands, standing failures)
- **Release engineering** → This skill; see also `behave-docs-and-writing` for version bumping workflow details
- **VMS/CMS variable pipeline** → `behave-vms-variable-pipeline` (when adding/modifying C++ variables, WASM bindings, CMS entities)

---

## Part 1: Running Behave7 in Every Mode

### Mode 1: Figwheel Development (ClojureScript Hot-Reload)

**Purpose:** Local development with live code reload for the Behave app or CMS frontend.

#### Behave App Dev (Port 8081)

```bash
cd /Users/rsheperd/code/sig/behave-app

# From root; starts Figwheel on port 8081
clojure -M:dev:behave/app:figwheel
```

**What happens:**
- Figwheel build config: `projects/behave/figwheel-main.edn` (line 5)
- Compiles dev build: `projects/behave/compile-dev.cljs.edn`
- Ring server starts on port **8081** (`figwheel-main.edn` line 5: `:port 8081`)
- Opens browser to `http://localhost:8081/worksheets`
- Hot-reloads CLJS on file changes in `src/cljs` and `bases/behave_components/src`

**How to know it's up:**
- Ring server logs appear: `Figwheel: ... Server started on ... :port 8081`
- Browser auto-opens (or manually open `http://localhost:8081/worksheets`)
- In terminal, see compile messages like `compiling ClojureScript...`

**Stopping:** `Ctrl-C` in terminal

---

#### Behave CMS Dev (Port 8082)

```bash
cd /Users/rsheperd/code/sig/behave-app

# From root; starts Figwheel CMS on port 8082
clojure -M:dev:behave/cms:figwheel
```

**What happens:**
- Figwheel build config: `cms.cljs.edn` (root; lines 9-12 override port)
- Uses compile-dev via figwheel.main `-b cms` → reads `cms.cljs.edn` config
- Ring server starts on port **8082** (`cms.cljs.edn` line 9: `:port 8082`)
- Opens browser to `http://localhost:8082/login`
- Watches `projects/behave_cms/src/cljs`, `bases/behave_components/src`

**How to know it's up:**
- Browser opens to CMS login page
- Figwheel logs show CMS compile messages

**Prerequisites:**
- Datomic transactor running on port 4334 (run `bb transactor` in separate terminal)
- PostgreSQL running (or already initialized in `pgdata/` at `localhost:5432`)

**Stopping:** `Ctrl-C` in terminal; also stop transactor

---

### Mode 2: Server Mode (HTTP Jetty, No Desktop)

**Purpose:** Headless server deployment (e.g., Docker, VPS, CI, or local testing without JCEF).

```bash
cd /Users/rsheperd/code/sig/behave-app/projects/behave

clojure -M:server
```

**What happens:**
- Entry point: `behave.server/-main` (projects/behave/src/clj/behave/server.clj)
- Reads config from `projects/behave/config.edn`
- Jetty starts on port **9101** (config.edn line 4: `:http-port 9101`)
- DataScript persisted to `~/.behave/db.sqlite` (config.edn line 1: `"~/.behave/db.sqlite"`)
- Does **not** load JCEF (no CEF natives bundled)

**How to know it's up:**
- Logs appear: `Jetty Started on port 9101`
- Access app at `http://localhost:9101`
- Database file created at `~/.behave/db.sqlite` (SQLite 3)

**How to run the built uberjar in server mode:**
```bash
cd /Users/rsheperd/code/sig/behave-app/projects/behave

# Build an uberjar
clojure -X:build-uberjar

# Run the jar (defaults to server mode if not Conveyor-packaged)
java -jar target/behave7-*.jar
```

**Stopping:** `Ctrl-C` in terminal; graceful shutdown closes database connection

---

### Mode 3: Standalone/Desktop Mode (JCEF + Conveyor)

**Purpose:** Packaged desktop application (macOS .dmg, Windows .exe, Linux .deb) with embedded CEF browser.

**Runtime detection:** When `System.getProperty("app.dir")` is set (by Conveyor), app starts in JCEF desktop mode (`behave.core/-main` line 119: `if (conveyor?)`).

#### Running the Desktop App from Source (Dev)

```bash
cd /Users/rsheperd/code/sig/behave-app/projects/behave

# Standalone alias starts in desktop mode; requires JVM module exports
clojure -M:standalone
```

**What happens:**
- Detects **not** a Conveyor build (no `app.dir` property), so falls back to HTTP server mode
- Actually runs as server mode in this case—true desktop mode only when released via Conveyor

#### Running the Desktop App from Release (Conveyor-Packaged)

```bash
# Downloaded .dmg (macOS), .exe (Windows), or .deb (Linux) from release page
# Extract and run the app via OS-specific launcher

# macOS
open ~/Downloads/behave7.dmg
# Double-click app

# Windows
cd ~/Downloads
behave7-setup.exe
# Follow installer

# Linux
sudo dpkg -i ~/Downloads/behave7_*.deb
behave7  # Run from shell
```

**What happens:**
- Conveyor sets `app.dir` property pointing to app installation dir
- behave.core detects `conveyor?` is true, calls `start-cef!`
- JCEF creates a native Chromium window
- App data stored at `~/.behave/db.sqlite` and `~/.behave/logs` (platform-specific via `app-data-dir`)
- UI launches at `http://localhost:RANDOM_PORT` (default 4242 in config.standalone.edn if not overridden)

**How to know it's up:**
- Native window opens with Behave logo
- App loads worksheets at the `/worksheets` route

**Stopping:** Close the window; app exits cleanly

---

### Mode 4: CMS Server (Admin Console, Port 8001)

**Purpose:** Variable Management System (VMS) admin interface for configuring app structure, variables, units, help content.

**Prerequisites:** Must start in this order (CMS depends on these):

1. **Start PostgreSQL** (if not already running):
   ```bash
   # If using local pgdata/ directory (development only):
   # PostgreSQL should already be initialized; check pgdata/ exists
   
   # If using system PostgreSQL:
   pg_isready -h localhost -p 5432
   # Should return "accepting connections"
   ```

2. **Start Datomic transactor:**
   ```bash
   cd /Users/rsheperd/code/sig/behave-app
   
   bb transactor
   ```
   
   **What happens:**
   - Reads config: `bases/datomic_store/config/datomic-sql.properties` (hardcoded JDBC URL, port 4334)
   - Connects to PostgreSQL at `jdbc:postgresql://localhost:5432/datomic` user `datomic`/`datomic`
   - Listens on port **4334** for peer connections
   - Logs appear: `Datomic database initialized...`
   
   **How to know it's up:**
   - Transactor logs show no errors
   - You can telnet: `telnet localhost 4334` (should connect)

3. **Start CMS server:**
   ```bash
   cd /Users/rsheperd/code/sig/behave-app/projects/behave_cms
   
   clojure -M:server
   ```
   
   **What happens:**
   - Entry point: `behave-cms.server/-main` (projects/behave_cms/src/clj/behave_cms/server.clj)
   - Reads config: `projects/behave_cms/resources/config.edn`
   - Connects to Datomic peer via transactor at `datomic:sql://...` (port 4334)
   - Jetty starts on port **8001** (config.edn line 2)
   - Migrations run automatically (if any pending)
   
   **How to know it's up:**
   - Logs: `Jetty Started on port 8001`
   - Access at `http://localhost:8001/login`
   - Datomic console available at `http://localhost:8001/console` (dev mode only)

#### CMS Dev Mode (Figwheel + Server)

To develop CMS frontend + backend together:

```bash
# Terminal 1: Datomic transactor
bb transactor

# Terminal 2: CMS server
cd projects/behave_cms
clojure -M:server

# Terminal 3: CMS Figwheel (hot-reload frontend)
cd /path/to/root
clojure -M:dev:behave/cms:figwheel
```

Then access `http://localhost:8082/login` (Figwheel dev port) or `http://localhost:8001/login` (production server).

---

## Part 2: Configuration Axes Catalog

All configuration is read from EDN files at startup. **No environment variables**—all config is via EDN.

### Behave App Configuration

**File:** `projects/behave/config.edn` (development) or `projects/behave/resources/config.edn` (production/server)

**How it's loaded:** `config.interface/load-config` reads from classpath at startup

| Key Path | Type | Default (dev) | Default (prod) | Description | Readers |
|----------|------|---|---|---|---|
| `:database.config.store.backend` | keyword | `:file` | `:file` | Persistence backend (`:file` = SQLite) | `server/init-db!` |
| `:database.config.store.path` | string | `"~/.behave/db.sqlite"` | `"~/.behave/db.sqlite"` | SQLite file path (expands `~` to home dir); modified by JCEF to `~/.behave/db.sqlite` in standalone mode | `datom-store.main/default-conn` |
| `:site.title` | string | `"Behave 7"` | `"Behave 7"` | Browser tab title, app name | `behave.views/render-app` |
| `:site.description` | string | `"Wildfire Analysis toolkit."` | (same) | Not currently used; reserved for future | — |
| `:site.org-name` | string | (not set in dev) | `"FireLab"` | Organization name for app-data-dir path (JCEF only) | `file-utils.interface/app-data-dir` |
| `:site.app-name` | string | (not set in dev) | `"Behave7"` | App name for app-data-dir path (JCEF only) | `file-utils.interface/app-data-dir` |
| `:server.http-port` | integer | `9101` | `9101` | Jetty listen port | `behave.server/start-server!` |
| `:server.mode` | string | `"dev"` | `"dev"` (overridden to `"prod"` in JCEF) | `"dev"` or `"prod"`; affects logging verbosity, database path in JCEF | `behave.core/start-cef!` line 84 |
| `:logging.log-dir` | string | `"logs"` | `"logs"` | Relative or absolute path for `.log` files; JCEF overrides to `~/.behave/logs` in prod mode | `logging.interface/start-logging!` |
| `:logging.log-memory-interval` | integer | `5` | `5` | Log memory stats every N seconds (0 = disabled) | `logging.interface/start-logging!` |
| `:vms.url` | string | `"https://localhost:8082"` | (same) | URL of CMS server for fetching layout.msgpack and VMS data | `behave.vms.store/load-vms!` |
| `:vms.secret-token` | string | `"<REDACTED — see projects/behave_cms/resources/config.edn>"` | (same) | Auth token for VMS sync calls (hardcoded in dev; should use env var in production) | `behave.sync/vms-sync!` |
| `:client.jar-local?` | boolean | (not set) | `true` (in standalone only) | If true, disables VMS URL override and uses embedded layout.msgpack | `behave.handlers/app-init-state` |

**Notes:**
- `:server.mode "dev"` enables CORS, test endpoints, REPL access
- `:server.mode "prod"` locks down endpoints, disables debug features
- `:vms.url` must be a running CMS instance (or `https://firelab-dev.sig-gis.com` for production)
- Database path `~/.behave/db.sqlite` is macOS/Linux; Windows uses `%APPDATA%\Behave\db.sqlite` (via `app-data-dir`)

---

### CMS Server Configuration

**File:** `projects/behave_cms/resources/config.edn`

| Key Path | Type | Default | Description | Readers |
|----------|------|---|---|---|
| `:database.project` | string | `"behave"` | Datomic database name (created by transactor if missing) | `datomic-store.main/default-conn` |
| `:server.http-port` | integer | `8001` | Jetty listen port | `behave-cms.server/-main` |
| `:server.mode` | string | `"prod"` | `"dev"` or `"prod"`; affects CORS, test endpoints | `behave-cms.server/-main` |
| `:server.log-dir` | string | `"logs"` | Relative path for CMS logs | `logging.interface/start-logging!` |
| `:mail.host` | string | `"smtp.gmail.com"` | SMTP server for sending verification/password-reset emails | `behave-cms.email/send-email!` |
| `:mail.user` | string | `"support@sig-gis.com"` | SMTP login email (app password, not user password) | — |
| `:mail.pass` | string | `"<REDACTED — see projects/behave_cms/resources/config.edn>"` | SMTP app password (Gmail 2-factor app password) | — |
| `:mail.port` | integer | `587` | SMTP port (587 = TLS, 25 = plaintext) | — |
| `:mail.tls` | boolean | `true` | Enable TLS encryption | — |
| `:mail.site-url` | string | `"https://firelab-dev.sig-gis.com"` | Base URL in password-reset/verification emails | — |
| `:secret-token` | string | `"<REDACTED — see projects/behave_cms/resources/config.edn>"` | Shared token for VMS ↔ CMS sync endpoint auth | `behave-cms.handlers/sync-endpoint` |

**Notes:**
- Mail config is hardcoded for Gmail SMTP; production must use actual Gmail credentials
- `:secret-token` must match between app (`behave/config.edn`) and CMS for sync to work
- Database name `:database.project "behave"` is created automatically by Datomic transactor if missing

---

### Standalone/Desktop Configuration Swap

When building for release via Conveyor:

1. **Dev config** (`projects/behave/config.edn`): Uses dev ports, localhost CMS URL
2. **Standalone config** (`projects/behave/resources/config.standalone.edn`): Uses prod paths, port 4242, embedded VMS data
3. **Release build** (`jar-builder.yml` line 63): `mv resources/config.standalone.edn resources/config.edn`

This swap ensures the released .jar/.dmg/.exe runs in prod mode with correct data paths.

**config.standalone.edn differences:**
```edn
:server.http-port 4242           ;; (vs 9101 in dev)
:server.mode "prod"              ;; (vs "dev")
:logging.log-dir "~/.behave/logs" ;; (vs "logs" relative)
:client.jar-local? true          ;; Use embedded VMS data, ignore :vms.url
```

---

### Version Dual-Source (Drift Hazard)

**Problem:** Version is defined in two places; release process must keep them in sync.

| File | Example | Updated By |
|------|---------|---|
| `projects/behave/resources/version.edn` | `{:version "v7.1.4"}` | `jar-builder.yml` line 64: `echo '{:version \"$version\"}' > resources/version.edn` (from git tag) |
| `projects/behave/conveyor.base.conf` | `app { version = 7.1.4 }` line 29 | **Manual** (must update before release or after tag) |

**Risk:** If `conveyor.base.conf` is not updated to match the git tag, the released .dmg/.exe will have a stale version string in the installer.

**Mitigation:** When releasing, verify both files match:
```bash
# Extract version from tag
VERSION=$(git describe --tags)
echo "Tag version: $VERSION"

# Check version.edn (will be updated by jar-builder.yml)
grep :version projects/behave/resources/version.edn

# Check conveyor.base.conf (must be manual)
grep "app.version" projects/behave/conveyor.base.conf
```

---

## Part 3: Data Conventions

### Worksheet Files (.bp7)

`.bp7` files are **SQLite 3 databases** containing user worksheet data (inputs, computed outputs, graphs, notes).

**Location:** Development/testing worksheets stored in `worksheets/` directory (as of 2026-07-06):
```
worksheets/
├── 30-min.bp7              # Example worksheet (30-minute fire scenario)
├── BHP1-1226.bp7           # Ticket-based test fixture
├── BHP1-1226.zip           # Exported results archive (companion file)
├── ...
```

**File Association:** Windows/macOS app registers `.bp7` extension (`conveyor.base.conf` line 32: `file-associations = [ .bp7 ]`); double-clicking opens in Behave7.

**Runtime Path (Standalone Mode):**
- macOS: `~/.behave/db.sqlite` (via `app-data-dir` "FireLab" / "Behave7")
- Windows: `%APPDATA%\Behave\db.sqlite`
- Linux: `~/.behave/db.sqlite`

**Runtime Path (Server Mode):**
- Always `~/.behave/db.sqlite` (config.edn line 1)

**Schema:** SQLite schema defined by DataScript Datom store; no documented human-readable schema (reverse-engineer via `sqlite3 ~/.behave/db.sqlite .schema` or read `bases/datom_store/src/datom_store/main.clj`).

**Export Format:** When user exports worksheet, app creates a `.zip` containing:
- `worksheet.edn` (DataScript datoms as EDN)
- `results.csv` (tabular outputs)
- `graphs/` (SVG diagram exports)
- Companion `.zip` file created alongside each `.bp7` for archival

**Import/Restore:** Open `.bp7` file in app (File > Open or drag-drop). App uses `d/restore-conn` (not `d/create-conn`) on existing DB files (DataScript storage-sql requirement).

---

### Logging and Data Directories

**Development/Server Mode:**
```
logs/                           # Relative path (cwd = projects/behave)
  behave7-YYYY-MM-DD.log       # Main app log
  behave7-request-YYYY-MM-DD.log # HTTP request log
```

**Standalone/Desktop Mode:**
```
~/.behave/logs/                 # Absolute path (home/.behave/logs/)
  behave7-YYYY-MM-DD.log
  behave7-request-YYYY-MM-DD.log
```

**Log Rotation:** Daily files; old logs retained (no automatic cleanup).

**Log Level:** Controlled by `logging.log-memory-interval`; all logs go to `.log` files and stdout (dev) or silent (prod).

---

### VMS Data (layout.msgpack)

The VMS layout is a binary MessagePack serialization of the app schema (modules, variables, units, help content).

**Loaded at startup via:** `behave.vms.store/load-vms!` → downloads from `:vms.url/sync` endpoint.

**Development:** CMS server (`http://localhost:8082` or `http://localhost:8001`) exports layout.msgpack dynamically on `/sync` endpoint; browser requests it at app init.

**Production/Standalone:** `.jar` includes a pre-built `cms-exports/layout-latest.msgpack` (generated offline during release build via `clojure -X:download-vms`).

**Versioning:** No explicit versioning; re-exporting CMS data immediately propagates schema changes to all clients on next app reload.

---

## Part 4: Import and Download Operations

### Import Help Content (bb import-help)

**Purpose:** Sync DITA documentation source (MadCap Flare) into CMS Datomic database.

```bash
cd /Users/rsheperd/code/sig/behave-app

bb import-help
```

**What happens (bb.edn lines 27-33):**

1. Runs `clojure -X:dev:import-help` (root `:import-help` alias, line 175-179)
   - Loads DITA XHTML from `bases/behave-docs/XHTML_Output/`
   - Parses via `dita.xhtml-cleaner` (hickory/hiccup DOM parser)
   - Extracts help-page keys from `<div class='metadata'>` in each topic
   - Transacts to Datomic with `:help-page/key`, `:help-page/content`, `:help-page/uuid`
   - Marks with migration ID for tracking (e.g., `"help-import-add-2026-07-06-14-30-45"`)

2. Copies images from `bases/behave-docs/XHTML_Output/Resources/Images/` to `projects/behave/resources/public/help/images/`

3. Converts all `.jpg`, `.jpeg`, `.png` to `.webp` format via ImageMagick:
   ```bash
   convert input.jpg output.webp
   ```

4. Deletes original `.jpg`/`.png` files (WebP only in production)

**Prerequisites:**
- Datomic transactor running (see Mode 4)
- ImageMagick installed (`convert` command available)
- CMS server running (required if using Datomic peer)

**How to know it worked:**
- Help content appears in CMS admin UI
- `~/.behave/db.sqlite` includes `:help-page/*` entities (if using DataScript locally)
- No errors in logs; image files converted to `.webp`

**Rollback:** To undo a help import:
```bash
cd /Users/rsheperd/code/sig/behave-app

clojure -X:dev:rollback-help-import
```
(Searches migration DB for last `help-import-*` ID and retracts those datoms)

---

### Download VMS Data (clojure -X:download-vms)

**Purpose:** Pre-release step to fetch latest CMS layout and save to jarfile resources.

```bash
cd /Users/rsheperd/code/sig/behave-app/projects/behave

clojure -X:download-vms :url "https://firelab-dev.sig-gis.com" :auth-token "YOUR_TOKEN"
```

**Alias Definition:** projects/behave `deps.edn` line 101:
```clojure
:download-vms {:exec-fn behave.download-vms/exec-export-from-vms}
```

**Arguments:**
- `:url` — VMS server URL (defaults to hardcoded `https://firelab.sig-gis.com` if omitted; **verify this default is correct**)
- `:auth-token` — Auth token for VMS sync endpoint (from `:vms.secret-token` in CMS config)

**What happens (projects/behave/src/clj/behave/download_vms.clj):**

1. GETs `{url}/sync` endpoint with `Authorization: Bearer {auth-token}` header
2. Receives MessagePack binary data (layout.msgpack)
3. Saves to `projects/behave/resources/public/layout-latest.msgpack`
4. Also downloads any companion files (images, etc.)

**How to know it worked:**
- File `projects/behave/resources/public/layout-latest.msgpack` created/updated
- No HTTP 401/403 errors (auth failure)
- File size > 1MB (typical layout is 10-50MB)

**Used by:** `jar-builder.yml` line 61 (in release build loop)

---

## Part 5: Release Operation End-to-End

### Release Flow Diagram

```
Tag push (v7.1.4)
  ↓
jar-builder.yml (automated, on any tag)
  ├─ Build JAR
  ├─ Download VMS data
  ├─ Package Windows (Conveyor)
  ├─ Sign Windows (Azure Trusted Signing)
  └─ Upload artifacts (5-day retention)
  
release.yml (manual dispatch, workflow_dispatch)
  ├─ User selects platforms (Windows, macOS-Intel, macOS-ARM, Linux)
  ├─ Download JAR artifact from jar-builder
  ├─ Build platform packages (Conveyor)
  ├─ Sign Windows (Azure Trusted Signing)
  ├─ Create GitHub Release
  └─ Upload all artifacts (permanent)
```

---

### Step 1: Prepare for Release

**Ensure version is synchronized:**

```bash
cd /Users/rsheperd/code/sig/behave-app

# Check current version in both files
cat projects/behave/resources/version.edn
grep "app.version" projects/behave/conveyor.base.conf

# They should match (e.g., both v7.1.4 or 7.1.4)
```

**Bump version if needed:**

Use GitHub Actions workflow (manual dispatch):

```bash
# Go to GitHub Actions > bump-version
# Input: new version (e.g., v7.1.5)
# Workflow updates:
#   - projects/behave/resources/version.edn
#   - Commits and pushes to main
# Then update conveyor.base.conf manually:

# Edit projects/behave/conveyor.base.conf line 29:
# app { version = 7.1.5 }  # Change this

# Commit and push:
git add projects/behave/conveyor.base.conf
git commit -m "BHP1-XXXX Bump version to 7.1.5"
git push origin main
```

---

### Step 2: Tag and Push (Triggers Automated jar-builder.yml)

```bash
cd /Users/rsheperd/code/sig/behave-app

# Create tag (use v prefix; jar-builder strips it)
git tag v7.1.5

# Push tag
git push origin v7.1.5
```

**What happens automatically:**

1. GitHub Actions detects tag push
2. `jar-builder.yml` workflow starts (runs on `ubuntu-latest` via Nix)
3. Extracts version from tag: `sed 's/refs\/tags\///g'` → `v7.1.5`
4. Downloads VMS data via `clojure -X:download-vms`
5. Builds JAR via `bb build-js && bb uber`
6. Builds Windows package via Conveyor (`make windows-zip`)
7. Signs Windows `.exe`/`.msi` via Azure Trusted Signing
8. Uploads artifacts to GitHub (5-day retention):
   - `behave7-jar` (uberJAR)
   - `windows-zip` (signed)
   - `mac-zip-amd64`, `mac-zip-aarch64` (notarized; optional)
   - `linux-deb` (optional)

**How to monitor:**
- Go to GitHub > Actions > "Build and Package" workflow
- Wait for `build-jar` job to complete (10-15 min)
- Check for success/failure; view logs

---

### Step 3: Manual Release (Workflow Dispatch)

If you need to re-build, change platforms, or skip jar-builder, use manual release:

```bash
# Go to GitHub > Actions > "Manual Release"
# Fill in inputs:
#   tag: v7.1.5
#   build_windows: true  (default)
#   build_macos: false   (default; set true for macOS)
#   build_linux: false   (default; set true for Linux)
# Click "Run workflow"
```

**OR** trigger via GitHub CLI:

```bash
gh workflow run release.yml -f tag=v7.1.5 -f build_macos=true
```

**Workflow steps:**

1. Downloads JAR artifact from jar-builder (or builds fresh if jar-builder hasn't run)
2. For each selected platform, calls `conveyor.yml` reusable workflow:
   - Windows: `make windows-zip`
   - macOS amd64: `make notarized-mac-zip -Kapp.machines=mac.amd64`
   - macOS aarch64: `make notarized-mac-zip -Kapp.machines=mac.aarch64`
   - Linux: `make debian-package`
3. Signs Windows via Azure Trusted Signing
4. Creates GitHub Release with auto-generated notes from git log
5. Uploads all artifacts to release (permanent)

**How to know it worked:**
- GitHub Release created with tag name and download links
- Download artifact and verify it runs locally

---

### Release Secret Environment Variables (DO NOT COMMIT)

These secrets are required in GitHub repository settings for signing/notarization to work. **Names only; never commit values.**

| Secret Name | Used By | Purpose |
|---|---|---|
| `VMS_URL` | `jar-builder.yml` | CMS server URL for downloading layout.msgpack |
| `VMS_AUTH_TOKEN` | `jar-builder.yml` | Auth token for VMS sync endpoint |
| `CONVEYOR_ROOT_KEY` | `conveyor.yml` | Conveyor license key (line 68 Windows) |
| `AZURE_SIGNING_ALIAS` | `sign.yml` (Azure action) | Certificate alias in Azure Key Vault |
| `AZURE_SIGNING_REGION` | `sign.yml` (Azure action) | Azure region (e.g., "eastus") |
| `AZURE_TENANT_ID` | `sign.yml` (Azure action) | Azure tenant ID for Trusted Signing |
| `AZURE_CLIENT_ID` | `sign.yml` (Azure action) | Azure service principal client ID |
| `AZURE_CLIENT_SECRET` | `sign.yml` (Azure action) | Azure service principal secret |
| `APPLE_SIGNING_KEY` | `conveyor.yml` (macOS) | P12 certificate password |
| `APPLE_SIGNING_KEY_PATH` | `conveyor.yml` (macOS) | Path to P12 certificate file in repo |
| `APPLE_SIGNING_P12_ENCODED` | `conveyor.yml` (macOS) | Base64-encoded P12 certificate (for CI) |
| `MAC_NOTARY_ISSUER` | `conveyor.yml` (macOS) | Apple notarization issuer ID |
| `MAC_NOTARY_KEY` | `conveyor.yml` (macOS) | Apple notarization key ID |
| `MAC_NOTARY_PRIVATE_KEY_ENCODED` | `conveyor.yml` (macOS) | Base64-encoded Apple private key (for CI) |

**Verification:**
```bash
# Check what secrets are configured (names only, not values)
gh secret list

# Output:
# APPLE_SIGNING_KEY_PATH  branch  ...
# AZURE_CLIENT_ID         branch  ...
# ... (others)
```

---

### Signing Details

#### Windows: Azure Trusted Signing

**Trigger:** `sign.yml` reusable workflow (called by `jar-builder.yml` and `release.yml`)

**What it does:**
1. Downloads `windows-zip` artifact (unsigned `.exe`/`.msi`)
2. Invokes custom GitHub Action: `rjsheperd/az-jsign-trusted-signing@main`
3. Connects to Azure Key Vault using service principal credentials
4. Signs binary with configured certificate alias
5. Re-uploads as `windows-zip-signed`

**Workflow file:** `.github/workflows/sign.yml` (reusable)

**Secret env vars:** `AZURE_*` (see table above)

**Verification:**
```bash
# After release, download Windows .exe
signtool verify /pa /v behave7-setup.exe
# Should show: "Successfully verified: behave7-setup.exe"
```

---

#### macOS: Notarization

**Trigger:** `conveyor.yml` reusable workflow (line 83-93)

**What it does:**
1. Decodes base64-encoded secrets to files:
   - `APPLE_SIGNING_P12_ENCODED` → `~/.behave/.env/apple.p12`
   - `MAC_NOTARY_PRIVATE_KEY_ENCODED` → `~/.behave/.env/AuthKey.p8`
2. Invokes Conveyor with macOS config overlays:
   - `conveyor.macos-ci.conf` (line 38-44)
3. Conveyor:
   - Signs `.dmg` with P12 certificate
   - Submits to Apple notarization service
   - Polls for completion (can take 5-30 min)
   - Staples notarization ticket to `.dmg`
4. Outputs `.dmg` to `output/mac-*/`

**Secret env vars:** `APPLE_*`, `MAC_NOTARY_*` (see table above)

**Verification:**
```bash
# After release, download macOS .dmg
spctl -a -vv -t install behave7.dmg
# Should show: "accepted" (notarization verified)
```

---

### Creating a Release Without Automated Build

If `jar-builder.yml` is skipped or broken, manually build and release:

```bash
cd /Users/rsheperd/code/sig/behave-app/projects/behave

# Step 1: Ensure version is correct
echo '{:version "v7.1.5"}' > resources/version.edn

# Step 2: Download VMS data
clojure -X:download-vms :url "https://firelab-dev.sig-gis.com" :auth-token "YOUR_TOKEN"

# Step 3: Build JAR
bb build-js
bb uber
mv target/behave7-*.jar target/behave7.jar

# Step 4: Manually build platform packages (requires Conveyor + signing keys locally)
# NOT RECOMMENDED; use GitHub Actions instead
```

---

## Part 6: Post-Release Verification

After a release is created, verify:

1. **GitHub Release exists** with correct tag and version string
2. **Artifacts downloadable:**
   ```bash
   # Test Windows download
   curl -L -o behave7-windows.zip "https://github.com/firelab/behave-app/releases/download/v7.1.5/behave-VERSION-Windows.zip"
   
   # Test macOS download
   curl -L -o behave7-macos.dmg "https://github.com/firelab/behave-app/releases/download/v7.1.5/behave-VERSION-macOS.dmg"
   ```

3. **Signatures valid:**
   - Windows: `signtool verify /pa /v` (see Signing Details section)
   - macOS: `spctl -a -vv -t install` (see Signing Details section)

4. **Version string correct in app:**
   - Extract `.jar` and check: `unzip -p behave7.jar resources/version.edn`
   - Should show: `{:version "v7.1.5"}`

5. **App runs from downloaded package:**
   - Windows: Run `.exe` installer, open app, check Help > About for version
   - macOS: Mount `.dmg`, drag app to Applications, launch, check About
   - Linux: Install `.deb`, run `behave7`, check version

---

## Provenance and Maintenance

**Date verified: 2026-07-06** — Re-verify these commands and facts:

### Critical ports and aliases
```bash
# Verify figwheel app port 8081
grep -n "8081" /Users/rsheperd/code/sig/behave-app/projects/behave/figwheel-main.edn

# Verify figwheel CMS port 8082
grep -n "8082" /Users/rsheperd/code/sig/behave-app/cms.cljs.edn

# Verify server port 9101
grep -n "9101" /Users/rsheperd/code/sig/behave-app/projects/behave/config.edn

# Verify CMS port 8001
grep -n "8001" /Users/rsheperd/code/sig/behave-app/projects/behave_cms/resources/config.edn

# Verify Datomic transactor port 4334
grep -n "4334" /Users/rsheperd/code/sig/behave-app/bases/datomic_store/config/datomic-sql.properties
```

### Configuration keys
```bash
# Verify config.edn keys
jq 'keys' /Users/rsheperd/code/sig/behave-app/projects/behave/config.edn

# Verify config.standalone.edn exists and has prod paths
cat /Users/rsheperd/code/sig/behave-app/projects/behave/resources/config.standalone.edn

# Verify CMS config keys
jq 'keys' /Users/rsheperd/code/sig/behave-app/projects/behave_cms/resources/config.edn
```

### Version dual-source
```bash
# Check both versions match
cat /Users/rsheperd/code/sig/behave-app/projects/behave/resources/version.edn
grep "app {" /Users/rsheperd/code/sig/behave-app/projects/behave/conveyor.base.conf | head -3
```

### Workflow files
```bash
# Verify jar-builder.yml downloads VMS and builds JAR
grep -A2 "Download VMS" /Users/rsheperd/code/sig/behave-app/.github/workflows/jar-builder.yml

# Verify release.yml dispatch trigger
grep -A10 "workflow_dispatch:" /Users/rsheperd/code/sig/behave-app/.github/workflows/release.yml | head -15

# Verify conveyor.yml multi-platform support
grep "make " /Users/rsheperd/code/sig/behave-app/.github/workflows/conveyor.yml
```

### Runtime paths
```bash
# Verify Conveyor detection
grep -n "conveyor?" /Users/rsheperd/code/sig/behave-app/projects/behave/src/clj/behave/core.clj

# Verify app-data-dir usage for JCEF
grep -n "app-data-dir" /Users/rsheperd/code/sig/behave-app/projects/behave/src/clj/behave/core.clj
```

### Data directory conventions
```bash
# Verify worksheet fixtures directory
ls -l /Users/rsheperd/code/sig/behave-app/worksheets/ | head -10

# Verify SQLite config
grep "\.sqlite" /Users/rsheperd/code/sig/behave-app/projects/behave/config.edn
```

### Import and download operations
```bash
# Verify bb import-help task
grep -A10 "import-help" /Users/rsheperd/code/sig/behave-app/bb.edn

# Verify download-vms alias
grep -n "download-vms" /Users/rsheperd/code/sig/behave-app/deps.edn
grep -n "exec-export-from-vms" /Users/rsheperd/code/sig/behave-app/projects/behave/src/clj/behave/download_vms.clj
```

