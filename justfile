set shell := ["bash", "-euo", "pipefail", "-c"]


default: build

build:
    cargo build

build-release:
    cargo build --release

test:
    cargo test

check:
    cargo check

fmt:
    cargo fmt

fmt-check:
    cargo fmt -- --check

clippy:
    cargo clippy -- -D warnings

check-all: fmt-check clippy test

doc:
    cargo doc --open

clean:
    cargo clean

update:
    cargo update

# Library project — nothing to install. The recipe exists so the language
# directory's recursive `just install` does not fail.
install:
    @echo "Library project — no binary to install"
