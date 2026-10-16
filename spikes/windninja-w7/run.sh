#!/usr/bin/env bash
# W7 — measure WindNinja's phase breakdown. Run INSIDE the nix shell:
#
#   nix develop
#   ./run.sh [dem.tif]
#
# With no argument it extracts a DEM from the Blue Mountain LCP sample.
# Exercised end to end; see README.org for results and for the five
# environment gotchas (B1-B5) the flake works around.
set -euo pipefail

: "${WINDNINJA_DATA:?run inside 'nix develop' — WINDNINJA_DATA must be set (B5)}"

HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$HERE/../../windninja"
WORK="${W7_WORK:-${TMPDIR:-/tmp}/w7}"
BUILD="$WORK/build"
mkdir -p "$WORK/out"

# --- DEM -------------------------------------------------------------------
DEM="${1:-$WORK/dem.tif}"
if [ ! -f "$DEM" ]; then
  echo "Extracting elevation (LCP band 1) -> $DEM"
  gdal_translate -q -b 1 -ot Float32 -a_nodata -9999 \
    "$HERE/../../SpatialFOFEM/SampleData/Landscape/BlueMountain_LCP_FCCS.tif" \
    "$DEM"
fi

# --- configure + build -----------------------------------------------------
# OPENMP_SUPPORT must stay ON: every phase timer is inside #ifdef _OPENMP
# (ninja.cpp:578), so a non-OpenMP build reports nothing at all (F1).
if [ ! -f "$BUILD/CMakeCache.txt" ]; then
  cmake -S "$SRC" -B "$BUILD" \
    -DCMAKE_BUILD_TYPE=Release \
    -DNINJA_GUI=OFF -DNINJA_CLI=ON -DNINJAFOAM=OFF \
    -DOPENMP_SUPPORT=ON \
    -DWITH_NOMADS_SUPPORT=OFF -DPHONE_HOME_QUERIES=OFF \
    -DDISABLE_THREDDS_UPDATE=ON -DWITH_LCP_CLIENT=OFF \
    -DENABLE_GMTED=OFF -DMAKE_DOCS=OFF \
    -DCMAKE_IGNORE_PREFIX_PATH=/opt/homebrew \
    -DCMAKE_EXE_LINKER_FLAGS="${SHAPELIB_LDFLAGS:-}"
fi
cmake --build "$BUILD" -j"$(sysctl -n hw.ncpu 2>/dev/null || nproc)"
CLI="$BUILD/src/cli/WindNinja_cli"

# --- run matrix ------------------------------------------------------------
NT="$(sysctl -n hw.ncpu 2>/dev/null || nproc)"
run_one() {  # $1 = label, rest = mesh flags
  local label="$1"; shift
  "$CLI" --elevation_file "$DEM" \
    --initialization_method domainAverageInitialization \
    --input_speed 10 --input_speed_units mps --input_direction 270 \
    --input_wind_height 10 --units_input_wind_height m \
    --output_wind_height 10 --units_output_wind_height m \
    --vegetation grass --num_threads "$NT" \
    --write_goog_output false --write_shapefile_output false \
    --write_ascii_output true --output_path "$WORK/out" \
    "$@" > "$WORK/run-$label.log" 2>&1
  echo "  $label -> $(grep -c 'time was' "$WORK/run-$label.log") timer lines"
}

echo "Running mesh_choice matrix..."
for M in coarse medium fine; do run_one "$M" --mesh_choice "$M"; done
echo "Running explicit mesh_resolution matrix..."
for R in 90 60 30; do
  run_one "res$R" --mesh_resolution "$R" --units_mesh_resolution m
done

bun run "$HERE/parse_timers.mjs" "$WORK"/run-*.log
