# Card connector obstacle routing verification

Verified 2026-10-05 on the native Linux renderer, against baseline commit
`87434eab70888e202ba3c2cab7ad33139fdb9d1a`.

## Reproduced defect and repair

The source `A\nB\nC\nD\nE\nA -> C: skips B` produces three card columns.
The old native SVG emitted `(300,120.5) → (742,120.5)`, through card B's
interior; the later-painted card hid the connector and its label. The repaired
route uses boundary ports and an orthogonal detour below B, with its label
centered on the clear segment. The canvas remains 1042 × 432.

Card positions, input order, styles, arrowhead semantics, and clear legacy
route/label geometry remain unchanged. Unsafe paths, labels, and duplicated
routes receive bounded deterministic repairs. Errors propagate through all five
card-family adapters before any SVG/PNG output is published.

The implementation uses the native visibility-grid routing pattern inspected in
`graph-ir-rs` and `tala-rs`. The router adds no layout-engine dependency. V1 compatibility separately pins
the old renderer library, as described below. Coordinate-grid allocation,
route vertices, card/edge counts, and counted geometry/search work have explicit
bounds. Port and clearance retries share those work bounds.

## Passed gates

- `cargo fmt --all -- --check`
- `cargo clippy --offline --locked --all-targets --all-features -- -D warnings`
- `cargo test --offline --locked --all-targets --all-features`: 91 tests
- `go test ./...` in `e2e`
- All thirteen valid E2E fixtures render; the malformed DBML fixture retains its
  rejection. This includes the four distinct extended-DBML associations.
- Ninety-six safe legacy SVG cases across fourteen canonical themes plus the two
  legacy palettes, and twelve native PNG cases, are byte-identical to baseline.
- Public source-to-AST replay and repeated rendering are deterministic.
- Card/connector/visibility-grid/work bounds and no-route cases fail explicitly.
  CLI tests verify stdout remains empty, existing output bytes survive, and
  missing output parents remain absent for routing-budget failures; both SVG
  and PNG no-fit failures also preserve an existing output.

Independent SVG auditing checked 531 single-edge cases (all successful) and 384
ordered two-edge cases (375 successful). Every emitted route, label pill, arrow,
and endpoint attachment passed the geometry checks. The matrix includes uneven
card heights, skipped columns/rows, self-loops, reverse and parallel pairs, and
wide labels. The nine two-edge rejections are wide self-loops after a prior wide
connector, described below.

Native SVG/PNG visual review covered the original five-card defect,
cross-row connectors, mixed directed/reverse/bidirectional/undirected styles,
triple parallel/reverse pairs, self-loops, wide paired labels, Chinese labels,
variable-height cards, and all four extended DBML associations. Transparent PNG
corners were checked; white-background inspection copies made the dark strokes
readable. Explicit installed DejaVu Sans and Noto Sans CJK SC fonts were used for
inspection without changing application defaults.

## Limits and unavailable gate

This is a bounded heuristic, not a complete or optimal layout solver. It tries
fixed cardinal ports and shortest-path alternatives with finite clearances.
A search error means no safe route/label was found among those alternatives;
it does not prove no geometric solution exists. Some of the nine wide-loop
rejections have valid longer detours. They deliberately fail closed rather than
emit obscured relationships.

Pill geometry and glyph ink are separate. Existing label truncation and
approximate text widths remain unchanged. Unusually wide glyph sequences or
custom fonts can exceed a pill's estimated bounds; this pre-existing typography
limitation was independently confirmed with identical baseline/candidate PNG
bytes and is outside this routing repair.

The original-provider visual-parity gate could not run: `e2e doctor` reports
`plot-provider-diagrams` missing (exit 3). Native tests, the Go harness tests,
all fixture acceptance checks, baseline byte parity, and the independent native
geometry/visual matrix did run. No missing upstream gate is counted as passed.

## V1 provider compatibility boundary

`plot-provider-dbml` V1 deliberately does not adopt this routing change. It links
the whole renderer library at the exact Git revision already declared by its
closed receipt schema (`8203dbe909d588ad0239f0bd77f9b439f48111e5`). Current
library/CLI users receive the repaired router; V1 consumers retain their pinned
rendering implementation. The descriptor/schema/profile identities are unchanged.
See [the provider verification](dbml-provider.md) for source-tree, Cargo and
exhaustive admitted-height parity checks.
