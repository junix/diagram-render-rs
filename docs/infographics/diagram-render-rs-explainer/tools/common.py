# Shared helpers for the diagram-render-rs explainer toolchain.
#
# Self-location: every tool takes an explicit `--tree` argument (preferred);
# when omitted, the tree root defaults to the parent directory of this file's
# `tools/` directory. The engine repository is passed with `--engine` or the
# DRR_ENGINE environment variable and is guarded before any work happens.

from __future__ import annotations

import hashlib
import os
import re
import subprocess
import sys
from pathlib import Path

FROZEN_HEAD = "b38ba079257a530691b8d2c700586fee5fb810ef"

TREE_REL = "docs/infographics/diagram-render-rs-explainer"

DEFAULT_ENGINE = os.environ.get("DRR_ENGINE", "")
DEFAULT_TARGET_DIR = os.environ.get(
    "DRR_CARGO_TARGET_DIR", "/tmp/ign-drr/target-rebuild"
)
DEFAULT_CHROME = os.environ.get(
    "DRR_CHROME",
    "~/Library/Caches/ms-playwright/chromium_headless_shell-1234/"
    "chrome-headless-shell-mac-arm64/chrome-headless-shell",
)
DEFAULT_MAGICK = os.environ.get("DRR_MAGICK", "/opt/homebrew/bin/magick")
DEFAULT_SVG_LINTER = os.environ.get("DRR_SVG_LINTER", "svg-linter")

# The seven example fixtures (public fixture names) and their CLI format
# values (public CLI flag values).
FORMATS = [
    ("dbml", "schema.dbml"),
    ("wavedrom", "timing.json5"),
    ("d2", "architecture.d2"),
    ("structurizr", "workspace.dsl"),
    ("likec4", "model.c4"),
    ("nomnoml", "classes.nomnoml"),
    ("pikchr", "flow.pikchr"),
]


class GateError(RuntimeError):
    """A hard failure that must stop the toolchain."""


def resolve_tree(explicit: str | None) -> Path:
    if explicit:
        # An explicit --tree path is trusted as-is (the vacuum re-run uses
        # flat /tmp copies whose directory names differ on purpose).
        return Path(explicit).expanduser().resolve()
    tree = Path(__file__).resolve().parent.parent
    if tree.name != "diagram-render-rs-explainer":
        raise GateError(f"tree root looks wrong: {tree}")
    return tree


def guard_engine(engine_arg: str | None) -> Path:
    """Verify the engine repo is at the frozen snapshot and unmodified.

    The only tolerated porcelain entries are untracked files inside the
    delivery tree itself.
    """
    engine = Path(engine_arg or DEFAULT_ENGINE).expanduser().resolve()
    if not (engine / "Cargo.toml").is_file():
        raise GateError(f"engine repo not found: {engine}")

    def git(*args: str) -> str:
        return subprocess.run(
            ["git", "-C", str(engine), *args],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    head = git("rev-parse", "HEAD")
    if head != FROZEN_HEAD:
        raise GateError(
            f"engine HEAD drifted: expected {FROZEN_HEAD}, got {head}"
        )
    dirty = [
        line for line in git("status", "--porcelain", "-uall").splitlines() if line
    ]
    allowed_prefix = f"?? {TREE_REL}/"
    offenders = [line for line in dirty if not line.startswith(allowed_prefix)]
    if offenders:
        raise GateError(f"engine porcelain is dirty outside the tree: {offenders}")
    return engine


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(Path(path).read_bytes())


def run_capture(cmd: list[str], cwd: Path | None = None) -> tuple[int, str]:
    """Run a command, return (exit code, combined utf-8 output)."""
    proc = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        capture_output=True,
        text=True,
        errors="replace",
    )
    output = proc.stdout + proc.stderr
    return proc.returncode, output


def normalize_transcript(text: str, work_dirs: list[str]) -> str:
    """Apply the rebuild-layer normalization documented in README.md.

    Tokens: <ts> for timestamps, <dur> for durations, <work> for scratch
    directory paths.
    """
    out = text
    for work in sorted(work_dirs, key=len, reverse=True):
        out = out.replace(work, "<work>")
    out = re.sub(r"\d+(\.\d+)?s\b(?=.* Finished|\s*$)", "<dur>", out)
    out = re.sub(
        r"\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:?\d{2})?\b",
        "<ts>",
        out,
    )
    return out


# --- structured SVG/PNG facts (pure python, no third-party deps) ----------


def svg_stats(svg_text: str) -> dict:
    """Structural element counts and canvas facts for one SVG document."""
    counts: dict[str, int] = {}
    for tag in (
        "rect",
        "ellipse",
        "line",
        "polyline",
        "polygon",
        "text",
        "g",
        "title",
        "path",
        "circle",
        "tspan",
    ):
        n = len(re.findall(rf"<{tag}[\s/>]", svg_text))
        if n:
            counts[tag] = n
    dims = re.search(
        r'<svg[^>]*\bwidth="([\d.]+)"[^>]*\bheight="([\d.]+)"', svg_text
    )
    title = re.search(r"<title[^>]*>(.*?)</title>", svg_text, re.S)
    return {
        "elements": counts,
        "width": float(dims.group(1)) if dims else None,
        "height": float(dims.group(2)) if dims else None,
        "title": title.group(1) if title else None,
    }


def png_dimensions(data: bytes) -> tuple[int, int]:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise GateError("not a PNG file")
    if data[12:16] != b"IHDR":
        raise GateError("PNG IHDR not where expected")
    w = int.from_bytes(data[16:20], "big")
    h = int.from_bytes(data[20:24], "big")
    return w, h


def byte_str(n: int) -> str:
    return f"{n:,}"
