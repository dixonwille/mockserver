{
  description = "Mockserver - A Lua-powered mock server for API development";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      perSystem = { config, self', inputs', pkgs, system, ... }:
        let
          # Apply rust-overlay to get latest stable Rust
          rustPkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };

          # Rust toolchain with components we need
          rustToolchain = rustPkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" "clippy" "llvm-tools-preview" ];
          };
        in
        {
          # Development shell
          devShells.default = pkgs.mkShell {
            name = "mockserver-dev";

            nativeBuildInputs = [
              pkgs.pkg-config
              rustToolchain
            ];

            # Darwin needs Apple SDK for system frameworks (CoreFoundation, etc.)
            buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.apple-sdk_15
            ];

            packages = with pkgs; [
              # Formatters
              rustfmt
              taplo

              # Language servers
              lua-language-server

              # Development tools
              cargo-watch
              cargo-outdated
              cargo-nextest
              cargo-deny
              cargo-about
              cargo-llvm-cov

              # Licensing
              reuse
            ];

            # Environment variables
            RUST_BACKTRACE = "1";
            RUST_LOG = "mockserver=debug,info";

            shellHook = ''
              # Add project scripts to PATH
              export PATH="$PWD/scripts:$PATH"

              echo "🦀 Mockserver development shell"
              echo ""
              echo "Available commands:"
              echo "  cargo build       - Build the project"
              echo "  cargo test        - Run tests"
              echo "  cargo nextest run - Run tests with nextest"
              echo "  cargo watch       - Auto-rebuild on changes"
              echo "  cargo clippy      - Run linter"
              echo "  cargo fmt         - Format code"
              echo "  deny-check        - Check dependent crates"
              echo ""
              echo "Code coverage:"
              echo "  cargo llvm-cov              - Run tests with coverage"
              echo "  cargo llvm-cov --html       - Generate HTML report"
              echo "  cargo llvm-cov --open       - Generate and open HTML report"
              echo "  cargo llvm-cov --lcov       - Generate lcov format"
              echo ""
              echo "Licensing:"
              echo "  reuse lint              - Check license compliance"
              echo "  reuse-header <file>     - Add license header to file"
              echo "  third-party-licenses    - Generate THIRD_PARTY_LICENSES.html"
              echo ""
              echo "Rust: $(rustc --version)"
              echo "Cargo: $(cargo --version)"
            '';
          };
        };
    };
}
