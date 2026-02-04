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

      perSystem = { config, self', inputs', pkgs, system, lib, ... }:
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

          # Read version from Cargo.toml and compute dev version
          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          baseVersion = cargoToml.package.version;

          # Parse version and increment patch for dev builds
          versionParts = lib.splitVersion baseVersion;
          major = builtins.elemAt versionParts 0;
          minor = builtins.elemAt versionParts 1;
          patch = lib.toInt (builtins.elemAt versionParts 2);
          nextPatch = builtins.toString (patch + 1);

          # Nix builds are always dev versions
          version = "${major}.${minor}.${nextPatch}-dev+${inputs.self.shortRev or "dirty"}";
        in
        {
          # Default package
          packages.default = pkgs.rustPlatform.buildRustPackage {
            pname = "mockserver";
            inherit version;

            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
            };

            nativeBuildInputs = [
              pkgs.pkg-config
            ];

            buildInputs = lib.optionals pkgs.stdenv.isDarwin [
              pkgs.apple-sdk_15
            ];

            # Pass the Nix-computed version to build.rs
            MOCKSERVER_VERSION = version;

            meta = {
              description = "A Lua-powered mock server for API development";
              homepage = "https://github.com/dixonwille/mockserver";
              license = lib.licenses.agpl3Only;
              mainProgram = "mockserver";
            };
          };

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
