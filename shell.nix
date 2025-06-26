{ pkgs ? import <nixpkgs> {} }:

let
  dlopenLibs = with pkgs; [
    libxkbcommon
    wayland
    vulkan-loader
  ];
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
  ] ++ dlopenLibs;

  shellHook = '' export RUSTFLAGS="-C link-arg=-Wl,-rpath,${pkgs.lib.makeLibraryPath dlopenLibs}" '';
}

