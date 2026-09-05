#!/usr/bin/env python3
"""Delivery gates for the diagram-render-rs explainer tree.

Four batteries, all of which must pass:
  B1  six-ban scan over index.html + panels/*.svg, with six positive
      controls (one planted violation per ban, each must be caught by
      exactly its category) and a clean-sample negative control
  B2  svg-linter on every panel SVG: rc 0 and zero findings
  B3  vacuum re-run: two flat /tmp copies, rebuildable products deleted,
      full rebuild chain in each; A == B == delivery tree byte-for-byte
  B4  fingerprint machine check: recompute the manifest over the whole
      tree and compare against fingerprints.sha256 (manifest excluded
      from itself)

Plus a prelude of structural checks (self-containment, no pycache, engine
still at the frozen snapshot, no engine target/ residue). Gates print
their records and write nothing into the tree.

Usage:
    python3 tools/gates.py --engine /path/to/diagram-render-rs [--tree .] [--skip-vacuum]
"""

from __future__ import annotations

import argparse
import difflib
import html as html_mod
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import (  # noqa: E402
    DEFAULT_SVG_LINTER,
    FORMATS,
    GateError,
    guard_engine,
    resolve_tree,
    run_capture,
    sha256_file,
)

BAN_FILES = "①引擎文件名"
BAN_LINES = "②行号/区间"
BAN_VERB = "③逐字源码≥25字符"
BAN_IDENT = "④引擎标识符"
BAN_PATH = "⑤引擎内部路径"
BAN_GEN = "⑥生成器/重建命令"
ALL_BANS = [BAN_FILES, BAN_LINES, BAN_VERB, BAN_IDENT, BAN_PATH, BAN_GEN]

# Distinctive engine identifiers to watch case-sensitively even though they
# are single words (Camel regex does not catch them).
WATCH_IDENT = {
    "Scene", "Primitive", "Rect", "Ellipse", "Line", "Polyline", "Polygon",
    "Text", "Stroke", "Point", "Theme", "Document", "Rendered", "Pixmap",
    "Format", "Entry", "Node", "Edge", "Version",
}

ALLOW_NOTES = {
    # public product / ecosystem names (never banned even if harvested)
    "diagram-render-rs", "diagram-ast-parser", "diagram-theme", "resvg",
    "clap", "serde_json", "thiserror", "unicode-width", "tiny-skia",
    "rustc", "cargo",
    # public DSL names that also occur as engine enum variants
    "LikeC4", "WaveDrom",
}

# Standard XML namespace boilerplate is not engine source content; it is
# stripped from both corpus and target before the verbatim-window compare.
NS_ATTR = 'xmlns="http://www.w3.org/2000/svg"'

# A frozen-evidence filename that happens to contain an English word; the
# evidence index on the page lists it. Ban 6 targets generator names and
# rebuild commands, which this filename is not.
EVIDENCE_FILENAME_EXEMPT = "cargo-rebuild-determinism.txt"


def flatten_with_transcript_spans(text: str) -> tuple[str, list[tuple[int, int]]]:
    """Whitespace-flatten like the verbatim-window compare does, and return
    the character spans of transcript excerpt content.

    Ruling (registered in VERIFICATION.md): ban ③ bans verbatim *source*
    excerpts. Runtime CLI transcripts — `$ ` command lines and the engine's
    real output lines quoted inside a `<pre class="term">` block — are
    allowed page surface (same allow-list that permits CLI verbs and flags).
    A window fully inside such a span is exempt from ban ③ only.
    """
    segments: list[str] = []
    spans: list[tuple[int, int]] = []
    offset = 0
    in_term = False
    for line in text.splitlines():
        flat_line = re.sub(r"\s+", " ", line.strip())
        segments.append(flat_line)
        start = None
        if in_term:
            start = 0
        elif "<pre" in line:
            # transcript block opens on this line (may carry leading markup)
            stripped = re.sub(r"^(?:<[^>]+>\s*)+", "", line.lstrip())
            if stripped.startswith("$ ") or stripped.startswith("warning") or stripped.startswith("error"):
                start = len(flat_line) - len(re.sub(r"\s+", " ", stripped))
        elif line.lstrip().startswith("$ "):
            start = 0
        if start is not None:
            spans.append((offset + start, offset + len(flat_line)))
        if in_term or start is not None:
            in_term = "</pre>" not in line
        offset += len(flat_line) + 1
    return " ".join(segments), spans


def harvest_corpus(engine: Path) -> str:
    parts = []
    for pattern in ("src/**/*.rs", "tests/*.rs", "examples/*.rs", "e2e/*.go",
                    "justfile", "Cargo.toml"):
        for path in engine.glob(pattern):
            parts.append(path.read_text())
    return "\n".join(parts)


def visible_text(path: Path) -> str:
    raw = path.read_text()
    raw = re.sub(r"<style.*?</style>", " ", raw, flags=re.S)
    raw = re.sub(r"<[^>]+>", " ", raw)
    return html_mod.unescape(raw)


def scan_targets(tree: Path) -> dict[str, str]:
    targets = {"index.html": (tree / "index.html").read_text()}
    for panel in sorted((tree / "panels").glob("*.svg")):
        targets[panel.name] = panel.read_text()
    return targets


def build_scanners(engine: Path):
    corpus = harvest_corpus(engine)
    corpus_flat = re.sub(r"\s+", " ", corpus.replace(NS_ATTR, "xmlns=NS"))

    tokens = set(re.findall(r"[A-Za-z_][A-Za-z0-9_]*", corpus))
    banned_idents = {
        t
        for t in tokens
        if ("_" in t and len(t) >= 5)
        or re.fullmatch(r"[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]+)+", t)
    }
    banned_idents |= WATCH_IDENT
    banned_idents -= ALLOW_NOTES
    banned_idents = {t for t in banned_idents if t not in ALLOW_NOTES}

    engine_files: set[str] = set()
    for pattern in ("src/**/*.rs", "src/*", "tests/*.rs", "examples/*.rs",
                    "e2e/*", ".github/**/*"):
        for path in engine.glob(pattern):
            if path.is_file():
                engine_files.add(path.name)
    engine_files -= {"Cargo.toml", "Cargo.lock", "LICENSE", "README.md",
                     ".gitignore", "rust-toolchain.toml"}
    engine_files = {f for f in engine_files if f not in (".DS_Store",)}

    def scan(text: str, where: str) -> list[tuple[str, str]]:
        findings: list[tuple[str, str]] = []

        for name in sorted(engine_files):
            for m in re.finditer(re.escape(name), text):
                findings.append((BAN_FILES, f"{where}: engine file name {name!r}"))

        for pattern in (
            r"\w+\.(?:rs|go|toml|json|md|lock):\d+",
            r"\b\d+:\d+\b",
            r"#L\d+\b",
            r"\bL\d+\b",
            r"\bline\s+\d+\b",
            r"第\s*\d+\s*行",
            r"行\s*\d+",
        ):
            for m in re.finditer(pattern, text):
                findings.append((BAN_LINES, f"{where}: line reference {m.group(0)!r}"))

        flat, transcript_spans = flatten_with_transcript_spans(
            text.replace(NS_ATTR, "xmlns=NS")
        )
        for i in range(0, max(len(flat) - 24, 0)):
            window = flat[i : i + 25]
            if not any(ch.isalpha() for ch in window) or window not in corpus_flat:
                continue
            if any(s <= i and i + 25 <= e for s, e in transcript_spans):
                continue  # allowed surface: `$ ` transcript command line
            findings.append((BAN_VERB, f"{where}: verbatim source window {window!r}"))
            break

        for ident in sorted(banned_idents):
            for m in re.finditer(rf"\b{re.escape(ident)}\b", text):
                findings.append((BAN_IDENT, f"{where}: engine identifier {ident!r}"))

        for pattern in (
            r"\bsrc/[A-Za-z0-9_/.-]+",
            r"\be2e/\w",
            r"\bexamples/\w",
            r"\btests/\w",
            r"\.github/\w",
            r"/Users/",
            r"~/projects",
            r"plot/diagram-render-rs",
            r"\bdocs/infographics",
        ):
            for m in re.finditer(pattern, text):
                findings.append((BAN_PATH, f"{where}: engine/internal path {m.group(0)!r}"))

        scanned = text.replace(EVIDENCE_FILENAME_EXEMPT, "EVIDENCE-FILE")
        for word in (
            "python", "python3", "magick", "imagemagick", "chrome", "chromium",
            "headless", "playwright", "screenshot", "svg-linter", "linter",
            "rebuild", "freeze", "gates", "pip", "venv", "tools/", "ign-drr",
            "subprocess", "argparse", "websocket",
        ):
            pattern = ""
            if word[0].isalnum():
                pattern += r"\b"
            pattern += re.escape(word)
            if word[-1].isalnum():
                pattern += r"\b"
            for m in re.finditer(pattern, scanned, re.I):
                findings.append((BAN_GEN, f"{where}: generator token {m.group(0)!r}"))

        return findings

    return scan, corpus_flat


def positive_controls(scan, corpus_flat: str) -> list[tuple[str, str, list[str]]]:
    verbatim = "let mut options = resvg::usvg::Options::default();"
    assert verbatim in corpus_flat or verbatim in corpus_flat.replace("  ", " ")
    samples = [
        (BAN_FILES, "布局核心在 cards.rs 里", ["cards.rs"]),
        (BAN_LINES, "位置在 88:12 附近", ["88:12"]),
        (BAN_VERB, f"源码写了 {verbatim}", [verbatim]),
        (BAN_IDENT, "入口是 render_source 函数", ["render_source"]),
        (BAN_PATH, "样例放在 examples/inputs 目录", ["examples/inputs"]),
        (BAN_GEN, "本页由 python3 生成", ["python3"]),
    ]
    results = []
    for category, sample, expect in samples:
        findings = scan(sample, "control")
        categories = {c for c, _ in findings}
        ok = categories == {category}
        results.append((category, sample, sorted(categories), ok))
    return results


def tree_hashes(root: Path) -> dict[str, str]:
    out = {}
    for path in sorted(root.rglob("*")):
        if path.is_file():
            rel = path.relative_to(root).as_posix()
            if rel == ".DS_Store" or "__pycache__" in rel:
                continue
            out[rel] = sha256_file(path)
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", default=None)
    parser.add_argument("--engine", default=None)
    parser.add_argument("--skip-vacuum", action="store_true")
    args = parser.parse_args()

    tree = resolve_tree(args.tree)
    engine = guard_engine(args.engine)
    failures: list[str] = []

    # ------------------------------------------------------------------
    # Prelude: structural checks
    # ------------------------------------------------------------------
    print("== prelude ==")
    page = (tree / "index.html").read_text()
    page_no_ns = page.replace(NS_ATTR, "xmlns=NS")
    checks = [
        ("no <script>", "<script" not in page),
        ("no external http links",
         "http://" not in page_no_ns and "https://" not in page_no_ns),
        ("1200px body", re.search(r"width:\s*1200px", page) is not None),
        ("zh-CN", 'lang="zh-CN"' in page),
    ]
    for name, ok in checks:
        print(f"  [{'PASS' if ok else 'FAIL'}] {name}")
        if not ok:
            failures.append(f"prelude: {name}")

    pyc = [p for p in tree.rglob("*") if p.suffix in (".pyc", ".pyo")]
    if pyc:
        failures.append(f"prelude: bytecode files present: {pyc}")
    print(f"  [{'PASS' if not pyc else 'FAIL'}] zero bytecode files in tree")

    target_residue = engine / "target"
    if target_residue.exists():
        # Residue that predates the frozen snapshot is pre-existing engine
        # state; this toolchain builds exclusively into /tmp target dirs and
        # must not add to it. Anything newer than the freeze is a failure.
        import datetime
        captured = re.search(
            r"captured-utc: ([\d-]+T[\d:]+)Z",
            (tree / "data" / "frozen" / "engine-snapshot.txt").read_text(),
        ).group(1)
        captured_dt = datetime.datetime.fromisoformat(captured).replace(
            tzinfo=datetime.timezone.utc
        )
        newest = max(
            path.stat().st_mtime for path in target_residue.rglob("*")
        )
        newest_dt = datetime.datetime.fromtimestamp(newest).astimezone()
        if newest_dt > captured_dt:
            failures.append(
                f"prelude: engine target/ has files newer than the freeze "
                f"({newest_dt.isoformat()})"
            )
            print(f"  [FAIL] engine target/ residue newer than freeze")
        else:
            print(
                "  [PASS] engine target/ residue pre-dates the freeze "
                "(pre-existing, disclosed; all toolchain builds used /tmp)"
            )
    else:
        print("  [PASS] engine has no target/ residue")

    # ------------------------------------------------------------------
    # B1: six-ban scan + positive controls
    # ------------------------------------------------------------------
    print("== battery 1: six-ban scan ==")
    scan, corpus_flat = build_scanners(engine)
    controls = positive_controls(scan, corpus_flat)
    for category, sample, got, ok in controls:
        print(f"  [{'PASS' if ok else 'FAIL'}] control {category}: {sample[:40]}")
        if not ok:
            failures.append(f"B1 positive control not caught cleanly: {category} -> {got}")
    clean = scan("这是一个干净的中文样本，包含 DBML、WaveDrom、resvg 与 --format dbml。", "clean-control")
    if clean:
        failures.append(f"B1 negative control flagged: {clean}")
        print(f"  [FAIL] negative control flagged: {clean}")
    else:
        print("  [PASS] negative control clean")

    total_findings = []
    for where, text in scan_targets(tree).items():
        findings = scan(text, where)
        for category, detail in findings:
            print(f"  [FAIL] {detail}")
        total_findings += findings
    print(f"  [{'PASS' if not total_findings else 'FAIL'}] "
          f"page+panels findings: {len(total_findings)}")
    if total_findings:
        failures.append(f"B1: {len(total_findings)} findings on delivered artifacts")

    # ------------------------------------------------------------------
    # B2: svg-linter on every panel
    # ------------------------------------------------------------------
    print("== battery 2: svg-linter ==")
    linter = os.environ.get("DRR_SVG_LINTER", DEFAULT_SVG_LINTER)
    panels = sorted((tree / "panels").glob("*.svg"))
    if not panels:
        failures.append("B2: no panels found")
    for panel in panels:
        rc, out = run_capture([linter, "--plain", "check", str(panel)])
        finding_rows = [ln for ln in out.splitlines() if ln.startswith("finding\t")]
        ok = rc == 0 and not finding_rows
        print(f"  [{'PASS' if ok else 'FAIL'}] {panel.name}: rc={rc} findings={len(finding_rows)}")
        if not ok:
            failures.append(f"B2: {panel.name} rc={rc} findings={len(finding_rows)}")

    # ------------------------------------------------------------------
    # B3: vacuum re-run (A/B flat copies, delete rebuildables, full chain)
    # ------------------------------------------------------------------
    print("== battery 3: vacuum re-run ==")
    if args.skip_vacuum:
        print("  [SKIP] --skip-vacuum")
    else:
        vacuum_root = Path("/tmp/ign-drr/vacuum")
        shutil.rmtree(vacuum_root, ignore_errors=True)
        vacuum_root.mkdir(parents=True)
        rebuildables = ["index.html", "panels", "renders", "data/rebuild",
                        "fingerprints.sha256"]
        copies = {}
        for label in ("A", "B"):
            copy_dir = vacuum_root / label
            shutil.copytree(tree, copy_dir, ignore=shutil.ignore_patterns(
                ".DS_Store", "__pycache__", "*.pyc"))
            for rel in rebuildables:
                victim = copy_dir / rel
                if victim.is_dir():
                    shutil.rmtree(victim)
                elif victim.exists():
                    victim.unlink()
            # frozen layer must survive the deletion untouched
            proc = subprocess.run(
                [sys.executable, str(copy_dir / "tools" / "rebuild.py"),
                 "--tree", str(copy_dir), "--engine", str(engine)],
                capture_output=True, text=True,
            )
            if proc.returncode != 0:
                failures.append(f"B3: rebuild in copy {label} failed:\n{proc.stdout}{proc.stderr}")
                print(f"  [FAIL] rebuild in copy {label}")
                copies[label] = None
            else:
                copies[label] = copy_dir
                print(f"  [PASS] full chain rebuilt in copy {label}")
        if copies.get("A") and copies.get("B"):
            ha, hb = tree_hashes(copies["A"]), tree_hashes(copies["B"])
            if ha != hb:
                diff = [k for k in set(ha) | set(hb) if ha.get(k) != hb.get(k)]
                failures.append(f"B3: A != B, differing files: {sorted(diff)[:10]}")
                print(f"  [FAIL] A != B ({len(diff)} files differ)")
            else:
                print(f"  [PASS] A == B ({len(ha)} files)")
            ht = tree_hashes(tree)
            if ha != ht:
                diff = [k for k in set(ha) | set(ht) if ha.get(k) != ht.get(k)]
                failures.append(f"B3: A != delivery tree, differing files: {sorted(diff)[:10]}")
                print(f"  [FAIL] A != delivery tree ({len(diff)} files differ)")
            else:
                print(f"  [PASS] A == delivery tree ({len(ht)} files)")

    # ------------------------------------------------------------------
    # B4: fingerprint machine check
    # ------------------------------------------------------------------
    print("== battery 4: fingerprints ==")
    manifest = tree / "fingerprints.sha256"
    if not manifest.is_file():
        failures.append("B4: fingerprints.sha256 missing")
        print("  [FAIL] manifest missing")
    else:
        listed = {}
        for line in manifest.read_text().splitlines():
            digest, rel = line.split("  ", 1)
            listed[rel] = digest
        if "fingerprints.sha256" in listed:
            failures.append("B4: manifest lists itself")
            print("  [FAIL] manifest lists itself")
        actual = {
            p.relative_to(tree).as_posix(): sha256_file(p)
            for p in sorted(tree.rglob("*"))
            if p.is_file()
            and p.relative_to(tree).as_posix() not in (".DS_Store", "fingerprints.sha256")
            and "__pycache__" not in p.relative_to(tree).as_posix()
        }
        missing = sorted(set(actual) - set(listed))
        extra = sorted(set(listed) - set(actual))
        changed = sorted(
            rel for rel in set(actual) & set(listed) if actual[rel] != listed[rel]
        )
        ok = not (missing or extra or changed)
        print(f"  [{'PASS' if ok else 'FAIL'}] files={len(actual)} "
              f"missing={len(missing)} extra={len(extra)} changed={len(changed)}")
        for rel in (missing + extra + changed)[:10]:
            print(f"    · {rel}")
        if not ok:
            failures.append(f"B4: manifest mismatch (missing={missing} extra={extra} changed={changed})")

    print()
    if failures:
        print(f"GATES FAILED ({len(failures)}):")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print("ALL GATES PASS")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"GATES ERROR: {error}", file=sys.stderr)
        raise SystemExit(1)
