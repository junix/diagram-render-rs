#!/usr/bin/env python3
"""One-time frozen-evidence capture for the diagram-render-rs explainer.

Runs the real cargo build/test and the real CLI over the seven example
fixtures, transcribing everything into data/frozen/. Frozen evidence is
never overwritten: if any target file already exists the tool refuses.

Usage:
    python3 tools/freeze.py --engine /path/to/diagram-render-rs [--tree .]

Scratch space lives under /tmp/ign-drr (frozen target dirs, render
artifacts, determinism passes). The engine repository is guarded to the
frozen HEAD and must be clean outside this delivery tree.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import (  # noqa: E402
    FORMATS,
    GateError,
    guard_engine,
    png_dimensions,
    resolve_tree,
    run_capture,
    sha256_file,
    svg_stats,
)

SCRATCH = Path("/tmp/ign-drr")
FROZEN_TARGET = SCRATCH / "target-frozen"
FROZEN_TARGET_B = SCRATCH / "target-frozen-b"
ART = SCRATCH / "frozen-artifacts"
ART_B = SCRATCH / "frozen-artifacts-b"


def refuse_if_frozen(frozen: Path, names: list[str]) -> None:
    existing = [n for n in names if (frozen / n).exists()]
    if existing:
        raise GateError(
            f"frozen evidence already present, refusing to overwrite: {existing}"
        )


def cargo_env(target_dir: Path) -> dict:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    return env


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", default=None)
    parser.add_argument("--engine", default=None)
    args = parser.parse_args()

    tree = resolve_tree(args.tree)
    engine = guard_engine(args.engine)
    frozen = tree / "data" / "frozen"

    outputs = [
        "engine-snapshot.txt",
        "cargo-build-release.txt",
        "cargo-test.txt",
        "code-metrics.txt",
        "cli-surface.txt",
        "cli-render-transcript.txt",
        "scene-stats.json",
        "png-determinism.txt",
        "cargo-rebuild-determinism.txt",
        "feature-matrix-totals.txt",
        "README.txt",
    ]
    refuse_if_frozen(frozen, outputs)

    for scratch in (FROZEN_TARGET, FROZEN_TARGET_B, ART, ART_B):
        shutil.rmtree(scratch, ignore_errors=True)
    SCRATCH.mkdir(parents=True, exist_ok=True)
    ART.mkdir(parents=True)
    ART_B.mkdir(parents=True)

    # ------------------------------------------------------------------
    # 1. Engine snapshot
    # ------------------------------------------------------------------
    lines: list[str] = []
    lines.append("# One-time frozen engine snapshot (never regenerated).")
    lines.append(f"captured-utc: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}")
    head = subprocess.run(
        ["git", "-C", str(engine), "rev-parse", "HEAD"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    lines.append(f"engine-HEAD: {head}")
    porcelain = subprocess.run(
        ["git", "-C", str(engine), "status", "--porcelain"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    lines.append(
        "engine-porcelain-at-capture: "
        + (porcelain.replace("\n", " | ") if porcelain else "(clean)")
    )
    for label, cmd in [
        ("uname", ["uname", "-srm"]),
        ("macos", ["sw_vers", "-productVersion"]),
        ("rustc", ["rustc", "--version"]),
        ("cargo", ["cargo", "--version"]),
    ]:
        proc = subprocess.run(cmd, capture_output=True, text=True)
        value = proc.stdout.strip() or proc.stderr.strip()
        lines.append(f"{label}: {value}")
    lines.append(f"cargo-lock-sha256: {sha256_file(engine / 'Cargo.lock')}")
    (frozen / "engine-snapshot.txt").write_text("\n".join(lines) + "\n")

    # ------------------------------------------------------------------
    # 2. Real cargo release build, fresh target dir, full transcript
    # ------------------------------------------------------------------
    started = time.time()
    proc = subprocess.run(
        ["cargo", "build", "--offline", "--release"],
        cwd=str(engine),
        env=cargo_env(FROZEN_TARGET),
        capture_output=True,
        text=True,
    )
    took = time.time() - started
    binary = FROZEN_TARGET / "release" / "diagram-render-rs"
    build_text = ["# cargo build --offline --release (fresh target dir, full transcript)"]
    build_text.append(f"$ cargo build --offline --release")
    build_text.append(proc.stdout + proc.stderr)
    build_text.append(f"exit-code: {proc.returncode}")
    if proc.returncode != 0:
        (frozen / "cargo-build-release.txt").write_text("\n".join(build_text) + "\n")
        raise GateError("cargo build failed; frozen transcript kept for audit")
    build_text.append(f"wall-seconds: {took:.1f}")
    build_text.append(f"binary-sha256: {sha256_file(binary)}")
    build_text.append(f"binary-bytes: {binary.stat().st_size:,}")
    (frozen / "cargo-build-release.txt").write_text("\n".join(build_text) + "\n")

    # ------------------------------------------------------------------
    # 3. Real cargo test, full transcript
    # ------------------------------------------------------------------
    proc = subprocess.run(
        ["cargo", "test", "--all-targets", "--all-features", "--offline"],
        cwd=str(engine),
        env=cargo_env(FROZEN_TARGET),
        capture_output=True,
        text=True,
    )
    test_text = ["# cargo test --all-targets --all-features --offline (full transcript)"]
    test_text.append("$ cargo test --all-targets --all-features --offline")
    test_text.append(proc.stdout + proc.stderr)
    test_text.append(f"exit-code: {proc.returncode}")
    (frozen / "cargo-test.txt").write_text("\n".join(test_text) + "\n")

    # ------------------------------------------------------------------
    # 4. Code metrics at the snapshot
    # ------------------------------------------------------------------
    metrics = ["# wc -l over engine Rust sources at the frozen snapshot"]
    grand_total = 0
    src_file_count = 0
    for group in ("src", "tests", "examples", "e2e"):
        files = sorted((engine / group).rglob("*.rs"))
        if not files:
            continue
        total = 0
        metrics.append(f"[{group}]")
        for path in files:
            n = len(path.read_text().splitlines())
            total += n
            metrics.append(f"{n:6d} {path.relative_to(engine)}")
        metrics.append(f"{total:6d} TOTAL ({group})")
        if group == "src":
            src_file_count = len(files)
            grand_total += total
        elif group != "e2e":
            grand_total += total
    metrics.append(f"{grand_total:6d} TOTAL (src+tests+examples)")
    metrics.append(f"src-rust-file-count: {src_file_count}")
    (frozen / "code-metrics.txt").write_text("\n".join(metrics) + "\n")

    # ------------------------------------------------------------------
    # 5. CLI surface transcripts
    # ------------------------------------------------------------------
    surface = ["# Real CLI surface transcripts (release binary from the frozen build)"]
    for label, cmd in [
        ("version", [str(binary), "--version"]),
        ("help", [str(binary), "--help"]),
        ("themes", [str(binary), "themes"]),
        ("themes-json", [str(binary), "themes", "--json"]),
    ]:
        rc, out = run_capture(cmd)
        surface.append("$ diagram-render-rs " + " ".join(cmd[1:]))
        surface.append(out.rstrip())
        surface.append(f"exit-code: {rc}")
        surface.append("")
    (frozen / "cli-surface.txt").write_text("\n".join(surface) + "\n")

    # ------------------------------------------------------------------
    # 6. Real renders of the seven fixtures (SVG + PNG each)
    # ------------------------------------------------------------------
    render = ["# Real CLI renders: seven example fixtures, SVG + PNG (scale 2 default)"]
    stats: list[dict] = []
    for fmt, fixture in FORMATS:
        input_path = engine / "examples" / "inputs" / fixture
        for kind in ("svg", "png"):
            out_path = ART / f"{fmt}.{kind}"
            cmd = [
                str(binary), fixture, "--format", fmt,
                "-o", str(out_path),
            ]
            rc, out = run_capture(cmd, cwd=engine / "examples" / "inputs")
            render.append(f"$ diagram-render-rs {fixture} --format {fmt} -o {fmt}.{kind}")
            render.append(out.rstrip())
            render.append(f"exit-code: {rc}")
            if rc != 0:
                raise GateError(f"render failed for {fixture} ({kind})")
        svg_path = ART / f"{fmt}.svg"
        png_path = ART / f"{fmt}.png"
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
        render.append(
            f"facts: svg={record['svg']['bytes']:,}B sha={record['svg']['sha256'][:16]}… "
            f"scene={record['svg']['width']:.0f}x{record['svg']['height']:.0f} "
            f"png={record['png']['bytes']:,}B {w}x{h} sha={record['png']['sha256'][:16]}…"
        )
        render.append("")

    # Shared invalid-input rejection (real run).
    invalid = engine / "e2e" / "fixtures" / "invalid.dbml"
    rc, out = run_capture([str(binary), "invalid.dbml", "--format", "dbml"],
                          cwd=invalid.parent)
    render.append("$ diagram-render-rs invalid.dbml --format dbml")
    render.append(out.rstrip())
    render.append(f"exit-code: {rc} (expected non-zero)")
    if rc == 0:
        raise GateError("invalid.dbml unexpectedly rendered")
    (frozen / "cli-render-transcript.txt").write_text("\n".join(render) + "\n")

    # ------------------------------------------------------------------
    # 7. Structured scene stats + frozen artifact copies
    # ------------------------------------------------------------------
    (frozen / "scene-stats.json").write_text(
        json.dumps({"artifacts": stats}, ensure_ascii=False, indent=1) + "\n",
        encoding="utf-8",
    )
    art_dir = frozen / "artifacts"
    art_dir.mkdir()
    for fmt, _ in FORMATS:
        for kind in ("svg", "png"):
            src = ART / f"{fmt}.{kind}"
            shutil.copy2(src, art_dir / src.name)

    # ------------------------------------------------------------------
    # 8. Same-input determinism: second render pass, byte compare
    # ------------------------------------------------------------------
    det = ["# Same-input double render: pass B compared byte-wise against pass A"]
    det.append(f"$ cmp pass-A pass-B  (fresh process per render, distinct output dirs)")
    all_identical = True
    for fmt, fixture in FORMATS:
        row = [fmt]
        for kind in ("svg", "png"):
            out_path = ART_B / f"{fmt}.{kind}"
            rc, out = run_capture(
                [str(binary), fixture, "--format", fmt, "-o", str(out_path), "--quiet"],
                cwd=engine / "examples" / "inputs",
            )
            if rc != 0:
                raise GateError(f"pass-B render failed: {fmt}.{kind}")
            identical = (
                sha256_file(out_path) == sha256_file(ART / f"{fmt}.{kind}")
            )
            all_identical = all_identical and identical
            row.append(f"{kind}={'identical' if identical else 'DIFFERS'}")
        det.append(" ".join(row))
    det.append(f"summary: {'all byte-identical across two fresh processes' if all_identical else 'DIFFERENCES PRESENT — see rows above'}")
    (frozen / "png-determinism.txt").write_text("\n".join(det) + "\n")

    # ------------------------------------------------------------------
    # 9. Toolchain determinism: rebuild binary in a second fresh target dir
    # ------------------------------------------------------------------
    det2 = ["# cargo rebuild determinism: second fresh target dir, binary compared"]
    proc = subprocess.run(
        ["cargo", "build", "--offline", "--release"],
        cwd=str(engine),
        env=cargo_env(FROZEN_TARGET_B),
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise GateError("second build failed")
    binary_b = FROZEN_TARGET_B / "release" / "diagram-render-rs"
    sha_a = sha256_file(binary)
    sha_b = sha256_file(binary_b)
    det2.append(f"pass-A binary sha256: {sha_a}")
    det2.append(f"pass-B binary sha256: {sha_b}")
    det2.append(f"result: {'byte-identical binaries' if sha_a == sha_b else 'BINARIES DIFFER'}")
    (frozen / "cargo-rebuild-determinism.txt").write_text("\n".join(det2) + "\n")

    # ------------------------------------------------------------------
    # 10. Feature-matrix totals computed from the snapshot's own file
    # ------------------------------------------------------------------
    matrix = json.loads((engine / "e2e" / "feature_matrix.json").read_text())
    per_lang: dict[str, dict[str, int]] = {}
    for feature in matrix["features"]:
        lang = feature["language"]
        slot = per_lang.setdefault(lang, {"aligned": 0, "intentional-exclusion": 0})
        slot[feature["status"]] += 1
    fm = ["# Feature matrix totals computed from the engine snapshot"]
    fm.append("$ python summary of e2e/feature_matrix.json")
    aligned_total = 0
    excluded_total = 0
    for lang in sorted(per_lang):
        a = per_lang[lang]["aligned"]
        e = per_lang[lang]["intentional-exclusion"]
        aligned_total += a
        excluded_total += e
        fm.append(f"{lang:12s} aligned={a:2d} intentional-exclusion={e:2d}")
    fm.append(f"{'TOTAL':12s} aligned={aligned_total} intentional-exclusion={excluded_total}")
    fm.append(f"languages: {len(per_lang)}")
    (frozen / "feature-matrix-totals.txt").write_text("\n".join(fm) + "\n")

    # ------------------------------------------------------------------
    # 11. Frozen README
    # ------------------------------------------------------------------
    readme = """# data/frozen — one-time evidence (NEVER regenerated)

Everything here is a real measurement captured once against the engine
snapshot recorded in engine-snapshot.txt. The freeze tool refuses to run
if any of these files already exist. Re-verification of the numbers uses
the deterministic rebuild layer in ../rebuild/ plus the gates; it never
rewrites this directory.

Files:
  engine-snapshot.txt          HEAD, porcelain, toolchain, lockfile hash
  cargo-build-release.txt      full offline release-build transcript
  cargo-test.txt               full cargo test transcript
  code-metrics.txt             wc -l over engine sources at the snapshot
  cli-surface.txt              --version / --help / themes / themes --json
  cli-render-transcript.txt    seven-fixture render transcript + facts
  scene-stats.json             structured per-format sizes/hashes/counts
  png-determinism.txt          same-input double render, byte comparison
  cargo-rebuild-determinism.txt  second fresh-dir build, binary comparison
  feature-matrix-totals.txt    per-language aligned/excluded totals
  artifacts/                   the 14 frozen render products (7 svg + 7 png)
"""
    (frozen / "README.txt").write_text(readme)

    print("frozen evidence captured:")
    for name in outputs:
        print(f"  {name}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"FREEZE FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
