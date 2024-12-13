{
  description = "A Nix Flake for compiling a library with CMake, Emscripten, and LLVM/Clang";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-22.11";
    flake-utils.url = "github:numtide/flake-utils";
  }

  outputs = { self, nixpkgs, flake-utils }: 
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = import nixpkgs { inherit system; };
      in {
        packages.${system} = pkgs.stdenv.mkDerivation {
          pname = "cmake-emscripten-lib";
          version = "1.0.0";

          src = ./.;

          buildInputs = [
            pkgs.cmake
            pkgs.llvmPackages.clang
            pkgs.emscripten
            pkgs.gnumake
          ];

          cmakeFlags = [
            "-DCMAKE_TOOLCHAIN_FILE=${pkgs.emscripten}/share/emscripten/cmake/Modules/Platform/Emscripten.cmake"
          ];

          buildPhase = ''
          make install
        '';
        };
      };
    );
}
