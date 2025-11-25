{ pkgs, ... }:

let 
  dyn_libs = with pkgs; [
    libxkbcommon
    wayland
    vulkan-loader
  ]; # Required for winit/wgpu windowing
in {
  packages = dyn_libs ++ [ 
    pkgs.rust-analyzer
    pkgs.perf # For flamegraph
  ];

  languages.rust = {
    enable = true;
    channel = "stable";
    # We need to add the language server manually so we can add it to the path correctly
    components = [ "rustc" "cargo" "clippy" "rustfmt" ];
    rustflags = "-C link-arg=-Wl,-rpath,${pkgs.lib.makeLibraryPath dyn_libs} ";
  };

  # Exposes path for nvim lsp
  env.RUST_ANALYZER = "${pkgs.rust-analyzer}/bin/rust-analyzer";
}
