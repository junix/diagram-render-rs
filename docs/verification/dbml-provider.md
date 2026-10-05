# Authored DBML native provider verification

Validated 2026-10-05 on Linux x86_64 with Rust/Cargo 1.99.0.
The unchanged renderer-code baseline is
`8203dbe909d588ad0239f0bd77f9b439f48111e5`; the parser and theme stay pinned to
`1bb33fbd22b6a51580021a157f30e1484221f0f5` and
`0d6530f0ccea5a61b0d2fef9a3e164ff816ca852`. All 85 baseline repository blobs
were compared with the fresh GitHub main tree. No existing dependency package
record changed; new provider dependencies were added with a locked build.

## Automated evidence

- Format check passed
- Clippy, all targets/all features with warnings denied, passed
- Existing Go E2E harness unit tests passed (no original external CLI run)
- `cargo run --locked --offline -- --help` still selects `diagram-render-rs`;
  package `default-run` preserves the prior route after adding a second binary
- All-target/all-feature tests passed: 72 tests, including 21 new provider tests
- Descriptor-driven Python schema gate passed: 32 actual executions across
  16 palettes and transparent/painted backgrounds, closed source/receipt schema,
  byte counts/SHA-256 and static SVG checks
- Rust native parity tests compare provider SVG with independently source-parsed
  native CLI output in all 32 theme/background combinations, then repeat each
  provider invocation for exact deterministic replay
- Independent provisional review exercised 255 native CLI probes, including
  128 independently constructed upstream-AST/native-renderer byte comparisons across four cardinalities,
  16 themes and both canvas modes; final review is performed on frozen files

Failures cover unknown/missing/null/duplicate JSON fields, escaped duplicate
keys, input bytes/depth/node/count limits, unsupported syntax/semantics,
identities, references, XML/Unicode/control rejection, flags, exact composed
labels, pins, backgrounds/themes, aliases and unchanged prior output pairs.
Unit tests exercise real native warning refusal, input/directory identity
drift, destination drift, private staging, first/second write failures, backup
preparation failures, hardlink-preserving rollback and recovery retention when
rollback fails. Deliberately injected failures are test-only, not runtime hooks.

## Visual boundary evidence

Initial 24-W identifiers with 34-cell column lines did not fit native cards in
Inkscape 1.4. An 18-cell W-heavy reference also exceeded its native label pill.
The final profile therefore refuses identifiers/types longer than 12 bytes,
composed column lines over 18 display cells and reference labels over 11 cells.
No renderer layout or font logic changed, and nothing is silently truncated.

The executable `users-posts.json` and `wide-labels.json` examples were rendered
by the provider and opened as SVG in the local Inkscape 1.4 renderer. Both
ordinary two-table relationships and maximal admitted W/M titles/columns fit.
Independent geometry checks and visual inspection covered all four cardinality
labels in both authored directions: eight accepted wide-glyph cases fit card,
canvas and label-pill bounds, with one arrowhead at the authored `to` table.
This is local viewer evidence, not universal font fitting or a raster guarantee.
Native raster output is outside the provider contract; installed viewers can
resolve the native SVG font stack differently.

## Scope of the result

This is local native-provider evidence for the closed authored JSON profile.
It does not establish general DBML-text acceptance, full database semantics,
column-port/crow's-foot ER rendering, complete unsupported-feature detection,
remote CI, cross-platform pixels, or a crash-atomic two-file transaction.
Hub admission and its compiled CLI/MCP transports are a separate consumer gate.
