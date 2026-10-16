#!/usr/bin/env bash
# W7 — measure WindNinja's phase breakdown at the three mesh resolutions.
#
# STATUS: configure/build steps are UNVALIDATED on this machine — the local
# Homebrew GDAL is broken (see README.org "Blocker"). Everything below is
# written from WindNinja's own build docs and CMake options, not from a
# successful run. Expect to iterate on the first attempt.
#
#   ./run.sh /path/to/dem.tif [builddir]
#
set -euo pipefail

DEM="${1:?usage: run.sh <dem.tif> [builddir]}"
BUILD="${2:-$PWD/build-w7}"
SRC="$(cd "$(dirname "$0")/../../windninja" && pwd)"
OUT="$PWD/w7-results"
mkdir -p "$OUT"

# --- configure -------------------------------------------------------------
# Minimal core per spike W1: no GUI, no OpenFOAM, no network features.
# OpenMP must stay ON — the phase timers are inside #ifdef _OPENMP, so a
# non-OpenMP build produces no measurements at all (ninja.cpp:578).
if [ ! -f "$BUILD/CMakeCache.txt" ]; then
  cmake -S "$SRC" -B "$BUILD" \
    -DCMAKE_BUILD_TYPE=Release \
    -DNINJA_GUI=OFF \
    -DNINJA_CLI=ON \
    -DNINJAFOAM=OFF \
    -DOPENMP_SUPPORT=ON \
    -DWITH_NOMADS_SUPPORT=OFF \
    -DPHONE_HOME_QUERIES=OFF \
    -DDISABLE_THREDDS_UPDATE=ON \
    -DWITH_LCP_CLIENT=OFF \
    -DENABLE_GMTED=OFF \
    -DMAKE_DOCS=OFF
fi
cmake --build "$BUILD" -j"$(sysctl -n hw.ncpu 2>/dev/null || nproc)"

CLI="$BUILD/src/cli/WindNinja_cli"
[ -x "$CLI" ] || { echo "WindNinja_cli not found at $CLI"; exit 1; }

# --- run matrix ------------------------------------------------------------
# mesh_choice maps to the targets in mesh.cpp:56-58 —
#   coarse=4000, medium=10000, fine=20000 horizontal cells.
# Domain-average initialisation keeps the outer matching loop single-pass, so
# the stock timers are trustworthy (see README.org F2). Add a point-init run
# separately once accumulate-timers.patch is applied.
for MESH in coarse medium fine; do
  LOG="$OUT/run-$MESH.log"
  echo "=== $MESH ==="
  "$CLI" \
    --elevation_file "$DEM" \
    --initialization_method domainAverageInitialization \
    --input_speed 10 --input_speed_units mps \
    --input_direction 270 \
    --input_wind_height 10 --units_input_wind_height m \
    --output_wind_height 10 --units_output_wind_height m \
    --vegetation grass \
    --mesh_choice "$MESH" \
    --num_threads "$(sysctl -n hw.ncpu 2>/dev/null || nproc)" \
    --write_goog_output false \
    --write_shapefile_output false \
    --write_ascii_output true \
    --output_path "$OUT/$MESH" \
    2>&1 | tee "$LOG"
done

# --- report ----------------------------------------------------------------
bun run "$(dirname "$0")/parse_timers.mjs" "$OUT"/run-*.log
