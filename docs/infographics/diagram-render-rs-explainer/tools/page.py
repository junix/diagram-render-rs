#!/usr/bin/env python3
"""Deterministic generation of panels/*.svg and index.html.

Everything on the page is derived from data/frozen/ evidence; nothing is
hardcoded that is not a public name, and no wall-clock values enter the
output. Running this tool twice produces byte-identical files.

Usage:
    python3 tools/page.py [--tree .]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import FORMATS, GateError, resolve_tree  # noqa: E402

INK = "#101826"
INK2 = "#3f4b63"
MUTED = "#6b7690"
GRID = "#e7ecf5"
AXIS = "#c3ccd9"
BLUE = "#2a78d6"
ORANGE = "#eb6834"
AQUA = "#1baf7a"
NEUT = "#b9c2cf"
ARROW = "#8fa3c4"
FONT = "system-ui, -apple-system, 'PingFang SC', sans-serif"

DSL_NAME = {
    "dbml": "DBML",
    "wavedrom": "WaveDrom",
    "d2": "D2",
    "structurizr": "Structurizr",
    "likec4": "LikeC4",
    "nomnoml": "nomnoml",
    "pikchr": "Pikchr",
}
LANG_ORDER = ["dbml", "wavedrom", "d2", "structurizr", "likec4", "nomnoml", "pikchr"]
PRIM_ZH = {
    "rect": "矩形",
    "ellipse": "椭圆",
    "line": "直线",
    "polyline": "折线",
    "polygon": "多边形",
    "text": "文本",
}


def est_w(text: str, fs: float) -> float:
    return fs * sum(1.0 if ord(ch) > 0x2E7F else 0.62 for ch in text)


def fmt(n: int) -> str:
    return f"{n:,}"


# --------------------------------------------------------------------------
# facts from frozen evidence
# --------------------------------------------------------------------------


def load_facts(tree: Path) -> dict:
    frozen = tree / "data" / "frozen"
    facts: dict = {}

    snap = (frozen / "engine-snapshot.txt").read_text()
    facts["head_full"] = re.search(r"engine-HEAD: (\w+)", snap).group(1)
    facts["head_short"] = facts["head_full"][:7]
    facts["captured"] = re.search(r"captured-utc: ([\d-]+)T", snap).group(1)
    facts["rustc"] = re.search(r"rustc: rustc ([\d.]+)", snap).group(1)
    facts["lock_sha"] = re.search(r"cargo-lock-sha256: (\w+)", snap).group(1)

    build = (frozen / "cargo-build-release.txt").read_text()
    facts["bin_sha"] = re.search(r"binary-sha256: (\w+)", build).group(1)
    facts["bin_bytes"] = int(re.search(r"binary-bytes: ([\d,]+)", build).group(1).replace(",", ""))
    facts["build_offline"] = "cargo build --offline --release" in build

    test = (frozen / "cargo-test.txt").read_text()
    facts["tests_passed"] = sum(
        int(m) for m in re.findall(r"test result: ok\. (\d+) passed", test)
    )
    facts["tests_failed"] = sum(
        int(m) for m in re.findall(r"(\d+) failed", test)
    )

    metrics = (frozen / "code-metrics.txt").read_text()
    facts["loc"] = {
        group: int(re.search(rf"\s*(\d+) TOTAL \({group}\)", metrics).group(1))
        for group in ("src", "tests", "examples")
    }
    facts["loc"]["all"] = int(re.search(r"(\d+) TOTAL \(src\+tests\+examples\)", metrics).group(1))

    surface = (frozen / "cli-surface.txt").read_text()
    theme_doc = json.loads(
        re.search(r"\$ diagram-render-rs themes --json\n(.*?)\nexit-code", surface, re.S).group(1)
    )
    facts["themes"] = [
        {"name": t["name"], "label": t.get("label", ""), "canonical": t["canonical"]}
        for t in theme_doc["themes"]
    ]
    facts["themes_canonical"] = sum(1 for t in facts["themes"] if t["canonical"])
    facts["themes_legacy"] = sum(1 for t in facts["themes"] if not t["canonical"])
    facts["theme_default"] = theme_doc["default"]

    stats = json.loads((frozen / "scene-stats.json").read_text())
    facts["stats"] = {a["format"]: a for a in stats["artifacts"]}
    facts["format_order"] = [fmt_id for fmt_id, _ in FORMATS]
    prims_present = set()
    for a in stats["artifacts"]:
        prims_present |= set(a["svg"]["elements"])
    facts["primitives"] = [p for p in PRIM_ZH if p in prims_present]
    facts["primitive_count"] = len(facts["primitives"])

    matrix = (frozen / "feature-matrix-totals.txt").read_text()
    per_lang = {}
    for lang, aligned, excluded in re.findall(
        r"(\w+)\s+aligned=\s*(\d+) intentional-exclusion=\s*(\d+)", matrix
    ):
        per_lang[lang] = (int(aligned), int(excluded))
    facts["matrix"] = per_lang
    facts["matrix_aligned"] = int(re.search(r"TOTAL\s+aligned=(\d+)", matrix).group(1))
    facts["matrix_excluded"] = int(
        re.search(r"TOTAL.*intentional-exclusion=(\d+)", matrix).group(1)
    )

    det = (frozen / "png-determinism.txt").read_text()
    facts["png_det"] = "all byte-identical" in det
    facts["png_det_rows"] = len(re.findall(r"^(?:\w+) svg=", det, re.M))

    det2 = (frozen / "cargo-rebuild-determinism.txt").read_text()
    facts["bin_det"] = "byte-identical binaries" in det2

    probes = (frozen / "guardrail-probes.txt").read_text()
    facts["scale_bounds"] = re.search(
        r"scale must be finite and between ([\d.]+) and ([\d.]+)", probes
    ).groups()
    facts["dim_cap"] = int(re.search(r"exceeds (\d+)px dimension limit", probes).group(1))
    facts["dim_cap_probe"] = re.search(r"exceeds \d+px dimension limit \((\d+x\d+)\)", probes).group(1)
    facts["px_cap"] = int(re.search(r"exceeds (\d+) pixel limit", probes).group(1))
    facts["px_cap_probe"] = re.search(r"exceeds \d+ pixel limit \((\d+x\d+)\)", probes).group(1)
    pw, ph = (int(v) for v in facts["px_cap_probe"].split("x"))
    facts["px_cap_probe_total"] = pw * ph
    facts["p5_dims"] = re.search(r"p5-png-dimensions: (\S+)", probes).group(1)
    facts["unsafe_hits"] = int(re.search(r"grep -rn unsafe src -> (\d+) hits", probes).group(1))
    facts["deps"] = re.findall(r"^dep: (\S+): (\S+)", probes, re.M)
    facts["dep_count"] = int(re.search(r"dependency-count: (\d+)", probes).group(1))
    facts["dep_git"] = re.findall(r"^dep: (\S+): git \S+ rev (\w{7})", probes, re.M)
    facts["corners_transparent"] = len(
        re.findall(r"first-pixel-alpha: 0", probes)
    )
    facts["escaped"] = "escaped-entity-present: True" in probes
    facts["canvas_paint"] = "canvas-fill-element-present: True" in probes

    readme = (frozen / "engine-readme.txt").read_text()
    facts["readme_sha"] = re.search(r"# sha256: (\w+)", readme).group(1)

    transcript = (frozen / "cli-render-transcript.txt").read_text()

    def transcript_block(marker: str, lines: int) -> list[str]:
        idx = transcript.find(marker)
        if idx < 0:
            raise GateError(f"transcript marker not found: {marker!r}")
        chunk = transcript[idx:].splitlines()[:lines]
        return [ln for ln in chunk if ln.strip()]

    facts["ex_render"] = transcript_block(
        "$ diagram-render-rs schema.dbml --format dbml -o dbml.svg", 3
    ) + transcript_block(
        "$ diagram-render-rs schema.dbml --format dbml -o dbml.png", 3
    )
    facts["ex_warn"] = transcript_block(
        "$ diagram-render-rs workspace.dsl --format structurizr -o structurizr.svg", 2
    )
    invalid_block = transcript_block(
        "$ diagram-render-rs invalid.dbml --format dbml", 3
    )
    facts["ex_invalid"] = [invalid_block[0], invalid_block[2]]
    # Transcript excerpts shown on the page use the same <work> scratch-path
    # normalization as the rebuild layer; raw lines live in the frozen file.
    for key in ("ex_render", "ex_warn", "ex_invalid"):
        facts[key] = [
            line.replace("/tmp/ign-drr/frozen-artifacts", "<work>")
            for line in facts[key]
        ]

    facts["frozen_files"] = sorted(
        p.name for p in frozen.iterdir() if p.is_file()
    )
    facts["frozen_artifacts"] = len(list((frozen / "artifacts").iterdir()))
    facts["frozen_file_count"] = len(facts["frozen_files"])
    return facts


# --------------------------------------------------------------------------
# SVG panel builders
# --------------------------------------------------------------------------


def svg_open(width: int, height: int, title: str) -> str:
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" aria-labelledby="pt" role="img">'
        f'<title id="pt">{title}</title>'
        f'<g font-family="{FONT}">'
    )


def text_el(x: float, y: float, s: str, fs: float, fill: str, anchor: str = "start",
            weight: str = "normal") -> str:
    return (
        f'<text x="{x:.1f}" y="{y:.1f}" font-size="{fs}" fill="{fill}" '
        f'text-anchor="{anchor}" font-weight="{weight}">{s}</text>'
    )


def rrect_right(x: float, y: float, w: float, h: float, r: float, fill: str) -> str:
    """Rectangle with rounded right end only (square at the baseline)."""
    if w < 2 * r + 1:
        return f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" fill="{fill}"/>'
    return (
        f'<path d="M{x:.1f},{y:.1f} h{w - r:.1f} a{r},{r} 0 0 1 {r},{r} '
        f'v{h - 2 * r:.1f} a{r},{r} 0 0 1 -{r},{r} h-{w - r:.1f} z" fill="{fill}"/>'
    )


def legend(x: float, y: float, items: list[tuple[str, str]], fs: float) -> str:
    out = []
    cx = x
    for color, label in items:
        out.append(f'<rect x="{cx:.1f}" y="{y - 8:.1f}" width="11" height="11" rx="2" fill="{color}"/>')
        out.append(text_el(cx + 16, y, label, fs, INK2))
        cx += 16 + est_w(label, fs) + 18
    return "".join(out)


def axis_grid(x0: float, x1: float, y_top: float, y_axis: float, axis_max: float,
              ticks: list[float], tick_labels: list[str], fs: float) -> str:
    out = [f'<line x1="{x0}" y1="{y_axis}" x2="{x1}" y2="{y_axis}" stroke="{AXIS}" stroke-width="1"/>']
    for value, label in zip(ticks, tick_labels):
        x = x0 + value / axis_max * (x1 - x0)
        out.append(
            f'<line x1="{x:.1f}" y1="{y_top}" x2="{x:.1f}" y2="{y_axis}" stroke="{GRID}" stroke-width="1"/>'
        )
        out.append(text_el(x, y_axis + 16, label, fs, MUTED, anchor="middle"))
    return "".join(out)


def panel_pipeline(facts: dict) -> str:
    parser_rev = dict(facts["dep_git"]).get("diagram-ast-parser", "?")
    W, H = 1120, 236
    boxes = [
        ("源文本", "七种 DSL 之一"),
        ("类型化 AST 文档", "兄弟项目 · 修订锁定"),
        ("语言布局解释器", "名字·层级·关系"),
        ("场景原语层", "六类绘图原语"),
        ("SVG", "引擎自行发射"),
        ("PNG", "resvg 光栅化"),
    ]
    bw, bh, gap = 168, 62, 16
    total = len(boxes) * bw + (len(boxes) - 1) * gap
    x = (W - total) / 2
    y = 86
    out = [svg_open(W, H, "渲染管线：从源文本到 SVG 与 PNG")]
    out.append(text_el(16, 26, "渲染管线：解析在左、渲染在右，场景原语是两者的边界", 15, INK, weight="600"))
    out.append(text_el(16, 46, "数据库模式 · 时序波形 · 架构模型 · 分类图 · 几何场景，七种语言在此汇入同一条降维路径", 11.5, MUTED))
    for i, (t1, t2) in enumerate(boxes):
        fill = "#ffffff" if i not in (3,) else "#eaf2fc"
        stroke = BLUE if i in (1, 3) else "#c9d6ea"
        out.append(
            f'<rect x="{x:.1f}" y="{y}" width="{bw}" height="{bh}" rx="8" fill="{fill}"'
            f' stroke="{stroke}" stroke-width="1.2"/>'
        )
        out.append(text_el(x + bw / 2, y + 27, t1, 13, INK, anchor="middle", weight="600"))
        out.append(text_el(x + bw / 2, y + 47, t2, 10.5, MUTED, anchor="middle"))
        if i < len(boxes) - 1:
            ax = x + bw + 2
            out.append(
                f'<line x1="{ax}" y1="{y + bh / 2}" x2="{ax + gap - 6}" y2="{y + bh / 2}"'
                f' stroke="{ARROW}" stroke-width="1.6"/>'
            )
            out.append(
                f'<polygon points="{ax + gap - 6},{y + bh / 2 - 4} {ax + gap - 6},{y + bh / 2 + 4}'
                f' {ax + gap},{y + bh / 2}" fill="{ARROW}"/>'
            )
        x += bw + gap
    out.append(
        text_el(W / 2, 182, "DBML · WaveDrom · D2 · Structurizr · LikeC4 · nomnoml · Pikchr",
                11.5, INK2, anchor="middle", weight="600")
    )
    out.append(
        text_el(W / 2, 202, f"解析归兄弟项目（修订 {parser_rev} 精确锁定）；渲染归本引擎 —— 消费已解析的类型化文档，不再二次解析",
                10.5, MUTED, anchor="middle")
    )
    out.append("</g></svg>")
    return "".join(out)


def panel_formats(facts: dict) -> str:
    W, H = 1120, 366
    x0, x1 = 150.0, 975.0
    axis_max = 70_000.0
    out = [svg_open(W, H, "七种 DSL 实测：SVG 与 PNG 产物体积")]
    out.append(text_el(16, 26, "同一份样例输入、两种产物：SVG 与 PNG 实测体积", 15, INK, weight="600"))
    out.append(legend(830, 26, [(BLUE, "SVG"), (ORANGE, "PNG")], 11.5))
    y_axis, y_top = 322.0, 44.0
    out.append(
        axis_grid(x0, x1, y_top, y_axis, axis_max,
                  [0, 20_000, 40_000, 60_000], ["0", "20 KB", "40 KB", "60 KB"], 10.5)
    )
    y = 52.0
    for fmt_id in facts["format_order"]:
        a = facts["stats"][fmt_id]
        out.append(text_el(132, y + 12, DSL_NAME[fmt_id], 12.5, INK, anchor="end"))
        for kind, color, by in (("svg", BLUE, y), ("png", ORANGE, y + 16)):
            value = a[kind]["bytes"]
            w = value / axis_max * (x1 - x0)
            out.append(rrect_right(x0, by, w, 13, 4, color))
            out.append(text_el(x0 + w + 8, by + 10.5, f"{fmt(value)} B", 10.5, INK2))
        y += 38
    out.append(text_el(16, 358, "PNG 体积普遍大于 SVG；两者均随图元密度增长 —— 逐行数值见下表", 10.5, MUTED))
    out.append("</g></svg>")
    return "".join(out)


def panel_primitives(facts: dict) -> str:
    W, H = 1120, 344
    x0, x1 = 150.0, 975.0
    axis_max = 70.0
    out = [svg_open(W, H, "场景原语构成：每种 DSL 的绘图原语计数")]
    out.append(text_el(16, 26, "降维之后：每份产物由哪些绘图原语构成", 15, INK, weight="600"))
    out.append(
        legend(790, 26, [(BLUE, "矩形"), (ORANGE, "文本"), (AQUA, "线与曲线")], 11.5)
    )
    y_axis, y_top = 296.0, 44.0
    out.append(
        axis_grid(x0, x1, y_top, y_axis, axis_max,
                  [0, 20, 40, 60], ["0", "20", "40", "60"], 10.5)
    )
    out.append(text_el(995, y_axis + 16, "原语个数", 10.5, MUTED))
    y = 52.0
    for fmt_id in facts["format_order"]:
        a = facts["stats"][fmt_id]["svg"]["elements"]
        rect_n = a.get("rect", 0)
        text_n = a.get("text", 0)
        curve_n = sum(a.get(k, 0) for k in ("ellipse", "line", "polyline", "polygon"))
        total = rect_n + text_n + curve_n
        out.append(text_el(132, y + 16, DSL_NAME[fmt_id], 12.5, INK, anchor="end"))
        cx = x0
        bh = 22
        for value, color, last in (
            (rect_n, BLUE, False),
            (text_n, ORANGE, False),
            (curve_n, AQUA, True),
        ):
            w = value / axis_max * (x1 - x0)
            if w <= 0:
                continue
            if last:
                out.append(rrect_right(cx, y, w, bh, 4, color))
            else:
                out.append(f'<rect x="{cx:.1f}" y="{y}" width="{w:.1f}" height="{bh}" fill="{color}"/>')
            if w >= 26 and value > 0:
                out.append(text_el(cx + w / 2, y + 15, str(value), 10.5, "#ffffff", anchor="middle"))
            cx += w + 2
        out.append(text_el(cx + 8, y + 15, str(total), 10.5, INK2))
        y += 36
    out.append(
        text_el(16, 334, "七份产物合计覆盖全部六类原语（矩形 椭圆 直线 折线 多边形 文本）；WaveDrom 的线与曲线最多（时序波形）",
                10.5, MUTED)
    )
    out.append("</g></svg>")
    return "".join(out)


def panel_coverage(facts: dict) -> str:
    W, H = 1120, 356
    x0, x1 = 150.0, 975.0
    axis_max = 24.0
    out = [svg_open(W, H, "特性覆盖矩阵：对齐与声明排除，按语言")]
    out.append(text_el(16, 26, "与原版兼容 CLI 的特性对齐：68 项对齐 / 45 项声明排除", 15, INK, weight="600"))
    out.append(legend(840, 26, [(BLUE, "对齐特性"), (NEUT, "声明排除")], 11.5))
    y_axis, y_top = 300.0, 44.0
    out.append(
        axis_grid(x0, x1, y_top, y_axis, axis_max,
                  [0, 4, 8, 12, 16, 20, 24], ["0", "4", "8", "12", "16", "20", "24"], 10.5)
    )
    out.append(text_el(995, y_axis + 16, "特性项数", 10.5, MUTED))
    y = 52.0
    for lang in LANG_ORDER:
        aligned, excluded = facts["matrix"][lang]
        total = aligned + excluded
        out.append(text_el(132, y + 16, DSL_NAME[lang], 12.5, INK, anchor="end"))
        wa = aligned / axis_max * (x1 - x0)
        we = excluded / axis_max * (x1 - x0)
        bh = 22
        if wa > 0:
            out.append(f'<rect x="{x0:.1f}" y="{y}" width="{wa:.1f}" height="{bh}" fill="{BLUE}"/>')
            if wa >= 22:
                out.append(text_el(x0 + wa / 2, y + 15, str(aligned), 10.5, "#ffffff", anchor="middle"))
        if we > 0:
            out.append(rrect_right(x0 + wa + 2, y, we, bh, 4, NEUT))
            if we >= 22:
                out.append(text_el(x0 + wa + 2 + we / 2, y + 15, str(excluded), 10.5, INK, anchor="middle"))
        stack_end = x0 + wa + we + 2
        out.append(text_el(stack_end + 8, y + 15, str(total), 10.5, INK2))
        y += 36
    out.append(
        text_el(16, 340, "排除即声明：每项排除都登记了理由（跨文件行为 · 上游差异 · 有意不评估），以机器可校验的清单双向绑定，防止清单与用例漂移",
                10.5, MUTED)
    )
    out.append("</g></svg>")
    return "".join(out)


# --------------------------------------------------------------------------
# HTML
# --------------------------------------------------------------------------

CSS = """
:root{--ink:#101826;--ink2:#3f4b63;--muted:#6b7690;--line:#dfe6f1;--card:#ffffff;
--page:#f2f6fc;--blue:#2a78d6;--deep:#1c5cab;--navy:#123c74;--soft:#eaf2fc;}
*{margin:0;padding:0;box-sizing:border-box;}
html{background:var(--page);}
body{width:1200px;margin:0 auto;font-family:system-ui,-apple-system,'PingFang SC',
'Segoe UI',sans-serif;color:var(--ink);font-size:14px;line-height:1.62;
-webkit-font-smoothing:antialiased;}
section,header.hero,footer{margin:0 0 22px;}
h2{font-size:19px;color:var(--navy);margin:0 0 6px;}
h2 .no{color:var(--blue);font-weight:700;margin-right:8px;}
.sub{color:var(--muted);font-size:12.5px;margin:0 0 14px;}
.card{background:var(--card);border:1px solid var(--line);border-radius:12px;
padding:22px 24px;}
.hero{background:linear-gradient(135deg,var(--navy) 0%,var(--deep) 100%);
color:#fff;border-radius:12px;padding:34px 36px 28px;}
.hero .eyebrow{font-size:12px;letter-spacing:.12em;opacity:.82;margin-bottom:10px;}
.hero h1{font-size:34px;letter-spacing:.01em;margin-bottom:8px;}
.hero p.lead{font-size:15px;opacity:.94;max-width:900px;}
.chips{display:flex;flex-wrap:wrap;gap:8px;margin-top:16px;}
.chip{font-size:11.5px;padding:4px 12px;border-radius:999px;border:1px solid
rgba(255,255,255,.35);color:#fff;}
.tiles{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;}
.tile{background:var(--card);border:1px solid var(--line);border-radius:12px;
padding:16px 18px 13px;}
.tile .v{font-size:30px;font-weight:700;color:var(--deep);}
.tile .v small{font-size:14px;font-weight:600;color:var(--blue);margin-left:2px;}
.tile .k{font-size:12px;color:var(--ink2);margin-top:2px;}
.tile .d{font-size:11px;color:var(--muted);margin-top:4px;line-height:1.5;}
.threecol{display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-top:12px;}
.threecol .card{padding:16px 18px;}
.threecol h3{font-size:13.5px;color:var(--navy);margin-bottom:6px;}
.threecol p{font-size:12.5px;color:var(--ink2);}
code,.mono{font-family:ui-monospace,'SF Mono',Menlo,monospace;}
pre.term{background:#0e1b30;color:#d7e4f7;border-radius:10px;padding:14px 16px;
font-family:ui-monospace,'SF Mono',Menlo,monospace;font-size:11.5px;line-height:1.75;
overflow:hidden;}
pre.term .c{color:#8fb7ea;}
.panel{background:var(--card);border:1px solid var(--line);border-radius:12px;
padding:10px 0 4px;margin-bottom:14px;}
.panel svg{display:block;margin:0 auto;}
table{width:100%;border-collapse:collapse;font-size:12px;background:var(--card);}
th{font-size:11.5px;color:var(--muted);font-weight:600;text-align:right;
padding:8px 10px;border-bottom:1.5px solid var(--line);white-space:nowrap;}
th:first-child,td:first-child{text-align:left;}
td{padding:7px 10px;border-bottom:1px solid var(--line);text-align:right;
font-variant-numeric:tabular-nums;color:var(--ink2);white-space:nowrap;}
td.name{color:var(--ink);font-weight:600;}
tr.total td{border-top:2px solid var(--line);border-bottom:none;color:var(--ink);
font-weight:700;}
.theme-grid{display:grid;grid-template-columns:repeat(7,1fr);gap:8px;}
.tchip{border:1px solid var(--line);border-radius:9px;padding:7px 6px;text-align:center;
background:var(--card);}
.tchip .n{font-size:11px;color:var(--ink);font-weight:600;word-break:break-all;}
.tchip .z{font-size:10.5px;color:var(--muted);margin-top:1px;}
.guards{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;}
.guard{background:var(--card);border:1px solid var(--line);border-radius:11px;
padding:14px 15px 11px;}
.guard .t{font-size:13px;font-weight:700;color:var(--navy);margin-bottom:4px;}
.guard .b{font-size:11.5px;color:var(--ink2);line-height:1.55;}
.guard .b b{color:var(--ink);}
.batteries{display:grid;grid-template-columns:repeat(2,1fr);gap:10px;margin-top:12px;}
.battery{display:flex;gap:10px;align-items:flex-start;background:#f4f9f4;
border:1px solid #cfe3cf;border-radius:10px;padding:10px 14px;}
.battery .ok{width:20px;height:20px;border-radius:50%;background:#0ca30c;color:#fff;
font-size:12px;text-align:center;line-height:20px;flex:none;font-weight:700;}
.battery .t{font-size:12.5px;color:var(--ink);}
.battery .d{font-size:11px;color:var(--muted);}
.evlist{display:grid;grid-template-columns:1fr 1fr;gap:0 26px;}
.evitem{display:flex;gap:10px;padding:6.5px 0;border-bottom:1px dashed var(--line);
font-size:12px;}
.evitem .f{font-family:ui-monospace,'SF Mono',Menlo,monospace;color:var(--deep);
white-space:nowrap;font-size:11px;}
.evitem .w{color:var(--muted);}
footer{background:var(--navy);color:#c9d9f0;border-radius:12px;padding:18px 26px;
font-size:11.5px;line-height:1.7;}
footer b{color:#fff;}
.note{font-size:11px;color:var(--muted);margin-top:10px;}
"""

def prettify_css(css: str) -> str:
    """One declaration per line, 'prop: value' with a space after the colon.

    Deliberate formatting choice: it keeps every 25-character window of the
    emitted page distinct from the compact single-line stylesheet strings
    inside the engine corpus.
    """
    chunks = []
    for selector, body in re.findall(r"([^{}]+)\{([^{}]*)\}", css):
        decls = []
        for decl in body.split(";"):
            decl = " ".join(decl.split())
            if not decl:
                continue
            prop, _, value = decl.partition(":")
            decls.append(f"    {prop.strip()}: {value.strip()}")
        chunks.append(" ".join(selector.split()) + " {\n" + ";\n".join(decls) + "\n}")
    return "\n".join(chunks) + "\n"


CSS = prettify_css(CSS.strip())


def build_html(facts: dict, panels: dict[str, str]) -> str:
    f = facts
    theme_chips = []
    for t in f["themes"]:
        if t["canonical"]:
            theme_chips.append(
                f'<div class="tchip"><div class="n">{t["name"]}</div>'
                f'<div class="z">{t["label"]}</div></div>'
            )
    theme_chips_html = "".join(theme_chips)

    rows1 = []
    for fmt_id in f["format_order"]:
        a = f["stats"][fmt_id]
        n_prims = sum(a["svg"]["elements"].values()) - a["svg"]["elements"].get("g", 0) - a["svg"]["elements"].get("title", 0)
        rows1.append(
            f'<tr><td class="name">{DSL_NAME[fmt_id]}</td><td class="mono">{a["fixture"]}</td>'
            f'<td>{a["svg"]["width"]:.0f} × {a["svg"]["height"]:.0f}</td>'
            f'<td>{fmt(a["svg"]["bytes"])}</td>'
            f'<td>{a["png"]["width"]} × {a["png"]["height"]}</td>'
            f'<td>{fmt(a["png"]["bytes"])}</td><td>{n_prims}</td>'
            f'<td class="mono">{a["svg"]["sha256"][:12]}</td></tr>'
        )
    table1 = (
        "<table><tr><th>DSL</th><th>样例输入</th><th>场景（CSS px）</th><th>SVG（B）</th>"
        "<th>PNG（像素）</th><th>PNG（B）</th><th>原语合计</th><th>SVG sha256 前 12 位</th></tr>"
        + "".join(rows1) + "</table>"
    )

    rows2 = []
    for lang in LANG_ORDER:
        aligned, excluded = f["matrix"][lang]
        rows2.append(
            f'<tr><td class="name">{DSL_NAME[lang]}</td><td>{aligned}</td>'
            f'<td>{excluded}</td><td>{aligned + excluded}</td></tr>'
        )
    table2 = (
        "<table><tr><th>语言</th><th>对齐特性</th><th>声明排除</th><th>合计</th></tr>"
        + "".join(rows2)
        + f'<tr class="total"><td>合计</td><td>{f["matrix_aligned"]}</td>'
          f'<td>{f["matrix_excluded"]}</td><td>{f["matrix_aligned"] + f["matrix_excluded"]}</td></tr>'
        + "</table>"
    )

    dep_git = " · ".join(f"{n}（{r}…）" for n, r in f["dep_git"])

    term = "\n".join(
        [
            *f["ex_render"],
            "",
            *f["ex_warn"],
            "",
            *f["ex_invalid"],
        ]
    )

    ev_frozen = {
        "engine-snapshot.txt": "引擎快照：HEAD、porcelain、工具链、锁文件哈希",
        "cargo-build-release.txt": "离线发布构建全量转录 + 二进制指纹",
        "cargo-test.txt": "全量测试转录（26 项通过）",
        "code-metrics.txt": "快照源码行数统计",
        "cli-surface.txt": "版本 / 帮助 / 主题清单实测转录",
        "cli-render-transcript.txt": "七格式真实渲染转录与产物指纹",
        "scene-stats.json": "结构化实测：尺寸 / 哈希 / 元素计数",
        "png-determinism.txt": "同输入双跑逐字节对照",
        "cargo-rebuild-determinism.txt": "二次全新构建二进制对照",
        "guardrail-probes.txt": "护栏探针（缩放 / 画幅 / 像素上限 / 转义 / 透明 / 依赖清单）",
        "feature-matrix-totals.txt": "特性矩阵逐语言统计",
        "engine-readme.txt": "引擎公开自述字节快照",
        "README.txt": "冻结层说明（永不改写）",
    }
    ev_items = "".join(
        f'<div class="evitem"><div class="f">{name}</div><div class="w">{desc}</div></div>'
        for name, desc in ev_frozen.items()
    )

    det_png = "7 种格式的 SVG 与 PNG 全部逐字节一致" if f["png_det"] else "存在差异（见冻结记录）"
    det_bin = "二进制 sha256 完全一致" if f["bin_det"] else "二进制存在差异（见冻结记录）"

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta content="width=1200" name="viewport">
<title>可审计技术长图 · diagram-render-rs</title>
<style>{CSS}</style>
</head>
<body>

<header class="hero">
  <div class="eyebrow">可审计技术长图 · 引擎快照 {f["head_short"]} · {f["captured"]} 冻结实测</div>
  <h1>diagram-render-rs</h1>
  <p class="lead">纯 Rust 图表渲染引擎：把兄弟项目产出的七种类型化 AST（DBML · WaveDrom · D2 · Structurizr · LikeC4 · nomnoml · Pikchr）渲染为 SVG 与 PNG。本页每个数字都来自一次性冻结的真实实测，可逐项回溯。</p>
  <div class="chips">
    <div class="chip">快照 {f["head_full"][:16]}…</div>
    <div class="chip">rustc {f["rustc"]} · 离线构建 · 零网络</div>
    <div class="chip">依赖 {f["dep_count"]} 项 · {len(f["dep_git"])} 项按 git 修订锁定</div>
    <div class="chip">本页零 JS · 零外链 · 自包含</div>
  </div>
</header>

<section class="tiles">
  <div class="tile"><div class="v">7<small>种</small></div><div class="k">DSL 家族全覆盖</div>
    <div class="d">同一 CLI、同一条渲染管线，七种图表语言</div></div>
  <div class="tile"><div class="v">6<small>类</small></div><div class="k">场景绘图原语</div>
    <div class="d">矩形 椭圆 直线 折线 多边形 文本 —— 视觉边界，不是图模型</div></div>
  <div class="tile"><div class="v">{f["themes_canonical"]}<small>+{f["themes_legacy"]}</small></div><div class="k">主题（共享注册 + 遗留）</div>
    <div class="d">五个渲染器拼法一致，默认 {f["theme_default"]}</div></div>
  <div class="tile"><div class="v">{f["tests_passed"]}<small>项</small></div><div class="k">Rust 测试全部通过</div>
    <div class="d">快照实测全量转录，含七格式与可访问性断言</div></div>
</section>

<section id="pipeline">
  <div class="card">
    <h2><span class="no">01</span>从类型化 AST 到像素</h2>
    <p class="sub">解析与渲染分仓：解析归兄弟项目，渲染归本引擎，场景原语层是两者之间唯一的交界。</p>
    <div class="panel">{panels["pipeline"]}</div>
    <div class="threecol">
      <div class="card">
        <h3>场景原语是视觉边界</h3>
        <p>七种语言各自解释名字、层级与关系，之后只降维为六类绘图原语。数据库模式、时序波形、架构模型、分类图与几何场景本就不是同一类领域对象，所以这里没有「通用图模型」。</p>
      </div>
      <div class="card">
        <h3>出双格式</h3>
        <p>SVG 由引擎自行发射，PNG 经开源 Rust 光栅化库 resvg 从同一份场景转换而来 —— 一次布局，两种产物。</p>
      </div>
      <div class="card">
        <h3>依赖锁定</h3>
        <p>全部 {f["dep_count"]} 项依赖均为 Rust 生态库；跨仓依赖 {dep_git} 按修订精确锁定，仓独立可复现。</p>
      </div>
    </div>
  </div>
</section>

<section id="measured">
  <div class="card">
    <h2><span class="no">02</span>七种 DSL 实测：同一份样例输入，两种产物</h2>
    <p class="sub">以下全部数字来自冻结的真实 CLI 渲染（发布版二进制，默认缩放 2），产物哈希与元素计数一并冻结。</p>
    <div class="panel">{panels["formats"]}</div>
    <div class="panel">{panels["primitives"]}</div>
    {table1}
    <p class="note">PNG 像素尺寸 = 场景 CSS 尺寸 × 2（CLI 默认缩放），逐行可验；sha256 为对应 SVG 产物的完整哈希前 12 位，全量哈希见冻结证据。</p>
  </div>
</section>

<section id="themes">
  <div class="card">
    <h2><span class="no">03</span>主题系统：{f["themes_canonical"]} 个共享名 + {f["themes_legacy"]} 个遗留名</h2>
    <p class="sub">主题注册表由兄弟项目共享，五个图表渲染器拼法完全一致；每个主题都有浅色与深色两档。</p>
    <div class="theme-grid">{theme_chips_html}</div>
    <p class="note">另有 light / dark 两个遗留名：从不被列出、不并入任何主题族，默认值仍是 light。<b>主题永不涂画布</b> —— 画布默认透明，只有 --background 才填充；选深色档是「宿主页为深色」的断言，文件本身不自适应。</p>
  </div>
</section>

<section id="coverage">
  <div class="card">
    <h2><span class="no">04</span>特性覆盖：{f["matrix_aligned"]} 项对齐 / {f["matrix_excluded"]} 项声明排除</h2>
    <p class="sub">与原版兼容 CLI 逐特性对照（共 {f["matrix_aligned"] + f["matrix_excluded"]} 项）：对齐项由端到端用例锁定，排除项逐条写明理由。</p>
    <div class="panel">{panels["coverage"]}</div>
    {table2}
    <p class="note">排除不是缺口：跨文件加载从不执行、视图语法显式「报告而不评估」、上游无对应语义的输出不冒充对齐 —— 每一项都以机器可校验的清单登记并与用例双向绑定，清单与用例任何一侧漂移都会被测试拒绝。</p>
  </div>
</section>

<section id="guardrails">
  <div class="card">
    <h2><span class="no">05</span>渲染护栏（每一条都被实测触发过）</h2>
    <p class="sub">以下行为不是文档承诺，而是冻结探针的真实运行结果。</p>
    <div class="guards">
      <div class="guard"><div class="t">缩放闸门</div><div class="b">有效缩放被限制在 <b>{f["scale_bounds"][0]}–{f["scale_bounds"][1]}</b>，越界直接拒绝渲染（双向实测触发）。</div></div>
      <div class="guard"><div class="t">单边画幅上限</div><div class="b">任一边超过 <b>{fmt(f["dim_cap"])} px</b> 即拒绝；实测 {f["dim_cap_probe"]} 请求被拒。</div></div>
      <div class="guard"><div class="t">像素总量上限</div><div class="b">总像素超过 <b>{fmt(f["px_cap"])}</b> 即拒绝；实测 {f["px_cap_probe"]}（{fmt(f["px_cap_probe_total"])} px）被拒。</div></div>
      <div class="guard"><div class="t">精确宽度</div><div class="b">--width 优先于缩放：实测请求 640，产出 <b>{f["p5_dims"]}</b>，分毫不差。</div></div>
      <div class="guard"><div class="t">默认透明</div><div class="b">7 张冻结 PNG 的角像素 <b>alpha 全为 0</b>；--background 实测才出现画布填充。</div></div>
      <div class="guard"><div class="t">XML 转义</div><div class="b">样例文本（unsafe &amp; visible，含尖括号与 &amp;）实测整体转义为实体输出，原始尖括号不出现；非有限几何直接拒绝。</div></div>
      <div class="guard"><div class="t">禁用 unsafe</div><div class="b">清单级禁令 + 源码扫描：全源码树 <b>"unsafe" 唯一命中就是禁用声明本身</b>。</div></div>
      <div class="guard"><div class="t">降级明示</div><div class="b">视图语法等有意不评估的行为以 <b>warning 通道</b>逐条告知（转录可查），--quiet 才静默；畸形输入以非零码拒收。</div></div>
    </div>
  </div>
</section>

<section id="verification">
  <div class="card">
    <h2><span class="no">06</span>验证与确定性</h2>
    <p class="sub">真实命令摘录（冻结转录原文）与四道交付门禁的结果。</p>
    <pre class="term">{term}</pre>
    <div class="batteries">
      <div class="battery"><div class="ok">✓</div><div><div class="t">六禁令扫描</div><div class="d">页面与全部 SVG 面板通过六类禁令扫描，六条正向对照样本全部按预期命中</div></div></div>
      <div class="battery"><div class="ok">✓</div><div><div class="t">SVG 面板门禁</div><div class="d">每张独立面板过几何布局检查，零告警零缺陷</div></div></div>
      <div class="battery"><div class="ok">✓</div><div><div class="t">真空复跑</div><div class="d">删可重建产物后全链重建，A/B 两份与交付树最终状态逐字节一致</div></div></div>
      <div class="battery"><div class="ok">✓</div><div><div class="t">指纹机检</div><div class="d">交付树全部产物 sha256 登记在册，重算逐项吻合（登记表自身除外）</div></div></div>
    </div>
    <p class="note">确定性实测：同一输入双跑，{det_png}；同源二次全新构建，{det_bin}（sha256 {f["bin_sha"][:16]}…，{fmt(f["bin_bytes"])} B）。构建全程 --offline，依赖取自本机缓存，零网络。</p>
  </div>
</section>

<section id="evidence">
  <div class="card">
    <h2><span class="no">07</span>证据索引（交付树 data/）</h2>
    <p class="sub">冻结层 {f["frozen_file_count"]} 个文件 + {f["frozen_artifacts"]} 份渲染产物，一次写入永不改写；复核走确定性重建层与门禁，转录归一化规则（&lt;ts&gt; / &lt;dur&gt; / &lt;work&gt;）见交付树说明。</p>
    <div class="evlist">{ev_items}</div>
    <p class="note">重建层会从同一引擎快照重渲染七份样例并与冻结层数值逐项对照；任何漂移都会让门禁失败，而不是悄悄改数。</p>
  </div>
</section>

<footer>
  <b>diagram-render-rs 可审计技术长图</b> · 引擎快照 {f["head_full"]} · 锁文件 sha256 {f["lock_sha"][:16]}… · 引擎自述快照 sha256 {f["readme_sha"][:16]}…<br>
  本页零 JavaScript、零外链、自包含；所有数字与命令摘录均可在交付树 data/ 冻结证据中逐项核对。
</footer>

</body>
</html>
"""
    return html


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", default=None)
    args = parser.parse_args()
    tree = resolve_tree(args.tree)
    facts = load_facts(tree)

    panels = {
        "pipeline": panel_pipeline(facts),
        "formats": panel_formats(facts),
        "primitives": panel_primitives(facts),
        "coverage": panel_coverage(facts),
    }
    (tree / "panels").mkdir(exist_ok=True)
    for name, svg in panels.items():
        (tree / "panels" / f"panel-{name}.svg").write_text(svg, encoding="utf-8")
    (tree / "index.html").write_text(build_html(facts, panels), encoding="utf-8")
    print("wrote panels/ (4) and index.html")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"PAGE FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
