# Scribe
# Justfile for common development tasks.

# Default recipe: list available recipes
default:
    @just --list

# --- Build ---

# Build all crates (debug)
build:
    cargo build --all

# Build all crates with release optimizations
build-release:
    cargo build --release --all

# --- Test & checks ---

# Run all tests
test:
    cargo test --all

# Type-check the workspace without producing artifacts
check:
    cargo check --all

# --- Formatting & lint ---

# Format all code with rustfmt
fmt:
    cargo fmt --all

# Verify formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Run clippy with warnings denied
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# --- Run ---

# Run the scribe binary with arbitrary args
run *args:
    cargo run -p scribe -- {{args}}

# Run scribe's `doctor` subcommand
doctor:
    cargo run -p scribe -- doctor

# --- Install & housekeeping ---

# Install the scribe binary to ~/.cargo/bin
install:
    cargo install --path crates/scribe

# Remove the target/ directory
clean:
    cargo clean

# --- Release ---

# Placeholder release dry-run; tracked in #4
release-dry-run:
    @echo "see #4"
