# Renderer warnings and opt-in publication refusal

Validated on 2026-10-05 using Rust/Cargo 1.99.0 on Linux x86_64.
This unit follows the DBML parser-pin alignment; it does not change parser
or theme dependencies, public `RenderOptions`, or SVG geometry.

## Bounded DBML warning

Inline column `[ref: ...]` settings are retained by the parser AST but are not
rendered as connectors. The renderer now returns one deterministic warning
counting authored column settings named `ref` (ASCII case-insensitive) in
both tables and table partials. It does not echo setting values or source
text. Table settings, comments, and similarly named settings are not counted.

The count is not a count of missing edges: a partial can be unused or reused,
and a relationship can also have an explicit `Ref` declaration. Partial
application does not multiply the count. Explicit declarations continue to
produce their existing connectors without this warning. Inline settings do
not add connectors or change existing SVG bytes.

## CLI publication policy

`--deny-warnings` returns exit 1 if rendering reports any warnings. The check
runs after rendering but before stdout emission, output-directory creation,
file creation, or replacement. It applies to SVG and PNG, source and AST JSON,
and every diagram family. `--quiet` cannot suppress this fatal policy or its
diagnostic. Without the flag, existing successful output behavior continues;
nonfatal warnings appear on stderr unless `--quiet` is used.

This is a gate on reported renderer warnings, not full DBML validation, a
complete unsupported-semantics detector, an import resolver, or an OS sandbox.
It does not protect files that a calling shell truncates before launching the
CLI; use `--output` for the tested no-replacement guarantee. The library still
returns `Rendered::warnings`, leaving policy to its caller.

## Regression coverage

`tests/warnings.rs` covers zero, one, repeated/multiple, and mixed-case inline
settings; unused and reused partials; mixed table/partial counts; ordinary
explicit references; comment/table-setting/lookalike controls; deterministic
warnings; source/serialized-AST parity; and unchanged geometry with inline
settings or duplicate explicit declarations.

CLI tests cover default warning output, quiet successful output, warning-free
strict stdout/file output, strict source and AST refusal, an existing D2 import
warning, and SVG/PNG refusal with both quiet modes. Refusal emits no stdout,
leaves an existing output sentinel unchanged, and does not create a missing
output directory or file.

An external exact-byte comparison against the aligned renderer checks all
seven native gallery fixtures in the default and fourteen canonical themes,
the five parser-alignment controls, and four inline-reference controls:
**114/114 SVG outputs unchanged**.

## Automated gates

- `cargo fmt --all -- --check`: passed
- `cargo clippy --locked --offline --all-targets --all-features -- -D warnings`: passed
- `cargo test --locked --offline --all-targets --all-features`: passed (51 tests)

The shared native gallery tests include SVG/PNG checks across all fourteen
canonical themes. No Go E2E, live original-provider comparison, browser
inspection, remote CI, or older Rust toolchain result is claimed here.
