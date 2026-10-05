# DBML parser rejection alignment

Validated on 2026-10-05 against renderer base
`c42ecad662f67efaf42385537e75d1fc88190bc5`, using Rust/Cargo 1.99.0 on
Linux x86_64. This is an exact-revision dependency update, not an adapter or
renderer language expansion.

## Dependency contract

- Parser manifest and lock source advance from
  `a8540652364157838d8c6398f08d9abfb4efcb93` to published commit
  `1bb33fbd22b6a51580021a157f30e1484221f0f5`.
- The shared theme stays at `0d6530f0ccea5a61b0d2fef9a3e164ff816ca852`.
- No registry versions, renderer production source, or image fixtures change.
- The offline build used an isolated Cargo directory-source replacement. All
  44 tracked parser files were independently checked against the published
  Git tree using Git blob hashes before compilation. No path dependency or
  fabricated Git revision was used.

## Rejection and compatibility coverage

`tests/dbml_rejections.rs` exercises the actual CLI with three formerly lossy
inputs: multiple relationships inside one Ref block, a braced column body,
and an extra token after a table alias. Each is checked with stdout output,
a nonexistent SVG path, and an existing SVG path. The contract is exit 1,
a useful parser diagnostic, zero stdout bytes, no new output file, and no
change to existing output bytes, including with `--quiet`.

Valid named and unnamed single-Ref syntax variants retain equal SVG output.
Aliases with empty settings, quoted aliases, and single/multidimensional array
types remain accepted and retain their rendered type text.

The old exact-revision binary accepted all three malformed cases and produced
SVG (4,132, 1,691, and 1,691 bytes respectively). The updated binary rejected
all three with exit 1 and zero stdout bytes.

An external byte-for-byte comparison checked all seven native gallery inputs
in the default theme and all fourteen canonical themes, plus five valid DBML
probes (single/named Ref, ordinary/quoted alias with empty settings, and array
types): **110/110 SVG outputs were unchanged**.

## Automated gates

- `cargo fmt --all -- --check`: passed
- `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`: passed
- `cargo test --locked --offline --all-targets --all-features`: passed (45 tests)

The shared gallery gate exercises real SVG/PNG output across all fourteen
canonical themes. No live original-provider parity, Go E2E harness, browser
inspection, or older Rust toolchain validation is claimed by this update.
