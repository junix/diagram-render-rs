# WaveDrom register width safety

Prepared against `ed1cdbcca83120bdd440bd263e259e3267e67f19` on
2026-10-04. The exact-PNG-width implementation from that commit is preserved.

## Behavior

- Missing and zero field widths both occupy one displayed bit, as the renderer
  already intended. The aggregate, geometry, and bit-range labels now use the
  same normalized width.
- The normalized aggregate uses checked `u64` addition. A total greater than
  `u64::MAX` returns `RenderError::InvalidScene` with the message
  `WaveDrom register total bit width exceeds u64::MAX` for both SVG and PNG.
  This is a rendering error rather than a warning: no representable bit-index
  range exists for that register. Existing nonfatal relationship warnings keep
  their behavior.
- Each rectangle is capped by the remaining register space. Float rounding
  cannot make the final rectangle negative; the ratio is computed before
  multiplication to avoid an unnecessarily large intermediate product.
- Positive widths retain their proportional layout and exact integer bit
  labels. A single `u64::MAX` field is supported, with no per-bit allocation.
  Empty registers remain valid. Public API signatures and AST types do not
  change.

Previously, three zero-width fields were totaled as one bit but each drawn as
one bit: their widths were `592`, `592`, and `-592` pixels on a 660-pixel canvas.
The corrected widths are approximately `197.333` pixels each, with labels
`2`, `1`, and `0`. A register with zero, missing, and two-bit fields now uses
widths `148`, `148`, and `296`, with labels `3`, `2`, and `1:0`.

## Regression coverage

Renderer unit tests check dimensions, adjacency, total occupied width, and
range labels for zero, missing, mixed, positive, and empty fields. Boundary
cases include `u64::MAX`, its two-field partitions, 4,096 one-bit fields, and
unequal widths. Overflow tests also verify that register drawing emits no
partial primitives.

The public API tests construct the typed AST directly, avoiding possible
JSON5 number conversion at the `u64` boundary. They reject `u64::MAX` followed
by one, zero, missing, or another `u64::MAX` field for both output formats.
Separate source-parsing tests render and decode PNG output. The existing
exact-width raster tests remain in the full native test run.

## Verification environment

Native verification uses Rust/Cargo 1.99.0 and Go 1.27.1. The renderer and all
76 original repository blobs were checked against the pinned Git tree. The
pinned parser dependency is unchanged.

The private theme dependency could not be fetched by Cargo's libgit2 or Git
CLI transport without additional authentication. A test-only Cargo path patch
uses the exact pinned theme revision
`0d6530f0ccea5a61b0d2fef9a3e164ff816ca852`, materialized through the connected
GitHub source and verified against its Git blob hashes. The separate native
test copy differs from the candidate only by removal of that dependency's Git
source line in its generated lockfile. Published `Cargo.toml` and `Cargo.lock`
remain unchanged; no dependency upgrade or credential configuration is part
of this patch.

The one existing formatting failure in the exact-PNG-width regression
assertion was corrected without changing the assertion.

### Native results

- `cargo fmt --all -- --check`: passed after the formatting-only correction
- Focused library and WaveDrom API tests: 14 passed
- `cargo test --all-targets --all-features`: 37 passed, including exact PNG
  widths, all seven AST families, CLI, checked-in artifacts, and theme tests
- `cargo clippy --all-targets --all-features -- -D warnings`: passed
- `go test ./...` in `e2e`: passed
- Native negative control against the unchanged parent, in a separate build
  directory: the new public overflow test failed with an integer-overflow
  panic and the zero-width test detected a negative SVG rectangle; the single
  `u64::MAX` field remained valid

An additional independent Python IEEE-754 binary32 model checked 10,015
bounded cases. It verified 2,518 representable aggregates and rejected 7,497
normalized overflows. Thirty cases produced negative widths in the old
arithmetic. This model supplements native execution; it is not a substitute
for it.

Browser inspection, CI, MSRV validation, live original-CLI parity, and the
external `diagram-theme check-output` aggregate gate are not claimed here.
