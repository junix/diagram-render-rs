# Diagram renderer parity E2E

This directory contains an independent Go harness that renders identical,
committed source bytes through two public CLIs:

- candidate: `diagram-render-rs`
- original compatibility CLI: `plot-provider-diagrams`, which delegates to the
  installed upstream tools and bundled upstream JavaScript renderers

The canonical invariant is: **for the shared supported slice of DBML,
WaveDrom, D2, Structurizr, LikeC4, nomnoml, and Pikchr, both CLIs accept the
same valid input and emit nontrivial diagrams with bounded normalized visual
drift; both reject the shared malformed DBML input.**

This is behavioral and coarse visual parity, not pixel identity. Different
layout engines, themes, fonts, and rasterizers make exact pixels unstable.

## Commands

```bash
go run . doctor
go run . list
go run . list --json
go run . matrix
go run . matrix --json
go run . run
go run . run --select PAR-003 --keep --report parity.json
go test ./...
```

Global participant overrides precede the command:

```bash
go run . \
  --diagram-render ../target/debug/diagram-render-rs \
  --original /path/to/plot-provider-diagrams \
  run
```

Resolution uses explicit flags, `DIAGRAM_RENDER_BIN` /
`PLOT_PROVIDER_DIAGRAMS_BIN`, in-tree or synchronized binaries, and `PATH`.
Both participants and all seven original backends are mandatory for the
default gate; a selected case requires only its backend. Missing required
infrastructure exits 3 instead of producing a false pass.

## Executable feature matrix

[`feature_matrix.json`](feature_matrix.json) is the exhaustive inventory for
the typed AST surface consumed by this renderer. It currently records 68
aligned features and 45 intentional exclusions across all seven languages.
Each aligned row names one or more registered cases, and each feature case
names the same feature IDs. Unit tests reject missing, duplicate, unknown, or
one-way bindings, so neither the matrix nor the registry can silently drift.

An intentional exclusion is not counted as parity. It covers fields that are
parsed but deliberately not evaluated, cross-file behavior outside this
renderer, or candidate output for which the installed original provider has no
comparable semantic value. `go run . matrix` prints per-language totals and
`go run . matrix --json` emits the complete machine-readable contract.

## Cases and assertions

| Case | Shared behavior |
|---|---|
| `PAR-001` | DBML schema rendering |
| `PAR-002` | WaveDrom timing rendering |
| `PAR-003` | D2 graph rendering |
| `PAR-004` | Structurizr context-view rendering |
| `PAR-005` | LikeC4 left-to-right view rendering |
| `PAR-006` | nomnoml classifier rendering |
| `PAR-007` | Pikchr object rendering |
| `PAR-008` | DBML extended declarations and four reference cardinalities |
| `PAR-009` | WaveDrom logic/clock/bus/unknown/high-Z symbols and node edges |
| `PAR-010` | WaveDrom register field widths and bit ranges |
| `PAR-011` | D2 map nesting, all edge operators, and edge chains |
| `PAR-012` | Structurizr container-view details and relationship metadata |
| `PAR-013` | nomnoml reverse/undirected/bidirectional and labeled associations |
| `PAR-014` | Pikchr shapes, four directions, and all implemented flow objects |
| `VAL-001` | malformed DBML rejection |

Every parity case compares decoded PNG facts after independently cropping the
ink bounds and normalizing to a 64×64 soft mask:

- ink aspect-ratio drift at most `0.85`;
- normalized ink-coverage drift at most `0.80` (`1.60` only for the intentionally
  denser card rendering versus the sparse D2 oracle in `PAR-011`);
- normalized mask intersection-over-union at least `0.10`.

The thresholds intentionally detect blank output, direction/aspect regressions,
and major structural drift without pretending the layout engines are identical.
For the six SVG-capable original backends, the harness additionally requires
case-specific shared semantic labels in both SVGs. Candidate-only and
original-only labels are separate assertions, which prevents an annotation
preserved by only one renderer from being reported as shared parity. LikeC4 is
PNG-only in the original CLI, so its case checks labels in the candidate SVG
and compares decoded PNG facts across both CLIs.

Each child receives a case-local `HOME`, fixed timezone/locale, a bounded
timeout, and a small environment allowlist. Proxy and credential variables are
not inherited. `--keep` retains both participants' artifacts for visual
diagnosis; otherwise scratch data is deleted. Harness exit codes are 0 for a
green run, 1 for case failures, 2 for usage/selection errors, and 3 for missing
required infrastructure.
