# Author-canvas marker and shared output-gate regression

Validated on 2026-10-04 against renderer parent
`7e6a99f13a895d1bdd35acab5b6651bda0f4748e`, using Rust/Cargo 1.99.0 and Go
1.27.1 on Linux x86_64.

## Defect and isolated fix

An explicit `RenderOptions::background` / `--background` emitted a full-canvas
rectangle marked `data-canvas-background="true"`. The shared output gate only
recognizes `data-canvas-background="author"`. Consequently an intentional,
off-palette canvas failed its SVG palette and full-bleed rules, and the paired
opaque PNG failed all twelve transparency samples.

The emitter now uses `author`. The rectangle geometry, paint, ordering,
default transparency, and raster dimensions are unchanged. The existing public
render assertion now checks the contract spelling.

## Real cross-crate integration coverage

`tests/canvas_contract.rs` calls the public `render_source` API and passes the
resulting SVG and PNG files to `diagram_theme::output::check_files`:

- All fourteen canonical themes with no background pass with no exemptions or
  skipped coverage. Their actual PNGs are decoded and sampled by the shared gate.
- All fourteen themes with explicit, off-palette `#123456` pass, report the
  author full-bleed exemption, and declare exactly one author-canvas marker.
  An independent PNG decode also checks the opaque corner's exact RGB value.
- Replacing the correct marker with the old `true` spelling fails the shared
  `full-bleed`, `palette`, and PNG `canvas` rules. The negative case retains the
  actual rendered PNG rather than substituting a synthetic image.

The fixture is an unlabelled D2 edge (`a -> b`). It isolates the canvas contract;
this original run did not check the full gallery. The earlier claim that the
0.98-opacity labels violate the pinned palette gate was stale: the declared
checker already accepts token colors at arbitrary opacity. See
[the subsequent gallery verification](gallery-contract.md) for all seven
fixtures in all fourteen themes. Canonical themes are used because the gate accepts
shared palettes; the legacy default `light` palette is intentionally distinct
and remains covered by the existing renderer tests.

## Dependency decision

No theme pin change is needed. The existing exact revision
`0d6530f0ccea5a61b0d2fef9a3e164ff816ca852` already exports `check_files`,
`check_svg`, `check_png`, `declares_author_canvas`, `Report`, and `Note` behind
its `cli` feature. A dev-dependency enables that feature only for testing.
The normal dependency remains `clap`-only.

Cargo updates the lockfile to add `png 0.17.16` and its `bitflags 1.3.2`
dependency, reuse the already locked `roxmltree 0.20.0` and `serde_json`, and
disambiguate existing `png`/`bitflags` entries. All existing package versions and
both private Git revisions are preserved. The normal/build feature tree was
checked: `diagram-theme` activates only `default` and `clap`, not `cli`.

## Verification performed

- `cargo fmt --all -- --check`: passed
- `cargo check --all-targets --all-features`: passed
- `cargo clippy --all-targets --all-features -- -D warnings`: passed
- `cargo test --all-targets --all-features`: 40 tests passed, including the exact
  PNG-width and WaveDrom register-width regressions
- `cargo test --doc`: passed (no doctests)
- `go test ./...` in `e2e`: passed
- Separate-target negative control: the same integration tests with the parent
  emitter restored produced exactly one expected failure, the explicit canvas
  acceptance test. Default transparency and the rejection test still passed.

The native run used an exact-source local patch for the private theme dependency:
its 20 materialized Rust/manifest/registry files were rechecked against the
GitHub connector's pinned blob hashes. Only the native test copy's theme Git
source line was removed from `Cargo.lock` for that local patch; the distributable
manifest and lock retain the original Git URL and exact revision. The parser
dependency was the existing exact pinned Cargo checkout. Registry dependencies
were resolved offline from the authorized cache. No credentials were used.

The declared Rust 1.85 MSRV, browser rendering, CI, live original-CLI parity, and
the external `just check-themes`/`just check-all` aggregate were not run for this
change. No remote writes or publication were performed.

## Reproduction

With access to the pinned Git dependencies:

```console
cargo test --locked --test canvas_contract
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo test --locked --doc
(cd e2e && go test ./...)
```

To demonstrate the original defect, restore only the `true` marker in
`src/svg.rs` and rerun `canvas_contract`. The explicit background test must fail
with SVG palette/full-bleed errors and opaque PNG canvas samples.
