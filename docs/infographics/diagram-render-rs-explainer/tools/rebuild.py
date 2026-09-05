#!/usr/bin/env python3
"""Deterministic rebuild chain for the diagram-render-rs explainer tree.

Steps (all guarded against engine drift):
  1. verify the engine is at the frozen HEAD and clean outside this tree
  2. cargo build --offline --release into the scratch target dir and assert
     the binary hash equals the frozen binary hash
  3. re-render the seven fixtures, normalize the transcript (<work>/<dur>/<ts>),
     and assert every SVG/PNG byte hash matches the frozen artifacts
  4. regenerate panels/*.svg and index.html from frozen evidence
  5. regenerate renders/ via the slice screenshot pipeline
  6. rewrite fingerprints.sha256 over the whole tree (excluding itself)

Running this tool twice must leave every produced file byte-identical.

Usage:
    python3 tools/rebuild.py --engine /path/to/diagram-render-rs [--tree .]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import (  # noqa: E402
    DEFAULT_TARGET_DIR,
    FORMATS,
    GateError,
    guard_engine,
    normalize_transcript,
    png_dimensions,
    resolve_tree,
    run_capture,
    sha256_bytes,
    sha256_file,
    svg_stats,
)

WORK = Path("/tmp/ign-drr/rebuild-work")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", default=None)
    parser.add_argument("--engine", default=None)
    parser.add_argument("--skip-shots", action="store_true",
                        help="internal: skip screenshot regeneration")
    args = parser.parse_args()

    tree = resolve_tree(args.tree)
    engine = guard_engine(args.engine)
    frozen = tree / "data" / "frozen"
    rebuild = tree / "data" / "rebuild"
    rebuild.mkdir(parents=True, exist_ok=True)

    # ------------------------------------------------------------------
    # 1-2. Engine build, binary pinned to the frozen hash
    # ------------------------------------------------------------------
    target_dir = Path(
        os.environ.get("DRR_CARGO_TARGET_DIR", DEFAULT_TARGET_DIR)
    ).expanduser()
    target_dir.parent.mkdir(parents=True, exist_ok=True)
    build = subprocess.run(
        ["cargo", "build", "--offline", "--release"],
        cwd=str(engine),
        env={**os.environ, "CARGO_TARGET_DIR": str(target_dir)},
        capture_output=True,
        text=True,
    )
    if build.returncode != 0:
        raise GateError("cargo build failed:\n" + build.stdout + build.stderr)
    binary = target_dir / "release" / "diagram-render-rs"
    bin_sha = sha256_file(binary)
    frozen_build = (frozen / "cargo-build-release.txt").read_text()
    frozen_bin_sha = frozen_build.split("binary-sha256: ", 1)[1].splitlines()[0]
    if bin_sha != frozen_bin_sha:
        raise GateError(
            f"rebuilt binary drifted: {bin_sha} != frozen {frozen_bin_sha}"
        )

    # ------------------------------------------------------------------
    # 3. Re-render the seven fixtures and compare against frozen bytes
    # ------------------------------------------------------------------
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    transcript = [
        "# Deterministic re-render of the seven fixtures (normalized transcript)",
        "# tokens: <work> scratch dir, <dur> durations, <ts> timestamps",
    ]
    frozen_stats = json.loads((frozen / "scene-stats.json").read_text())
    frozen_by_format = {a["format"]: a for a in frozen_stats["artifacts"]}
    stats: list[dict] = []
    for fmt, fixture in FORMATS:
        for kind in ("svg", "png"):
            out_path = WORK / f"{fmt}.{kind}"
            rc, out = run_capture(
                [str(binary), fixture, "--format", fmt, "-o", str(out_path)],
                cwd=engine / "examples" / "inputs",
            )
            transcript.append(f"$ diagram-render-rs {fixture} --format {fmt} -o {fmt}.{kind}")
            transcript.append(out.rstrip())
            transcript.append(f"exit-code: {rc}")
            if rc != 0:
                raise GateError(f"re-render failed: {fmt}.{kind}")
        svg_path = WORK / f"{fmt}.svg"
        png_path = WORK / f"{fmt}.png"
        svg_text = svg_path.read_text()
        png_bytes = png_path.read_bytes()
        w, h = png_dimensions(png_bytes)
        record = {
            "format": fmt,
            "fixture": fixture,
            "svg": {
                "bytes": svg_path.stat().st_size,
                "sha256": sha256_file(svg_path),
                **svg_stats(svg_text),
            },
            "png": {
                "bytes": png_path.stat().st_size,
                "sha256": sha256_file(png_path),
                "width": w,
                "height": h,
            },
        }
        stats.append(record)
        reference = frozen_by_format[fmt]
        for kind, product in (("svg", svg_path), ("png", png_path)):
            if record[kind]["sha256"] != reference[kind]["sha256"]:
                raise GateError(
                    f"engine drift: rebuilt {fmt}.{kind} hash differs from frozen"
                )
        transcript.append(
            f"facts: svg={record['svg']['bytes']:,}B sha={record['svg']['sha256'][:16]}… "
            f"scene={record['svg']['width']:.0f}x{record['svg']['height']:.0f} "
            f"png={record['png']['bytes']:,}B {w}x{h} sha={record['png']['sha256'][:16]}…"
        )
        transcript.append("")
    if json.dumps(stats, sort_keys=True) != json.dumps(
        frozen_stats["artifacts"], sort_keys=True
    ):
        raise GateError("structured scene stats drifted from the frozen record")

    (rebuild / "rerender-transcript.txt").write_text(
        normalize_transcript("\n".join(transcript) + "\n", [str(WORK)]),
        encoding="utf-8",
    )
    (rebuild / "scene-stats.json").write_text(
        json.dumps({"artifacts": stats}, ensure_ascii=False, indent=1) + "\n",
        encoding="utf-8",
    )
    (rebuild / "README.txt").write_text(
        "# data/rebuild — deterministic rebuild layer\n"
        "\n"
        "Everything here is regenerated by the rebuild tool and must be\n"
        "byte-identical across runs on this machine. Transcripts are\n"
        "normalized: <work> scratch directory, <dur> durations, <ts>\n"
        "timestamps. The rebuild asserts the seven re-rendered artifacts\n"
        "hash-match data/frozen/artifacts before writing anything here.\n"
        "\n"
        "Files:\n"
        "  rerender-transcript.txt   normalized re-render transcript\n"
        "  scene-stats.json          structured stats (equal to frozen values)\n"
    )

    # ------------------------------------------------------------------
    # 4. Panels + page from frozen evidence
    # ------------------------------------------------------------------
    proc = subprocess.run(
        [sys.executable, str(tree / "tools" / "page.py"), "--tree", str(tree)],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        raise GateError("page generation failed:\n" + proc.stdout + proc.stderr)

    # ------------------------------------------------------------------
    # 5. Screenshots
    # ------------------------------------------------------------------
    if not args.skip_shots:
        proc = subprocess.run(
            [sys.executable, str(tree / "tools" / "screenshot.py"),
             "--tree", str(tree)],
            capture_output=True, text=True,
        )
        if proc.returncode != 0:
            raise GateError("screenshot stage failed:\n" + proc.stdout + proc.stderr)

    # ------------------------------------------------------------------
    # 6. Fingerprint manifest over the whole tree (excluding itself)
    # ------------------------------------------------------------------
    manifest = tree / "fingerprints.sha256"
    lines = []
    for path in sorted(tree.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(tree).as_posix()
        if rel == "fingerprints.sha256":
            continue
        lines.append(f"{sha256_file(path)}  {rel}")
    manifest.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"rebuild complete: {len(lines)} files fingerprinted")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"REBUILD FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
