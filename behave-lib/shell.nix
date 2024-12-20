let
  nixpkgs = fetchTarball "https://github.com/NixOS/nixpkgs/tarball/nixos-24.11";
  pkgs = import nixpkgs { config = {}; overlays = []; };
in

pkgs.mkShellNoCC {
  packages = with pkgs; [
    # Docs
    doxygen

    # Building/Tests
    gnumake
    cmake
    llvm
    clang
    emscripten
    fd
    python3
  ];

  shellHook = ''
  export EM_CACHE="$PWD/.em_cache"
  export WEBIDL=$(fd webidl_binder /nix/store | head -n1)
  '';
}
