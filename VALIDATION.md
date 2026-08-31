# Validation

Validated on 2026-08-30 on macOS arm64 with `rustc 1.98.0` and
`cargo 1.98.0`. The repository also pins Rust 1.85 as its minimum supported
toolchain for CI.

## Automated gates

- `just check-all`: passed (`rustfmt`, Clippy with `-D warnings`, all Rust
  targets/tests, the Go E2E harness tests, and live original-CLI parity).
- Test result: 14 Rust tests passed (4 renderer unit tests and 10 integration
  tests). These cover all seven AST families,
  SVG accessibility/escaping, transparent and explicitly painted canvases,
  PNG decoding, exact-width rasterization, serialized-AST input, bounded
  raster options, CLI output selection, and the checked-in gallery artifacts.
- The independent Go harness passed 12 unit tests covering its executable
  feature matrix, bidirectional matrix/registry guards, selectors, fixture
  coverage, deterministic environment, normalized image comparator, SVG text
  extraction, panic recovery, command dispatch, global flags, and drift
  semantics.
- Live parity passed 15/15 cases against the installed
  `plot-provider-diagrams` CLI: 14 parity cases covering the 68 aligned matrix
  rows across DBML, WaveDrom, D2, Structurizr, LikeC4, nomnoml, and Pikchr,
  plus shared malformed-DBML rejection. The matrix separately records 45
  intentional exclusions. `doctor` confirmed that both CLIs and all seven
  original backends were available.
- A negative probe replaced `diagram-render-rs` with `/usr/bin/true`; `PAR-003`
  failed with harness exit 1 and retained the case ID, assertion, both argv and
  exit values, and bounded subprocess evidence.
- `cargo doc --no-deps`: passed.
- `cargo machete`: no unused dependencies.
- `just examples`: regenerated seven SVGs, seven RGBA PNGs, and
  `examples/rendered/index.html` through the public library API.
- `just install`: installed the release binary to
  `~/sync/macos-arm64-bin/diagram-render-rs`.

## Rendered-artifact checks

- Every checked-in PNG decodes through `tiny-skia`, exceeds 200 px in both
  dimensions, and retains alpha zero at the top-left canvas pixel.
- The responsive gallery was opened with browser automation at desktop size.
  All seven cards rendered without horizontal overflow and the browser console
  contained no messages.
- The gallery was inspected at both the top and bottom of the page. Exact-size
  PNG inspection additionally covered DBML, WaveDrom, D2, and Pikchr after the
  final regeneration; text, connectors, clipping, and spacing were readable.

## Project-manager status

The project is registered as `diagram-render-rs`, with `origin` and the
upstream `main` branch configured at
<https://github.com/junix/diagram-render-rs>. `pm doctor --deep` passes all
environment and project checks.
