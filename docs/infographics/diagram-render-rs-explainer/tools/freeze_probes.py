#!/usr/bin/env python3
"""One-time guardrail probes (supplementary frozen evidence).

Empirically measures the raster guardrails, escaping, transparency, the
dependency inventory, and the no-unsafe policy, writing a single additional
frozen file. Refuses to overwrite. Never touches existing frozen evidence.

Usage:
    python3 tools/freeze_probes.py --engine /path/to/diagram-render-rs [--tree .]
"""

from __future__ import annotations

import argparse
import re
import struct
import subprocess
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import FORMATS, GateError, guard_engine, resolve_tree  # noqa: E402

BINARY = Path("/tmp/ign-drr/target-frozen/release/diagram-render-rs")
SCRATCH = Path("/tmp/ign-drr/probes")


def run(cmd: list[str], stdin: str | None = None, cwd: Path | None = None):
    proc = subprocess.run(
        cmd, input=stdin, capture_output=True, text=True, cwd=str(cwd) if cwd else None
    )
    return proc.returncode, (proc.stdout + proc.stderr).strip()


def first_pixel_alpha(png_path: Path) -> int:
    data = png_path.read_bytes()
    pos = 8
    idat = b""
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        if kind == b"IDAT":
            idat += data[pos + 8 : pos + 8 + length]
        pos += 12 + length
    raw = zlib.decompress(idat)
    # First scanline: 1 filter byte + 4 bytes per pixel. For pixel 0 every
    # predictor is zero, so raw[4] is the alpha regardless of filter type.
    return raw[4]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", default=None)
    parser.add_argument("--engine", default=None)
    args = parser.parse_args()

    tree = resolve_tree(args.tree)
    engine = guard_engine(args.engine)
    target = tree / "data" / "frozen" / "guardrail-probes.txt"
    if target.exists():
        raise GateError("guardrail-probes.txt already exists; frozen is never rewritten")
    if not BINARY.is_file():
        raise GateError(f"frozen build binary missing: {BINARY}")

    SCRATCH.mkdir(parents=True, exist_ok=True)
    lines: list[str] = []
    lines.append("# One-time guardrail probes against the frozen build (never regenerated)")
    lines.append("# every fact below is a real CLI invocation or byte inspection")

    def probe(label: str, cmd_note: str, cmd: list[str], stdin: str | None = None,
              cwd: Path | None = None):
        rc, out = run(cmd, stdin=stdin, cwd=cwd)
        lines.append(f"")
        lines.append(f"## {label}")
        lines.append(f"$ {cmd_note}")
        lines.append(out)
        lines.append(f"exit-code: {rc}")
        return rc, out

    rc, _ = probe(
        "P1 raster scale upper bound rejected",
        "echo 'a -> b' | diagram-render-rs - --format d2 -T png -o p1.png --scale 100",
        [str(BINARY), "-", "--format", "d2", "-T", "png", "-o", str(SCRATCH / "p1.png"), "--scale", "100"],
        stdin="a -> b",
    )
    if rc == 0:
        raise GateError("P1 unexpectedly succeeded")

    rc, _ = probe(
        "P2 raster scale lower bound rejected",
        "echo 'a -> b' | diagram-render-rs - --format d2 -T png -o p2.png --scale 0.01",
        [str(BINARY), "-", "--format", "d2", "-T", "png", "-o", str(SCRATCH / "p2.png"), "--scale", "0.01"],
        stdin="a -> b",
    )
    if rc == 0:
        raise GateError("P2 unexpectedly succeeded")

    rc, _ = probe(
        "P3 per-dimension pixel cap rejected (wide scene at scale 16)",
        "printf 'box; right; box; right; ... x10' | diagram-render-rs - --format pikchr -T png -o p3.png --scale 16",
        [str(BINARY), "-", "--format", "pikchr", "-T", "png", "-o", str(SCRATCH / "p3.png"), "--scale", "16"],
        stdin="box; right; " * 9 + "box",
    )
    if rc == 0:
        raise GateError("P3 unexpectedly succeeded")

    rc, _ = probe(
        "P4 total-pixel cap rejected (schema fixture at scale 16)",
        "diagram-render-rs schema.dbml --format dbml -o p4.png --scale 16",
        [str(BINARY), "schema.dbml", "--format", "dbml", "-o", str(SCRATCH / "p4.png"), "--scale", "16"],
        cwd=engine / "examples" / "inputs",
    )
    if rc == 0:
        raise GateError("P4 unexpectedly succeeded")

    rc, _ = probe(
        "P5 exact PNG width honored",
        "diagram-render-rs architecture.d2 --format d2 -o p5.png --width 640 --quiet",
        [str(BINARY), "architecture.d2", "--format", "d2", "-o", str(SCRATCH / "p5.png"), "--width", "640", "--quiet"],
        cwd=engine / "examples" / "inputs",
    )
    if rc != 0:
        raise GateError("P5 failed")
    p5 = (SCRATCH / "p5.png").read_bytes()
    w = int.from_bytes(p5[16:20], "big")
    h = int.from_bytes(p5[20:24], "big")
    lines.append(f"p5-png-dimensions: {w}x{h} (requested width 640)")

    rc, out = probe(
        "P6 explicit canvas paint emitted",
        "echo 'a -> b' | diagram-render-rs - --format d2 -o p6.svg --background '#ffffff' --quiet",
        [str(BINARY), "-", "--format", "d2", "-o", str(SCRATCH / "p6.svg"), "--background", "#ffffff", "--quiet"],
        stdin="a -> b",
    )
    svg6 = (SCRATCH / "p6.svg").read_text()
    lines.append(f"canvas-fill-element-present: {'data-canvas-background' in svg6}")

    rc, _ = probe(
        "P7 SVG text is XML-escaped",
        "echo 'a: \"<unsafe & visible>\"' | diagram-render-rs - --format d2 -o p7.svg --quiet",
        [str(BINARY), "-", "--format", "d2", "-o", str(SCRATCH / "p7.svg"), "--quiet"],
        stdin='a: "<unsafe & visible>"',
    )
    svg7 = (SCRATCH / "p7.svg").read_text()
    lines.append(f"escaped-entity-present: {'&lt;unsafe &amp; visible&gt;' in svg7}")
    lines.append(f"raw-angle-bracket-present: {'>unsafe & visible<' in svg7}")

    lines.append("")
    lines.append("## P8 frozen PNG canvases keep a transparent corner pixel")
    for fmt, _fixture in FORMATS:
        alpha = first_pixel_alpha(tree / "data" / "frozen" / "artifacts" / f"{fmt}.png")
        lines.append(f"{fmt}.png first-pixel-alpha: {alpha}")
        if alpha != 0:
            raise GateError(f"{fmt}.png corner pixel is not transparent")

    lines.append("")
    lines.append("## P9 unsafe scan over engine sources")
    rc, out = run(["grep", "-rn", "unsafe", str(engine / "src")])
    hits = out.strip()
    n_hits = len(hits.splitlines()) if hits else 0
    lines.append(f"grep -rn unsafe src -> {n_hits} hits")
    lines.append(hits if hits else "(no matches)")

    lines.append("")
    lines.append("## P10 dependency inventory from the engine manifest")
    import tomllib

    with (engine / "Cargo.toml").open("rb") as handle:
        manifest = tomllib.load(handle)
    deps = manifest["dependencies"]
    for name, spec in deps.items():
        if isinstance(spec, str):
            lines.append(f"dep: {name}: crates.io {spec}")
        elif "git" in spec:
            lines.append(f"dep: {name}: git {spec['git']} rev {spec['rev']}")
        else:
            lines.append(f"dep: {name}: crates.io {spec.get('version', '?')}")
    lines.append(f"dependency-count: {len(deps)}")

    target.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {target.relative_to(tree)} ({len(lines)} lines)")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"PROBES FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
