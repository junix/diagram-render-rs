set shell := ["bash", "-euo", "pipefail", "-c"]

default: build

build:
    cargo build

build-release:
    cargo build --release

test:
    cargo test --all-targets --all-features

e2e-test:
    cd e2e && go test ./...

e2e: build
    cd e2e && go run . --diagram-render "{{ target_dir }}/debug/diagram-render-rs" run

e2e-doctor:
    cd e2e && go run . doctor

e2e-list:
    cd e2e && go run . list

e2e-matrix:
    cd e2e && go run . matrix

check:
    cargo check --all-targets --all-features

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

check-all: fmt-check clippy test e2e-test e2e

# Render all seven typed-AST examples to SVG and 2x transparent PNG, then
# generate a browser-viewable gallery from the public library API.
examples:
    cargo run --release --example gallery -- examples/rendered

doc:
    cargo doc --open

clean:
    cargo clean

update:
    cargo update

# Compiled binaries install per operating system and architecture (ADR-749).
os_name := if os() == "macos" { "macos" } else { "linux" }
arch_name := if arch() == "aarch64" { "arm64" } else { "x86" }
default_install_bin := home_directory() / "sync" / (os_name + "-" + arch_name + "-bin")
install_bin := env("SYNC_BIN_DIR", default_install_bin)
target_dir := env("CARGO_TARGET_DIR", justfile_directory() / "target")

install: build-release
    mkdir -p "{{ install_bin }}"
    cp "{{ target_dir }}/release/diagram-render-rs" "{{ install_bin }}/diagram-render-rs"
    chmod +x "{{ install_bin }}/diagram-render-rs"
    echo "Installed {{ install_bin }}/diagram-render-rs"
