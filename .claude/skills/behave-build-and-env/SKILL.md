---
name: behave-build-and-env
description: Build a working Behave7 development or production environment from scratch; prerequisites checklist; WASM toolchain setup; known traps (Bun-shim node, EM_CACHE, externs, submodules); build command working directories
---

# Behave7 Build and Environment Setup

**Date**: 2026-07-06 | **Version**: v7.1.4 (verify in resources/version.edn and conveyor.base.conf)

## Purpose

This skill guides you from a blank machine to a working Behave7 dev or production environment. It covers every prerequisite, known environment traps, the WASM build chain, and build command working directories. Use it when:

- Setting up a dev machine for the first time
- Debugging cryptic build or test failures (often rooted in missing/misconfigured prerequisites)
- Building a release JAR or desktop/web distribution
- Understanding why `bb build-js` or `clojure -X:build-uberjar` fail silently

**Do NOT use this skill for**:
- Running the app (→ `behave-run-and-operate`)
- Testing and validation procedures (→ `behave-validation-and-qa`)
- Debugging solver logic or units (→ `behave-debugging-playbook`)
- VMS variable pipeline or data import (→ `behave-vms-variable-pipeline`)

---

## What Is Behave7? (30-second glossary)

Behave7 is a USFS fire-behavior modeling application: a Clojure/ClojureScript monorepo with a Polylith layout.

- **C++ fire science** (`behave-lib/behave-mirror` submodule, branch `rj-rust-port`): Rothermel model compiled to WebAssembly (WASM)
- **Front-end SPA** (`projects/behave`): re-frame + re-posh + DataScript, ships as uberjar + JCEF Chromium or web server (port 9101)
- **CMS** (`projects/behave_cms`): Datomic + Postgres (port 8001 dev server), defines variables/modules/units/help content → exports as `layout.msgpack`
- **Build outputs**: `.wasm` + `.js` WASM bindings (checked in), compiled ClojureScript, uberjar, native installers (Conveyor)

---

## Prerequisites Checklist

### 1. Java Development Kit (JDK)

**Requirement**: JDK 17 (OpenJDK or vendor-specific)

| Prerequisite | Requirement | Verification Command | Expected Output |
|---|---|---|---|
| JDK version | 17 (required for behave, JCEF interop) | `java -version` | `openjdk version "17.x.x"` or similar; **NOT** 11, 21, or older |
| JDK location | Anywhere in PATH | `which java` | `/usr/libexec/java_home` or similar |

**Setup**:

```bash
# macOS (recommended)
brew install openjdk@17

# Ubuntu/Debian
apt-get install openjdk-17-jdk

# Or download from adoptium.net or aws.amazon.com/corretto
```

### 2. Clojure CLI

**Requirement**: Clojure 1.11.1+ (via `clojure` CLI tool)

| Prerequisite | Requirement | Verification Command | Expected Output |
|---|---|---|---|
| Clojure CLI installed | 1.11+ | `clojure --version` | `Clojure CLI version 1.11.x` or higher |
| `deps.edn` parsing | Valid root `deps.edn` | `cd /path/to/behave-app && clojure -Spath \| head -c 100` | Classpath list (no error) |

**Setup**:

```bash
# macOS
brew install clojure

# Ubuntu/Debian/Linux
curl -O https://download.clojure.org/install/linux-install-1.11.4.1357.sh
chmod +x linux-install-*.sh
sudo ./linux-install-*.sh

# Or: https://clojure.org/guides/install_clojure
```

### 3. Babashka

**Requirement**: Babashka 1.0+ (task runner for Clojure scripts)

| Prerequisite | Requirement | Verification Command | Expected Output |
|---|---|---|---|
| Babashka installed | 1.0+ | `bb --version` | `babashka v1.x.x` or higher |
| `bb.edn` tasks available | Root `bb.edn` loads | `bb tasks` | List of tasks (e.g., `component`, `base`, `project`, `build-js`) |

**Setup**:

```bash
# macOS
brew install babashka

# Linux / other platforms: https://github.com/babashka/babashka#installation
# Docker
docker run -it clojure:temurin-17-lein-latest bash -c "curl -sL https://github.com/babashka/babashka/releases/download/v1.3.180/bb-1.3.180-linux-amd64.tar.gz | tar xz && ./bb --version"
```

### 4. Real Node.js (NOT Bun Shim)

**Critical Trap**: On macOS with Bun installed, `which node` and `node --version` may point to a Bun shim (`/opt/homebrew/bin/node`), not real Node. This breaks `bb build-js`.

| Prerequisite | Requirement | Verification Command | Expected Output |
|---|---|---|---|
| Real Node.js | 18+ (LTS) | `which node` | `/usr/local/bin/node` or `/opt/homebrew/opt/node/bin/node` (NOT `/opt/homebrew/bin/node` if Bun is installed) |
| `node --version` | 18+ | `node --version` | `v18.x.x` or higher (must actually run Node, not Bun) |
| `@cljs-oss/module-deps` | ^1.1.1 in `projects/behave/package.json` | `ls -la projects/behave/node_modules/@cljs-oss/module-deps 2>/dev/null && echo OK` | Directory exists; OK printed |

**Detect the Bun-shim trap**:

```bash
# Both of these should point to the SAME binary
which node
file $(which node)

# If it shows "Mach-O universal binary" → real Node
# If it shows "script text" or "ASCII" → Bun shim (PROBLEM)

# Real Node prints version with v prefix
node --version  # v18.16.0 (real) vs /opt/homebrew/bin/node: command not found (shim)
```

**Fix**:

```bash
# If you have Bun shim problem:

# Option A: Reinstall Node without Bun interference
brew uninstall node bun
brew install node  # Installs real Node at /opt/homebrew/opt/node/bin/node

# Option B: Alias PATH before build
export PATH="/usr/local/bin:/opt/homebrew/opt/node/bin:$PATH"

# Verify:
which node  # Must now point to real Node
node --version
npm --version  # npm comes with real Node
```

### 5. npm + @cljs-oss/module-deps

**Requirement**: npm (comes with Node.js) + dependency installed

| Prerequisite | Requirement | Verification Command | Expected Output |
|---|---|---|---|
| npm installed | Shipped with Node.js | `npm --version` | `x.y.z` |
| Dependencies installed in behave | `projects/behave/package.json` locked | `cd projects/behave && npm ls @cljs-oss/module-deps` | `@cljs-oss/module-deps@1.1.1` (or ^1.1.1) listed; no errors |

**Setup**:

```bash
cd projects/behave
npm install  # Installs @cljs-oss/module-deps (declared in package.json)

# Verify
npm ls @cljs-oss/module-deps
```

### 6. Git Submodules

**Requirement**: Two critical submodules; `rj-rust-port` branch for behave-lib

| Submodule | Path | Branch | Verification Command | Expected Output |
|---|---|---|---|---|
| Behave C++ | `behave-lib/behave-mirror` | `rj-rust-port` | `cd behave-lib/behave-mirror && git branch` | `* rj-rust-port` (branch checked out) |
| Help/docs | `bases/behave-docs` | (any; linked to repo) | `ls bases/behave-docs/XHTML_Output/Resources/ \| head` | Image dirs, HTML files present |

**Setup**:

```bash
# Clone with submodules
git clone --recurse-submodules git@github.com:firelab/behave-app.git

# Or init after clone
git submodule update --init --remote

# Verify both are present
ls -d behave-lib/behave-mirror bases/behave-docs

# Check behave-mirror branch
cd behave-lib/behave-mirror && git branch
# Should show: * rj-rust-port
```

**Why `rj-rust-port`?** The C++ code on this branch compiles to WASM via Emscripten. Main branch may not have Emscripten-compatible builds.

### 7. PostgreSQL

**Requirement**: PostgreSQL 12+, running on localhost:5432, with `datomic` user/database

| Prerequisite | Requirement | Verification Command | Expected Output |
|---|---|---|---|
| PostgreSQL running | 12+ on port 5432 | `psql --version` | `psql (PostgreSQL) 12.x` or higher |
| Datomic DB exists | Created by SQL scripts | `psql -U postgres -l \| grep datomic` | `datomic \| datomic \| ...` line present |
| Datomic user created | With password `datomic` | `psql -U datomic -d datomic -c "SELECT 1"` | `1` returned (no auth error) |

**Setup** (from `arch/01_getting_started.org`):

```bash
# macOS
brew install postgresql@15  # Or any 12+
brew services start postgresql@15

# Ubuntu/Debian
apt-get install postgresql-12 postgresql-contrib

# Create Datomic DB and user (run from /path/to/behave-app)
cd bases/datomic_store/sql/
psql -U postgres -f 01_setup.sql      # Creates DB and datomic user
psql -U datomic datomic -f 02_tables.sql  # Sets up KV table
```

### 8. Datomic Pro

**Requirement**: Datomic Pro 1.0.7075 at `~/.datomic/current`

| Prerequisite | Requirement | Verification Command | Expected Output |
|---|---|---|---|
| Datomic Pro installed | 1.0.7075 | `ls -l ~/.datomic/current && ls ~/.datomic/current/bin/transactor` | Symlink to version dir; `transactor` binary present |
| Transactor binary | Executable | `~/.datomic/current/bin/transactor --version` | Version info or help text (no error) |

**Setup**:

```bash
mkdir -p ~/.datomic
cd ~/.datomic

# Download (requires Datomic Pro license; community edition NOT suitable for dev/prod)
curl -O https://datomic-pro-downloads.s3.amazonaws.com/1.0.7075/datomic-pro-1.0.7075.zip

unzip *.zip
ln -s $PWD/datomic-pro-1.0.7075 $PWD/current

# Add to PATH (optional, but recommended)
echo 'export PATH="$HOME/.datomic/current/bin:$PATH"' >> ~/.bashrc  # Or ~/.zshrc
source ~/.bashrc
```

**Verify setup**:

```bash
bb transactor --help  # (bb.edn expects this at ~/.datomic/current/bin/transactor)
```

### 9. WASM Toolchain (Nix + Emscripten)

**Requirement**: Nix + Emscripten for building C++ → WASM. **BUT**: Skip this if C++ code unchanged (WASM `.js` + `.wasm` files are checked in).

| Prerequisite | Requirement | Verification Command | Expected Output |
|---|---|---|---|
| Nix installed (optional) | For WASM build only | `nix --version` | `nix (Nix) x.y.z` or error OK if you skip WASM builds |
| Behave-min WASM checked in | Fallback for dev/production | `ls -la projects/behave/resources/public/js/behave-min.{js,wasm}` | Both files present, non-empty |
| Flake.nix present | At `behave-lib/flake.nix` | `cat behave-lib/flake.nix \| grep "description\|emscripten"` | Flake description and emscripten ref found |

**When to skip WASM build**:
- You are NOT modifying C++ code in `behave-lib/behave-mirror/`
- You are NOT changing units, enums, or solver outputs
- Files `projects/behave/resources/public/js/behave-min.{js,wasm}` are present and up-to-date

**When you MUST build WASM**:
- C++ sources in `behave-lib/behave-mirror/src/behave/` changed
- Enums in `behave-lib/include/cpp/emscripten/enums.cpp` changed
- `make install` in `behave-lib/` must run successfully

**Setup** (only if building WASM):

```bash
# Install Nix (single-user or multi-user; follow nix.dev instructions)
sh <(curl -L https://nixos.org/nix/install)

# Enter Nix dev shell (auto-loads Emscripten, CMake, etc.)
cd behave-lib
nix flake update  # Optional: update lockfile
nix develop --impure
# Now in shell:
make install      # Compiles C++ → WASM, copies to projects/behave/resources/public/js/

# Exit shell
exit
```

---

## Build Commands and Working Directories

All commands assume you are in `/path/to/behave-app/` (repo root) unless stated otherwise.

### Compile ClojureScript → JavaScript

**When**: Every time ClojureScript changes. Needed before running the app or building a JAR.

**Command**:

```bash
cd projects/behave
bb build-js
```

**What it does**:
- Runs `clojure -M:compile-cljs compile-prod.cljs.edn` (defined in `projects/behave/deps.edn`)
- Invokes `behave.compile-cljs/-main` (src/clj/behave/compile_cljs.clj)
- Calls `cljs.main` to compile with externs (`resources/behave_externs.js`, `resources/katex_externs.js`)
- Outputs optimized JS to `projects/behave/target/cljsbuild/prod/` (advanced compilation)
- Produces `behave.min.js` (and source maps if enabled)

**Expected output**:

```
[plenty of compiler warnings OK]
... Build complete in NNN ms
```

**Troubleshooting**:

| Problem | Cause | Fix |
|---|---|---|
| `'node' command not found` or hangs | Bun shim at `which node` | See "Real Node.js" prerequisite above |
| `@cljs-oss/module-deps not found` | npm dependencies missing | `cd projects/behave && npm install` |
| `ClassNotFoundException: cljs.main` | ClojureScript not in classpath | Verify `projects/behave/deps.edn` has `org.clojure/clojurescript` |
| Externs errors in output | Missing/stale externs files | Check `resources/behave_externs.js` and `resources/katex_externs.js` are present |

### Build Uberjar (Production JAR)

**When**: Building a runnable `.jar` for deployment (server or standalone desktop).

**Command**:

```bash
cd projects/behave
bb uber
```

**What it does**:
- Runs `clojure -X:build-uberjar` (defined in `projects/behave/deps.edn` lines 93–96)
- Invokes `packaging/build-uberjar` (src/clj/packaging.clj)
- Packs all dependencies + compiled code into a single JAR
- Outputs to `projects/behave/target/behave7-YYYY.MM.DD-HHMM.jar`

**Prerequisites**:
- `bb build-js` must have run successfully first (JS output in `target/cljsbuild/prod/`)
- `resources/config.edn` configured (see `behave-run-and-operate` for detail)
- Version in `resources/version.edn` and `conveyor.base.conf` synchronized (watch for drift)

**Expected output**:

```
[warnings OK]
... Building uberjar...
Created: target/behave7-2026.07.06-0034.jar (XXX MB)
```

**Troubleshooting**:

| Problem | Cause | Fix |
|---|---|---|
| JAR too small (<50 MB) | JS not compiled | Run `bb build-js` first |
| Version mismatch warnings | `version.edn` ≠ `conveyor.base.conf` | Edit one or both to match; see "Version Drift" section below |
| UnsatisfiedLinkError at runtime | JCEF native binary missing (macOS/Windows) | Use Conveyor for native packaging; or use web server mode |

### Build WASM (C++ → JavaScript)

**When**: C++ code in `behave-lib/behave-mirror` or enums changed.

**Command** (from `behave-lib/`):

```bash
cd behave-lib
nix develop --impure  # Enter Nix dev shell
make install          # Compiles C++ → WASM, copies to projects/behave/resources/public/js/
exit                  # Exit Nix shell
```

**What it does**:
- CMake configures the build
- Emscripten compiles C++ classes (SIGSurface, SIGCrown, etc.) to WASM
- Generates IDL bindings for ClojureScript
- Copies `behave-min.wasm` and `behave-min.js` to `projects/behave/resources/public/js/`

**Prerequisites**:
- Nix installed and working (`nix --version`)
- `flake.nix` at `behave-lib/`
- `behave-lib/behave-mirror` checked out on `rj-rust-port` branch

**Expected output**:

```
Environment ready!
EM_CACHE: /path/to/behave-lib/.em_cache
WEBIDL: ...
[cmake output]
[make output]
[install copying files]
```

**Key environment variables** (set by `behave-lib/flake.nix`):

| Variable | Value | Purpose |
|---|---|---|
| `EM_CACHE` | `$PWD/.em_cache` (inside behave-lib) | Emscripten compiler cache; speeds up rebuilds |
| `WEBIDL` | Path to `webidl_binder.py` in Emscripten | IDL → WASM glue code generator |

**Troubleshooting**:

| Problem | Cause | Fix |
|---|---|---|
| `nix: command not found` | Nix not installed | Install from nix.dev; or skip WASM build if `.wasm` already checked in |
| Emscripten not found in `nix develop` | Flake.nix broken | Check `behave-lib/flake.nix` has `emscripten` in `buildInputs` |
| WASM file ~0 bytes | Build failed silently | Re-run with error output: `make install 2>&1 \| tail -50` |

---

## Known Traps and Gotchas

### Version Drift (Hard to Debug)

**The problem**: Two places hardcode version: `conveyor.base.conf` (line 29) and `projects/behave/resources/version.edn`.

- `conveyor.base.conf` line 29: `version = 7.1.4`
- `resources/version.edn`: `{:version "v7.1.4"}`

If they diverge, release workflows may produce confusing artifacts or fail silently.

**Detection**:

```bash
grep "^  version = " projects/behave/conveyor.base.conf
cat projects/behave/resources/version.edn
```

**Fix**: Keep them in sync. The orchestrator's release script sets `resources/version.edn` from a tag; manual builds must edit both.

### Externs Not Regenerated

**The problem**: `behave_externs.js` and `katex_externs.js` are hand-written hints for the ClojureScript advanced compiler. If you add a new external library (e.g., a JavaScript library), externs must be updated, or advanced compilation will mangle library names → runtime errors ("foo is not a function").

**Detection**:

```bash
# Check if externs files exist and have content
wc -l projects/behave/resources/*externs.js

# Check if they're included in compile-prod.cljs.edn
grep -i externs projects/behave/compile-prod.cljs.edn
```

**Fix**: Add missing externs. See `arch/02_building_wasm.org` for Hatchet + externs workflow.

### Submodule Stuck on Wrong Branch

**The problem**: `behave-lib/behave-mirror` MUST be on `rj-rust-port` to build WASM. If it drifts to `master`, Emscripten build will fail or produce wrong output.

**Detection**:

```bash
cd behave-lib/behave-mirror
git branch
# Should show:   * rj-rust-port
# NOT: * master
```

**Fix**:

```bash
cd behave-lib/behave-mirror
git checkout rj-rust-port
git pull origin rj-rust-port
```

### EM_CACHE Not Set (Emscripten Rebuild Penalty)

**The problem**: Without `EM_CACHE` set, Emscripten recompiles system libraries from scratch every time. Builds take 10+ minutes instead of seconds.

**Detection**:

```bash
echo $EM_CACHE  # Should print path, not empty

# Inside nix develop:
env | grep EM_CACHE  # Should be set
```

**Fix**: The flake.nix sets it automatically. If building manually (outside Nix), export before make:

```bash
export EM_CACHE=$PWD/.em_cache
make install
```

### Bun Shim Node at PATH

**The problem**: Bun installs a shim at `/opt/homebrew/bin/node`. If Bun is in PATH before real Node, `bb build-js` will fail with cryptic errors ("module not found", "undefined is not a function", etc.).

**Detection**:

```bash
which node          # If /opt/homebrew/bin/node → PROBLEM
file $(which node)  # If "ASCII text" or "script" → Bun shim

# Test with both:
node --version      # Real Node prints v18.16.0; shim fails
npm --version       # Real Node + npm; shim fails
```

**Fix**:

```bash
# Option 1: Reinstall Node properly
brew uninstall bun node
brew install node

# Option 2: PATH precedence
export PATH="/opt/homebrew/opt/node/bin:/usr/local/bin:$PATH"

# Verify (all three should work)
which node
node --version
npm --version
```

---

## VERIFY-YOUR-ENV Checklist

Run this checklist on a new machine before attempting any build. Each command should succeed.

| # | Check | Command | Expected | Critical? |
|---|---|---|---|---|
| 1 | JDK 17 | `java -version \| grep -o "openjdk.*17"` | `openjdk version "17.x.x"` | YES |
| 2 | Clojure CLI | `clojure --version \| grep -o "1\.11"` | `Clojure CLI version 1.11.x` | YES |
| 3 | Babashka | `bb --version \| grep -o "v1\."` | `babashka v1.x.x` | YES |
| 4 | Real Node (not Bun) | `file $(which node) \| grep -i "mach-o\|elf"` | Mach-O or ELF (binary, not text) | YES |
| 5 | Node version 18+ | `node --version \| grep -o "v1[8-9]\|v[2-9]"` | `v18.x` or higher | YES |
| 6 | npm installed | `npm --version` | `x.y.z` | YES |
| 7 | @cljs-oss/module-deps | `ls projects/behave/node_modules/@cljs-oss/module-deps` | Directory exists | YES |
| 8 | Behave-mirror branch | `cd behave-lib/behave-mirror && git branch \| grep "rj-rust-port"` | `* rj-rust-port` | YES (if building WASM) |
| 9 | Behave-docs submodule | `ls bases/behave-docs/XHTML_Output \| head -1` | Directory listing (not error) | NO (web mode only) |
| 10 | PostgreSQL running | `psql -U datomic -d datomic -c "SELECT 1" 2>&1 \| grep -c "1"` | `1` | YES (if CMS needed) |
| 11 | Datomic Pro installed | `ls ~/.datomic/current/bin/transactor` | File exists | YES (if CMS needed) |
| 12 | Behave-min WASM checked in | `ls -la projects/behave/resources/public/js/behave-min.wasm \| awk '{print $5 > 100000 ? "OK" : "TOO_SMALL"}'` | `OK` (file >100KB) | YES |
| 13 | Externs files present | `test -f projects/behave/resources/behave_externs.js && test -f projects/behave/resources/katex_externs.js && echo OK` | `OK` | YES |
| 14 | Nix available (optional) | `nix --version` | `nix (Nix) x.y.z` | NO (only if building WASM) |
| 15 | bb.edn tasks load | `bb tasks \| grep build-js` | `build-js` listed | YES |

**Quick smoke test** (after checklist passes):

```bash
cd /path/to/behave-app

# 1. Verify root deps load
clojure -Spath > /dev/null && echo "✓ deps.edn OK"

# 2. Verify projects/behave build alias exists
cd projects/behave && clojure -Spath -M:compile-cljs > /dev/null && echo "✓ compile-cljs alias OK"

# 3. Verify bb tasks
cd /path/to/behave-app && bb tasks | grep -q build-js && echo "✓ bb tasks OK"

# 4. If Postgres running: verify Datomic
psql -U datomic -d datomic -c "SELECT 1" > /dev/null 2>&1 && echo "✓ Postgres + Datomic OK"
```

---

## Typical First-Time Workflow

```bash
# 1. Clone repo with submodules
git clone --recurse-submodules git@github.com:firelab/behave-app.git
cd behave-app

# 2. Run the checklist above
# ... (fix any failures)

# 3. Verify submodules
cd behave-lib/behave-mirror && git checkout rj-rust-port && cd ../..

# 4. Compile ClojureScript
cd projects/behave
bb build-js

# 5. (Optional) If you edited C++ code, rebuild WASM
cd behave-lib
nix develop --impure
make install
exit
cd ../projects/behave

# 6. (Optional) Test with figwheel dev mode
# See behave-run-and-operate for dev setup

# 7. Build uberjar (production)
bb uber

# 8. Verify JAR exists
ls -lh target/behave7-*.jar
```

---

## Troubleshooting: Build Fails Silently

If `bb build-js` or `bb uber` runs but produces no output and no error, try:

```bash
# 1. Enable verbose output
cd projects/behave
clojure -M:compile-cljs compile-prod.cljs.edn 2>&1 | tail -50

# 2. Check if process is hanging on I/O
# (Open another terminal, check if node is running)
ps aux | grep node

# 3. Check disk space
df -h /

# 4. Look for stale compilation artifacts
rm -rf target/cljsbuild/
bb build-js  # Retry
```

---

## Provisioning a CI/CD Environment

For GitHub Actions or similar CI, use Nix for reproducibility:

```yaml
# Example: .github/workflows/build.yml (like jar-builder.yml)
- name: Install Nix
  uses: DeterminateSystems/nix-installer-action@main

- name: Build from Nix dev environment
  env:
    NIXPKGS_ALLOW_UNFREE: 1  # Needed for JCEF
  run: |
    cd projects/behave
    nix develop --impure --command bash -c "
      bb build-js
      bb uber
    "
```

This guarantees reproducible builds across machines (no reliance on local Java, Node, etc.).

---

## Provenance and Maintenance

**Last verified: 2026-07-06**

| Fact | Verification Command | Expected | Re-check When |
|---|---|---|---|
| Java requirement | `grep "java-version: '17'" .github/workflows/jar-only.yml` | `java-version: '17'` found | CI workflow changes |
| Clojure version | `grep "org.clojure/clojure" deps.edn` | `:mvn/version "1.11.1"` | deps.edn changes |
| ClojureScript version | `grep "org.clojure/clojurescript" projects/behave/deps.edn` | `:mvn/version "1.11.54"` | ClojureScript updates |
| Behave-mirror branch | `cd behave-lib/behave-mirror && git rev-parse --abbrev-ref HEAD` | `rj-rust-port` | After `git submodule update` |
| WASM files checked in | `stat projects/behave/resources/public/js/behave-min.wasm \| grep Size` | Size > 100KB | After WASM rebuild |
| Version in conveyor | `grep "^  version = " projects/behave/conveyor.base.conf` | `version = 7.1.4` | Before release |
| Version in resources | `cat projects/behave/resources/version.edn` | `{:version "v7.1.4"}` | Before release |
| Datomic version | `grep "com.datomic/peer" deps.edn` | `:mvn/version "1.0.7075"` | Datomic updates |
| Datomic config path | `grep "datomic-sql.properties" bb.edn` | Path to config present | If config location changes |
| Server default port | `grep "http-port" projects/behave/resources/config.edn` | `:http-port 9101` | If port changes |
| Postgres min version | `grep "Postgres\|>=12" arch/01_getting_started.org` | Requirement documented | Postgres upgrades |
| Node shim detection | `file $(which node) 2>&1` | "Mach-O" or "ELF" (not "text") | When Bun/Node updated |
| @cljs-oss/module-deps version | `grep "@cljs-oss/module-deps" projects/behave/package.json` | `"@cljs-oss/module-deps": "^1.1.1"` | package.json changes |
| Externs files | `wc -l projects/behave/resources/*externs.js` | Both files >0 lines | After solver/UI library changes |

---

## Related Skills

- **`behave-run-and-operate`**: Running dev mode (figwheel), server, desktop app, CMS — config flags and ports
- **`behave-validation-and-qa`**: Test suites, golden data, how to run and debug tests
- **`behave-vms-variable-pipeline`**: Adding new solver variables, regenerating WASM bindings with Hatchet
- **`behave-change-control`**: PR/commit conventions, non-negotiables around WASM and migrations
