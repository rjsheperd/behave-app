{
  description = "WindNinja build environment for the W7 Amdahl spike";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # W7 depends on OpenMP: every phase timer in ninja.cpp:578-595 sits
        # inside `#ifdef _OPENMP`, so a build without it reports nothing at
        # all. Apple clang ships no libomp, so on Darwin we must bring our
        # own and point the compiler at it explicitly.
        openmp = pkgs.llvmPackages.openmp;

        deps = with pkgs; [
          gdal          # required; the whole reason for this flake
          boost         # date_time, program_options, test
          netcdf
          proj
          sqlite
          libxml2
          zlib
          curl
          openssl
          shapelib
        ];

        tools = with pkgs; [ cmake pkg-config gnumake git ];

        # WindNinja's non-MSVC branch calls `find_package(netCDF REQUIRED)`
        # (CMakeLists.txt:121) and then uses the `netCDF::netcdf` imported
        # target. Unlike shapelib immediately above it, there is no
        # pkg-config fallback — so a netCDF *CMake config package* is
        # mandatory. nixpkgs' netcdf ships pkg-config only, so we synthesise
        # the config package here rather than patching upstream.
        netcdfCMakeShim =
          let
            ext = pkgs.stdenv.hostPlatform.extensions.sharedLibrary;
          in
          pkgs.runCommand "netcdf-cmake-config" { } ''
            mkdir -p $out/lib/cmake/netCDF
            cat > $out/lib/cmake/netCDF/netCDFConfig.cmake <<'EOF'
            if(NOT TARGET netCDF::netcdf)
              add_library(netCDF::netcdf UNKNOWN IMPORTED)
              set_target_properties(netCDF::netcdf PROPERTIES
                IMPORTED_LOCATION "@LIB@"
                INTERFACE_INCLUDE_DIRECTORIES "@INC@")
            endif()
            set(netCDF_FOUND TRUE)
            set(netCDF_LIBRARIES netCDF::netcdf)
            set(netCDF_INCLUDE_DIR "@INC@")
            EOF
            substituteInPlace $out/lib/cmake/netCDF/netCDFConfig.cmake \
              --replace '@LIB@' '${pkgs.netcdf}/lib/libnetcdf${ext}' \
              --replace '@INC@' '${pkgs.netcdf}/include'
          '';
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = tools;
          buildInputs = deps ++ [ openmp ];

          # nixpkgs enables `format` hardening by default, which adds
          # -Werror=format-security. That fails WindNinja's build at
          # pointInitialization.cpp:840:
          #
          #   CPLError(CE_Warning, CPLE_AppDefined, oWarnString.c_str());
          #
          # passing a runtime-built std::string as the *format* argument. It is
          # a real latent bug — a weather-station name containing "%s" would
          # make CPLError consume varargs that were never passed — and the
          # station name comes from a user-supplied shapefile. Reported in
          # README.org; disabled here so the spike can proceed without
          # patching upstream.
          hardeningDisable = [ "format" ];

          shellHook = ''
            # Make OpenMP discoverable to CMake's FindOpenMP, which on
            # Darwin/clang does not find libomp without help.
            export OpenMP_ROOT="${openmp}"
            export CPPFLAGS="-I${openmp.dev}/include $CPPFLAGS"
            export LDFLAGS="-L${openmp}/lib $LDFLAGS"
            export LIBRARY_PATH="${openmp}/lib:$LIBRARY_PATH"

            # GDAL needs its data directory at runtime; this is the exact
            # failure mode W2 predicts for the in-memory API path.
            export GDAL_DATA="${pkgs.gdal}/share/gdal"
            export PROJ_DATA="${pkgs.proj}/share/proj"
            export PROJ_LIB="$PROJ_DATA"

            # WindNinja also demands its OWN runtime data directory, and
            # throws std::runtime_error("no valid WINDNINJA_DATA found") on
            # startup without it — a second filesystem dependency beyond
            # GDAL_DATA that spike W2 must account for (31 MB in-tree).
            export WINDNINJA_DATA="$PWD/../../windninja/data"

            # Point find_package(netCDF) at the synthesised config package.
            export netCDF_DIR="${netcdfCMakeShim}/lib/cmake/netCDF"
            export CMAKE_PREFIX_PATH="${netcdfCMakeShim}:$CMAKE_PREFIX_PATH"

            # shapelib has no CMake config in nixpkgs either, so WindNinja
            # falls through to its pkg-config branch. That sets
            # SHAPELIB_LIBRARY=PkgConfig::SHAPELIB for libninja, but
            # src/cli/CMakeLists.txt:53 hardcodes the *other* branch's target
            # name (shapelib::shp) instead of using ${"$"}{SHAPELIB_LIBRARY} —
            # so the CLI link finds no DBF*/SHP* symbols. Export the explicit
            # link flags as a workaround; see README.org.
            export SHAPELIB_LDFLAGS="$(pkg-config --libs shapelib)"

            echo "WindNinja W7 environment"
            echo "  gdal    $(gdal-config --version 2>/dev/null || echo '?')"
            echo "  cmake   $(cmake --version | head -1 | awk '{print $3}')"
            echo "  openmp  ${openmp}"
            echo "  GDAL_DATA=$GDAL_DATA"
            echo
            echo "Build with:  ./run.sh <dem.tif>"
          '';
        };
      }
    );
}
