# Native authored DBML JSON provider

`plot-provider-dbml` exposes `diagram.dbml.render-svg-v1`: a deliberately small,
closed **authored DBML JSON subset**, passed to the real `render_document` API.
It does not accept general DBML source text or upstream serialized `Document`.
Existing `diagram-render-rs` library and CLI routes keep their original behavior.

## Invocation

```sh
cargo build --locked --bin plot-provider-dbml
plot-provider-dbml describe --json
plot-provider-dbml doctor --json
INPUT=examples/dbml-authored/users-posts.json
SHA=$(sha256sum "$INPUT" | cut -d ' ' -f 1)
plot-provider-dbml render-svg --input "$INPUT" \
  --resource-pins "{\"input\":\"$SHA\"}" \
  --output /tmp/schema.svg --receipt /tmp/schema.receipt.json
```

Successful rendering writes the two files and emits no stdout/stderr. Invalid
CLI arguments exit 2; runtime validation/render/publication failures exit 1
with a stderr diagnostic. `doctor` only reports linked-renderer readiness.
The executable is local, with no downloads, imports, resource discovery,
external rendering process, font discovery, network requests or viewer launch.
There is no OS sandbox claim.

The exact command descriptor is
[`assets/dbml-render-svg-command-v1.json`](../assets/dbml-render-svg-command-v1.json).
`--resource-pins` is mandatory, duplicate-free, closed JSON containing exactly
`input`, a lowercase SHA-256 of the original file bytes. Formatting changes to
input change its receipt hash even when the resulting SVG is identical.

## Closed profile

[`dbml.authored/v1`](../schemas/dbml-authored-v1.schema.json) requires all fields:
`schema_version`, `tables`, `refs`; each table has `name`, `columns`; each column
has `name`, `data_type`, `flags`; each reference has `from`, `to`, `cardinality`;
each endpoint has `table`, `column`. Objects reject unknown, missing, null and
duplicate fields, including JSON-escaped duplicate names. A bounded syntax
scan runs before typed decoding; no generic map can erase duplicate keys.

- 1–2 tables, each with 1–16 columns; 0–1 explicit reference
- Case-sensitive exact table identity; case-sensitive column identity within
  its table. Duplicate identities are rejected. Authored order is retained
- Table/column names: `[A-Za-z_][A-Za-z0-9_]{0,11}`. Type tokens:
  `[A-Za-z][A-Za-z0-9_]{0,11}`. Type names are display labels, not validated SQL
  types. Non-ASCII, XML metacharacters, controls and whitespace are rejected
- `flags` contains unique members of `pk`, `unique`, `not null`, in authored
  order. Combinations must also fit the complete label budget. These become native valueless settings; they are display annotations,
  not database constraint validation
- References require two different existing tables and one existing column at
  each endpoint; no implicit/placeholder identities or column-port geometry
- `many-to-one`, `one-to-many`, `one-to-one`, `many-to-many` become native
  `N:1`, `1:N`, `1:1`, `N:N` labels. Every connector is directed from the authored
  `from` table card to the `to` table card; this is not crow's-foot ER notation
- Exact native composed column labels must fit 18 display cells; reference
  labels must fit 11. The 12-byte table bound is below the native title cap of
  31. Native column formatting collapses its two separator spaces to one
- Schemas, aliases, named refs, source spans, projects, enums, notes, groups,
  partials, indexes, checks, inline refs, arbitrary settings and valued flags
  are outside this profile and rejected rather than omitted

Limits before rendering: 64 KiB raw input, JSON depth 16, 2,048 JSON value nodes,
4 KiB pin argument, 4,096 UTF-8 bytes per explicit path. Standard JSON only,
without BOM, JSON5, trailing values or non-finite numbers. The composed-label
cross-field checks are runtime constraints in addition to the published schema.
The bounded model limits the native scene before output construction; completed
SVG is capped at 256 KiB and receipt at 16 KiB. These are artifact caps, not
process-memory or execution-time isolation.

## Rendering and fidelity

`--theme` accepts the fourteen canonical native presets plus legacy `light`
(default) and `dark`. Omitted `--background` is transparent; the only admitted
painted background is literal `#RRGGBB` (hex letter case preserved). A theme
never paints the canvas. No arbitrary palette, font, paint URL or embedded
resource is accepted. Every returned native warning is fatal before staging.
An empty warning list does not establish complete DBML or database semantics.

The adapter constructs the pinned parser's typed AST with synthetic zero spans;
it does not invoke the DBML source parser. Geometry and SVG generation stay in
the native renderer. This is a native table-card view with display-width label
limits, not measured-font fitting. Native font-family strings are resolved by
the eventual viewer; fonts are neither embedded nor pinned. Glyph coverage,
measured shaping and cross-machine pixel parity are not asserted. The conservative 12-byte tokens, 18-cell column labels and 11-cell reference
labels also address actual wide-glyph overflow found during local review. The
older 24-W title/body and 18-cell W-heavy reference limit visibly overflowed
cards/pills and are now rejected. The accepted
[`wide-labels.json`](../examples/dbml-authored/wide-labels.json) fixture exercises
maximum W/M titles and column content plus the worst-case admitted W-heavy
relationship label. Local Inkscape 1.4 inspection shows them fitting; this does
not certify every viewer/font. Same-build SVG byte determinism is tested
separately from local visual inspection.

## Receipt and publication

The mandatory closed typed
[`dbml.render-receipt/v1`](../schemas/dbml-render-receipt-v1.schema.json) includes:

- Profile, provider/engine versions, native renderer-code baseline revision,
  exact parser and theme dependency revisions
- `plot.artifact-receipt-core/v1` bindings for exact original input bytes and
  actual SVG bytes, stable `figure` artifact ID, `primary` role, `output`
  argument and `svg` kind
- Actual dimensions, admitted table/column/ref counts, theme/background,
  fail-closed warning policy and actual native warning vector
- Explicit fidelity limitations; no generated or caller filesystem paths

The `renderer_code` revision identifies the unchanged native library source
used here, not the later provider commit. Parser identity describes the linked
AST model dependency, not a source-parse event. Maintainers must update this
provenance and parity evidence when native renderer/dependency code changes.

IO, path checks and pair staging follow the established Graph IR provider:
input raw hash/bytes, file identity and bundle directory are rechecked before
publication; aliases between input/output/receipt/executable are refused,
including symlinks, hardlinks and normalized missing-parent paths. Inputs must
be regular files directly within their explicit bundle directory. Destination
nonregular files are rejected. All parsing/render/receipt validation precedes
output-directory creation.

Both outputs stage in private same-parent directories and are verified before
publication. Ordinary destination replacement uses rename; all rollback
material is prepared first. A handled failure restores earlier destinations;
existing hardlinked destinations are updated/rolled back in place to preserve
shared identity. This is best-effort pair rollback, **not a crash-atomic pair
transaction, lock or defense against concurrent/adversarial filesystem races**.
Rollback failures report and retain recovery material. Parent directories created
during a staging attempt may remain empty. Shell redirections are outside this
contract; use the explicit output flags.

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
python3 scripts/validate-dbml-provider.py target/debug/plot-provider-dbml
```

The final command additionally requires Python `jsonschema` and performs no
installation/network access. Rust tests cover exact native source-parser/CLI
SVG parity in all 16 themes and transparent/painted backgrounds, deterministic
replay, semantic changes, closed schema and bounds, pins/path aliases, preserved
outputs, input identity drift, real native warning refusal, staging failures and
pair rollback. The schema gate checks descriptor-driven invocation, the complete
receipt schema, source/output hashes and static SVG structure. The source oracle
[`users-posts.dbml`](../examples/dbml-authored/users-posts.dbml) is a test fixture,
not an alternate provider input format.

Hub admission is a separate unit: the consumer must fix the actual provider
executable and descriptor identity, use complete receipt-core feature preflight
from the same execution binary/server, stage and verify both artifacts, and
exercise CLI, generic MCP and expanded tools. Generic receipt-core success
proves artifact byte relations; it does not certify provider DBML semantics.

See the [dated validation report](verification/dbml-provider.md) for the local
execution and visual boundaries actually checked.
