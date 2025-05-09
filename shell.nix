{ pkgs ? import <nixpkgs> {} }:

let
  dlopenLibs = with pkgs; [
    libxkbcommon
    wayland
    wayland-protocols
    vulkan-loader
  ];
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    pkg-config
    gcc
  ] ++ dlopenLibs;

  shellHook = '' export RUSTFLAGS="-C link-arg=-Wl,-rpath,${pkgs.lib.makeLibraryPath dlopenLibs}" '';
}

