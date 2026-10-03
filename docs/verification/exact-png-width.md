# Exact PNG width verification

Date: 2026-10-03. Base: `95a3f64d3209a8beff712c5978195e695dddc23c`.

## Fix

`RenderOptions::width` promises an exact PNG pixel width. Previously, the
rasterizer divided this integer by the SVG width into a binary32 scale, then
multiplied the scale by the SVG width and rounded upward. Binary32 error can
put that product just above the requested integer.

For a 660 CSS-pixel scene, the old arithmetic maps requested width 44 to 45
and width 702 to 703. The fix retains the requested integer as the PNG buffer
width. Effective scale, height rounding, rendering transform, and PNG encoding
stay unchanged. In particular, the existing binary32 height calculation is
preserved; the 660-by-220 scene at width 702 still has computed height 235.

The existing checks still run before allocating the raster buffer:

- Finite effective scale in the inclusive range 0.05 through 16
- Nonzero output dimensions
- At most 32,768 pixels on either dimension
- At most 100,000,000 total pixels, with multiplication in `u64`

The public zero-width rejection and scale-option validation are unchanged.
This patch does not change background behavior, dependencies, or rasterization
when only a scale is requested.

## Native regression coverage added

These tests are written but have **not been executed** in this environment.

- `tests/render.rs`: extend the existing exact-width regression to decode
  the PNG and compare its dimensions with the returned metadata. Cover the
  original D2 request and WaveDrom scenes with CSS widths 660, 702, and 1162,
  including failing arithmetic witnesses and neighboring requests
- `tests/render.rs`: reject zero width and widths outside the existing
  effective-scale interval through the public API
- `src/raster_tests.rs`: decode PNGs for explicit-width rounding witnesses,
  fractional source dimensions, both inclusive scale bounds, and the inclusive
  32,768-pixel dimension boundary. Assert exact widths and expected heights
- `src/raster_tests.rs`: verify the unchanged scale-only ceiling behavior
- `src/raster_tests.rs`: reject out-of-range effective scales, output width
  above the dimension cap, output height above the cap, and total pixel count
  above the budget. Rejected fixtures do not require large pixel allocations

## Checks actually performed

- Re-read GitHub `main` and its complete tree; it still pointed to the base
  commit above. No repository `AGENTS.md` or bundled skill files were present
- Verified all 31 locally available baseline files against their fresh remote
  Git blob IDs. Read README, validation notes, manifest, test/build recipes,
  CI workflow, and the relevant source before editing
- Static comparison verified that the sole production behavior change is
  retaining explicit width. Every guard, limit, height calculation, transform,
  and allocation/encoding step is otherwise byte-for-byte unchanged
- A standard-library Python binary32 arithmetic witness reproduced the
  44-to-45 and 702-to-703 cases for CSS width 660. A 5,875-case width sweep
  across source widths 660, 702, and 1162 found 414 old rounding mismatches
- The same numeric witness checked the new tests' literal expected heights,
  effective-scale rejection rows, scale-only dimensions, and pixel-budget
  arithmetic. This is an arithmetic model, not Rust execution or PNG rendering

## Verification limits and next commands

`rustc`, `cargo`, and `rustfmt` are unavailable on PATH. No toolchain installation,
dependency downloads, browser work, or CI runs were performed. Compilation,
rustfmt, Clippy, native tests, decoded PNG execution, theme checks, gallery
regeneration, and CLI parity remain unverified for this patch. Earlier results
in the repository's `VALIDATION.md` do not validate this change.

In a complete checkout with the pinned toolchain and dependencies available:

```sh
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib raster::tests
cargo test --test render
cargo test --all-targets --all-features
# Full repository gate also needs its external theme and original-CLI tools:
just check-all
```
