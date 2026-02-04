# Contributing to Mockserver

Thank you for your interest in contributing to Mockserver. This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- **Rust toolchain**: Rust 2024 edition (latest stable)
- **Optional**: Nix package manager with flakes enabled

### Using Nix (Recommended)

The project includes a Nix flake that provides a complete development environment with all necessary tools.

**With direnv (automatic):**

```bash
# Install direnv if you haven't already
# Allow the .envrc file (one-time setup)
direnv allow

# The environment loads automatically when you cd into the project
cd mockserver
```

**Without direnv (manual):**

```bash
nix develop
```

The Nix environment includes:

- Latest stable Rust toolchain with rust-analyzer, clippy, and llvm-tools
- Formatters: rustfmt, taplo (TOML)
- Development tools: cargo-watch, cargo-nextest, cargo-deny, cargo-about, cargo-llvm-cov
- Language servers: lua-language-server
- Licensing tools: reuse

### Without Nix

If you prefer not to use Nix:

1. Install Rust via [rustup](https://rustup.rs/):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Install required components:

   ```bash
   rustup component add clippy rustfmt
   ```

3. Optional tools (for full development workflow):

   ```bash
   cargo install cargo-watch cargo-nextest cargo-deny cargo-about cargo-llvm-cov
   pip install reuse  # For license compliance checking
   ```

### Building the Project

```bash
# Debug build
cargo build

# Release build (with LTO, optimized)
cargo build --release

# Watch mode (auto-rebuild on changes)
cargo watch -x build
```

## Running Tests

```bash
# Run all tests
cargo test

# Run tests with nextest (faster, better output)
cargo nextest run

# Run tests with coverage
cargo llvm-cov
cargo llvm-cov --html       # Generate HTML report
cargo llvm-cov --open       # Generate and open HTML report

# Run specific test
cargo test test_name
```

### Linting and Formatting

```bash
# Check formatting
cargo fmt -- --check

# Apply formatting
cargo fmt

# Run clippy lints
cargo clippy -- -D warnings

# Check dependency licenses and security
deny-check
# Or directly: cargo deny check -c .config/deny.toml
```

## Code Style

### Formatting

This project uses the default `rustfmt` configuration. Run `cargo fmt` before committing.

### Clippy Lints

All clippy warnings are treated as errors in CI. Address all warnings before submitting a PR:

```bash
cargo clippy -- -D warnings
```

### Naming Conventions

Follow standard Rust naming conventions:

- `snake_case` for functions, methods, variables, and modules
- `PascalCase` for types, traits, and enum variants
- `SCREAMING_SNAKE_CASE` for constants and statics
- Descriptive names that convey intent

### Documentation

- Add doc comments (`///`) to all public items
- Include examples in doc comments where helpful
- Keep comments concise and focused

## Project Structure

```
src/
  main.rs           # Entry point, CLI argument parsing
  lib.rs            # Library root, re-exports
  config.rs         # Configuration types and loading
  error.rs          # Error types (thiserror)
  templates.rs      # Embedded file templates

  cli/              # CLI subcommands
    mod.rs
    init.rs         # `mockserver init` - initialize mocks directory
    new.rs          # `mockserver new` - create new domain mock
    serve.rs        # `mockserver serve` - start the server
    check.rs        # `mockserver check` - validate Lua scripts

  server/           # Mock server (handles incoming requests)
    mod.rs
    router.rs       # Domain-based request routing
    handler.rs      # Request handling and Lua execution

  api/              # Admin API (request inspection, config)
    mod.rs
    router.rs       # API route definitions
    handlers/       # API endpoint handlers
      health.rs     # Health check endpoint
      requests.rs   # Request query endpoints
      cleanup.rs    # Data cleanup endpoints
      config.rs     # Configuration endpoints

  lua/              # Lua runtime integration
    mod.rs
    manager.rs      # Lua VM pool management
    sandbox.rs      # Sandboxed execution environment
    modules/        # Lua standard library modules
      json.rs       # JSON encoding/decoding
      log.rs        # Logging functions
      delay.rs      # Async delay/sleep
      uuid.rs       # UUID generation
      time.rs       # Time utilities
      fs.rs         # Filesystem access (sandboxed)
      state.rs      # Persistent state storage

  db/               # SQLite database layer
    mod.rs
    migrations.rs   # Schema migrations
    models.rs       # Data models
    queries.rs      # Database queries

  watcher/          # File watching for hot reload
    mod.rs
```

## Making Changes

1. **Fork and clone** the repository:

   ```bash
   git clone https://github.com/YOUR_USERNAME/mockserver.git
   cd mockserver
   ```

2. **Create a feature branch**:

   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **Make your changes** with appropriate tests

4. **Run all checks**:

   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   reuse lint  # Check license compliance
   ```

5. **Commit with a clear message**:

   ```bash
   git commit -m "feat: add support for custom headers"
   ```

6. **Push and open a PR**:

   ```bash
   git push origin feature/your-feature-name
   ```

## Pull Request Guidelines

### PR Title

Use a descriptive title that summarizes the change:

- `feat: add WebSocket support`
- `fix: handle empty request bodies`
- `docs: update Lua scripting guide`
- `refactor: simplify router logic`

### PR Description

- Describe what the PR does and why
- Link to related issues (e.g., "Fixes #123")
- Note any breaking changes
- Include screenshots for UI changes (if applicable)

### Requirements

- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] License compliance passes (`reuse lint`)
- [ ] New features include tests
- [ ] Documentation updated if needed

## License

This project is licensed under **AGPL-3.0-only**.

### REUSE Compliance

This project follows the [REUSE specification](https://reuse.software/) for license compliance. All source files must have proper license headers.

### Adding Headers to New Files

When creating new source files, add the appropriate license header:

**For Rust files:**

```bash
# Using the helper script (requires reuse tool)
reuse-header src/your_new_file.rs
```

This runs:

```bash
reuse annotate --license AGPL-3.0-only --copyright "mockserver contributors" <file>
```

**Manual header for Rust:**

```rust
// SPDX-FileCopyrightText: 2026 mockserver contributors
// SPDX-License-Identifier: AGPL-3.0-only
```

**For non-source files**, the `REUSE.toml` file contains bulk annotations for configuration files, scripts, and other assets that cannot contain comments.

### Checking Compliance

```bash
reuse lint
```

### Third-Party Licenses

To regenerate the third-party license file:

```bash
third-party-licenses
# Or: cargo about generate -c .config/about.toml .config/about.hbs > THIRD_PARTY_LICENSES
```

To check dependency licenses:

```bash
deny-check
# Or: cargo deny check -c .config/deny.toml
```

Allowed licenses for dependencies: MIT, Apache-2.0, Unlicense, BSD-3-Clause, CC0-1.0, ISC, Unicode-3.0, Zlib.

## Questions

If you have questions or need help, feel free to:

- Open an issue for discussion
- Comment on an existing issue or PR
