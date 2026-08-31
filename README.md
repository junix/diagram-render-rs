# diagram-render-rs

Pure-Rust SVG/PNG rendering for the seven typed AST families produced by
[`diagram-ast-parser`](https://github.com/junix/diagram-ast-parser): DBML,
WaveDrom, D2, Structurizr DSL, LikeC4, nomnoml, and Pikchr.

The library consumes an already parsed `Document`. The CLI also accepts source
text as a convenience, but parsing remains owned by `diagram-ast-parser`.

```text
source text ──diagram-ast-parser──▶ typed, source-spanned Document
                                             │
             ┌───────────────────────────────┼───────────────────────────────┐
             ▼                               ▼                               ▼
      schema/card layouts             timing/register layout          geometric layout
  DBML · D2 · Structurizr ·          WaveDrom lanes/fields                 Pikchr
      LikeC4 · nomnoml
             └───────────────────────────────┼───────────────────────────────┘
                                             ▼
                              finite drawing Scene primitives
                                             ▼
                                     SVG ──resvg──▶ PNG
```

The shared `Scene` is a visual boundary—rectangles, ellipses, polylines,
polygons, and text—not a universal graph model. Each language resolves its own
names, hierarchy, relationships, and layout meaning before lowering compatible
visual constructs. This matters because a schema, timing diagram, architecture
model, classifier graph, and procedural geometric scene are not interchangeable
domain objects.

## Rendered surface

| AST family | Current rendering |
|---|---|
| DBML | project title, tables/columns/settings, enums, partials, groups, notes, references and cardinality labels |
| WaveDrom | nested signal lanes, 0/1/clock/bus/unknown/high-Z cells, data labels, node edges, header/footer, register bit fields |
| D2 | entries/maps, labels/properties, directed/reverse/undirected/bidirectional edge chains |
| Structurizr DSL | workspace/model elements, nesting hints, technology/description, relationships; view syntax is reported but not evaluated |
| LikeC4 | model/deployment elements, nesting hints, typed relationships; view predicates/includes are reported but not evaluated |
| nomnoml | classifier types, attributes, compartments, labels, and association direction/style |
| Pikchr | common objects, direction changes, flow lines/arrows, labels; expression-level geometry and macro expansion are reported but not evaluated |

The renderer is intentionally syntax-AST-driven. It does not load imports,
resolve cross-file names, evaluate Structurizr/LikeC4 views, expand Pikchr
macros, or claim upstream-renderer pixel parity. Non-fatal degradations are
returned in `Rendered::warnings` and printed by the CLI unless `--quiet` is
used.

## CLI

Output format is inferred from `--output`; SVG is the stdout default.

```console
diagram-render-rs schema.dbml --format dbml -o schema.svg
diagram-render-rs timing.json5 --format wavedrom -o timing.png --scale 2
diagram-render-rs architecture.d2 --format d2 -o architecture.png --width 1400
cat model.c4 | diagram-render-rs - --format likec4 > model.svg
```

The canvas is transparent by default. Paint it only when requested:

```console
diagram-render-rs workspace.dsl -f structurizr -o workspace.png \
  --background '#ffffff' --theme light
```

If a pipeline already has the serialized parser AST, skip source parsing:

```console
diagram-parse source.dbml --format dbml > document.ast.json
diagram-render-rs document.ast.json --ast-json -o document.svg
```

Run `diagram-render-rs --help` for all options, including dark theme,
font-family override, exact PNG width, stdin/stdout, and explicit `-T svg|png`.

## Library

Render source text through the convenience boundary:

```rust
use diagram_render_rs::{
    DiagramFormat, OutputFormat, RenderOptions, render_source,
};

let source = "Table users { id bigint [pk] }";
let rendered = render_source(
    DiagramFormat::Dbml,
    source,
    OutputFormat::Png,
    &RenderOptions { scale: 2.0, ..RenderOptions::default() },
)?;

std::fs::write("schema.svg", &rendered.svg)?;
std::fs::write("schema.png", rendered.png.expect("PNG requested"))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Or render an AST without reparsing:

```rust
use diagram_render_rs::{OutputFormat, RenderOptions, render_document};

let rendered = render_document(&document, OutputFormat::Svg, &RenderOptions::default())?;
assert!(rendered.png.is_none());
# Ok::<(), Box<dyn std::error::Error>>(())
```

SVG is always retained in `Rendered`, even for PNG output. PNG allocation is
bounded to 32,768 pixels per dimension and 100 million pixels total. Raster
buffers start as transparent black, so untouched corners retain alpha zero
unless `RenderOptions::background` paints the canvas.

## Examples and validation

The editable inputs live under [`examples/inputs`](examples/inputs). Generate
seven SVGs, seven 2× PNGs, and a responsive visual gallery with:

```console
just examples
open examples/rendered/index.html
```

Run the full local gate:

```console
just check-all   # Rust gates + harness tests + original-CLI parity E2E
just install     # ~/sync/<os>-<arch>-bin/diagram-render-rs
```

The independent [`e2e/`](e2e/) harness compares this CLI with the original
`plot-provider-diagrams` compatibility CLI across all seven source languages.
It checks shared acceptance, semantic SVG labels where the original supports
SVG, participant-specific value labels, decoded PNG structure, and shared
invalid-input rejection. Its executable
[`feature_matrix.json`](e2e/feature_matrix.json) binds 68 aligned features to
14 parity cases and records 45 intentional exclusions instead of presenting
candidate-only or unsupported semantics as parity. Run `just e2e-doctor` to
resolve both CLIs and the original backend toolchain, `just e2e-list` to inspect
all 15 cases, or `just e2e-matrix` for the per-language alignment totals.

The parser dependency is an exact Git revision, rather than a local path, so
the project remains an independently reproducible repository.

## License

MIT; see [LICENSE](LICENSE).
