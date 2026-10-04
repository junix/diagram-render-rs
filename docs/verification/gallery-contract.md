# Reproducible gallery output gate

Validated on 2026-10-04 against renderer
`b7eed9b45e5f9847221e5c8dfab5d9a493d7c200`, with Rust/Cargo 1.99.0 on Linux
x86_64. No renderer paint, geometry, fixture, checked-in image, dependency
version, or dependency feature changed.

## Diagnosis

The connector-label pill remains `surface_alt` at `fill-opacity="0.980"`.
The renderer's declared `diagram-theme` revision
`0d6530f0ccea5a61b0d2fef9a3e164ff816ca852` already accepts a palette token at
arbitrary opacity. Its `is_allowed` implementation checks RGB membership, and
its own `a_token_at_any_opacity_is_still_that_token` regression explicitly
covers this renderer's 0.98 pill. The old claim that these labels fail palette
containment was stale. Making the labels opaque or broadening the palette gate
would therefore be an unjustified rendering or contract change.

The old `check-themes` recipe selected an unrelated installed `diagram-theme`
executable from `PATH`, so the checker was not tied to the renderer's declared
contract. Its comment also said the recipe was outside `check-all`, although
`check-all` already depended on it. The recipe now invokes the pinned
`gallery_contract` integration test. The test still renders through the real
CLI, using the default 2x PNG scale; it expands the old schema-only matrix to
all seven gallery source families and rejects any failure, skip, or exemption.

Full-bleed semantics are unchanged. A small translucent label is not a canvas.
The earlier canvas-marker negative control's full-bleed failures concern the
explicit full-canvas `#123456` rectangle, not a connector label. The new
negative controls independently confirm that even a theme-colored full-canvas
fill at 0.98 remains forbidden without author authorization.

## Coverage and results

- All 98 real CLI SVG/PNG pairs (seven gallery fixtures, fourteen canonical
  themes): passed with zero failures, skipped coverage, or exemptions
- The labelled DBML, D2, Structurizr, and LikeC4 outputs explicitly retain their
  group-token label pills at 0.98 opacity
- In each theme, replacing a rendered pill's color with off-palette magenta
  fails `palette`; adding a theme-colored full-canvas rectangle at the same
  0.98 opacity fails `full-bleed`: 28 expected rejections
- Independent 1x public-library gallery matrix: all 98 pairs passed both the
  exact declared checker and a separately built checker from current shared
  theme revision `1cc4e6667aa86444a7e9055549aa85cac293dc07`, with no skips or
  exemptions. This is verification only; the renderer's pin is unchanged
- `cargo fmt --all -- --check`: passed
- `cargo check --locked --all-targets --all-features`: passed
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed
- `cargo test --locked --all-targets --all-features`: all 42 tests passed,
  including author-canvas, exact PNG width, and WaveDrom register regressions
- `cargo test --locked --doc`: passed (no doctests)
- `go test ./...` in `e2e`: passed
- Actual `just 1.58.0` dispatch of `check-themes` passed with a deliberately
  failing `diagram-theme` executable first on `PATH`; that executable was not
  called. The exact recipe body was also checked directly

The native test copy used a local path patch containing the exact declared
shared-theme source, verified against all 20 materialized source blob hashes.
The parser used its exact pinned Cargo checkout. The distributable Cargo files
remain byte-identical to the baseline. Registry packages came from the offline
cache. The comparison checker was independently source-verified and its
executable copied and hashed before use, so no matrix points at a mutable
shared target binary.

The source was a verified materialized snapshot rather than a Git clone.
The global Git-derived `stamp` variable therefore needed an explicit value:
`just --set stamp verified-b7eed9b check-themes`. That value is unused by this
recipe, which dispatches the unmodified pinned Cargo command. This is a
source-snapshot accommodation, not a pristine-clone or full aggregate claim.

No browser, remote CI, Rust 1.85 MSRV, live original-backend parity, or complete
`just check-all` run is claimed. The full aggregate still includes live backend
parity, which needs separately installed original CLIs.

## Reproduction

With access to the declared Git dependencies:

```console
just check-themes
# Equivalent pinned command, without requiring just or diagram-theme:
cargo test --locked --test gallery_contract
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo test --locked --doc
(cd e2e && go test ./...)
```
