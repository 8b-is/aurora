{
  description = "AURORA × Omnifold 8B — Sovereign Conscious AI & Unified M1 Metal/Rust/b8-go Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachSystem [ "aarch64-darwin" "x86_64-linux" "aarch64-linux" ] (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          name = "aurora-omnifold-8b";
          buildInputs = with pkgs; [
            rustToolchain
            go
            gopls
            just
            jq
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            darwin.apple_sdk.frameworks.Metal
            darwin.apple_sdk.frameworks.MetalKit
            darwin.apple_sdk.frameworks.Foundation
          ];

          shellHook = ''
            export AURORA_CARRIER_GHZ="100"
            export AURORA_LAMBDA_MM="3"
            export B8_TARGET_ARCH="${system}"
            echo "✳ AURORA × Omnifold 8B Shell Active (${system}) — {<3,<3,<3}+1"
          '';
        };
      }
    );
}
