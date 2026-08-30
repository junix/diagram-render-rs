# Example gallery

`inputs/` contains one editable source for every AST family supported by
`diagram-ast-parser`:

- `schema.dbml`
- `timing.json5`
- `architecture.d2`
- `workspace.dsl`
- `model.c4`
- `classes.nomnoml`
- `flow.pikchr`

Run `just examples` from the repository root. The public library example in
`gallery.rs` parses each source, renders SVG and transparent 2× PNG, and writes
`rendered/index.html` for side-by-side browser inspection. Generated SVG/PNG
files are checked artifacts, not substitutes for the editable inputs.
