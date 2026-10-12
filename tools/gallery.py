#!/usr/bin/env python3
"""Build gallery.html for a single repo: every example as a card.

Data model (gallery.json at the repo root):

  {
    "title": "...", "tagline": "...", "rebuild": "just rust-gallery",
    "backdrop": "checker" | "light" | "dark",
    "groups": [                      // glob-collected cards
      {
        "label": "Sequence",         // optional filter category
        "sources": ["tests/fixtures/*.mmd"],
        "titles": {"basic": "HTTP request through the stack"},
        "artifacts": [
          {"glob": "examples/{stem}.svg",        "label": "Light"},
          {"glob": "examples/{stem}.dark.svg",   "label": "Dark",  "backdrop": "dark"},
          {"glob": "examples/{stem}.png",        "label": "2x PNG proof", "detail": true}
        ]
      }
    ],
    "items": [                        // explicit cards for irregular layouts
      {"id": "composition", "title": "...", "blurb": "...", "group": "SVG",
       "tags": ["compose"],
       "sources": ["examples/composition.json"],   // disclosed behind <details>
       "previews": [                                // images, or HTML with "embed"
         {"src": "examples/board.svg", "label": "Output"}
       ],
       "previewGlobs": [                            // expanded previews
         {"glob": "examples/themes/fragments.*.svg"}
       ],
       "code": [                                    // embedded text previews (parse output…)
         {"label": "AST", "src": "out/schema.ast.json"}
       ],
       "links": [{"href": "...", "label": "..."}]}
    ]
  }

Subcommands:
  (default)   read gallery.json, write gallery.html
  --check     byte-compare instead of writing; exit 1 on drift
  wrap        turn a colored HTML <pre> fragment on stdin into a standalone
              page: tools/gallery.py wrap --stem NAME [--variant V] [--text] -o OUT

Rules: stdlib only; relative links; relative images only (never inlined);
files missing on disk are skipped with a warning; previews absent from git
ls-files are reported so a fresh clone cannot silently show blank cards.
"""
from __future__ import annotations

import argparse
import html
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "gallery.json"
MAX_SOURCE_LINES = 200
MAX_CODE_LINES = 80
IMAGE_EXT = {".svg", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif"}


def fail(message: str) -> None:
    print("gallery: " + message, file=sys.stderr)
    raise SystemExit(1)


def natural_key(text: str):
    return [int(p) if p.isdigit() else p.lower() for p in re.split(r"(\d+)", text)]


def humanize(stem: str) -> str:
    words = re.sub(r"[-_]+", " ", stem).strip()
    return (words[:1].upper() + words[1:]) if words else stem


def tracked_files() -> set[str]:
    try:
        out = subprocess.run(["git", "-C", str(ROOT), "ls-files"],
                             capture_output=True, text=True, timeout=20)
    except (OSError, subprocess.SubprocessError):
        return set()
    return set(out.stdout.split()) if out.returncode == 0 else set()


def readme_blurbs() -> dict[str, str]:
    """Last descriptive cell of the README table row that links a path."""
    path = ROOT / "README.md"
    if not path.exists():
        return {}
    blurbs: dict[str, str] = {}
    link = re.compile(r"\[[^\]]*\]\(([^)#]+)")
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.lstrip().startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        hit = None
        for cell in cells:
            m = link.search(cell)
            if m:
                hit = m.group(1).rstrip("/")
                break
        if not hit:
            continue
        tail = [c for c in cells if c and not link.search(c) and not c.isdigit()]
        if tail:
            text = re.sub(r"\*\*([^*]*)\*\*", r"\1", tail[-1])
            text = re.sub(r"`([^`]*)`", r"\1", text)
            blurbs[hit] = text
    return blurbs


def content_words(text: str, limit: int = 500) -> str:
    words = re.sub(r"<!--.*?-->", " ", text, flags=re.S)
    words = re.sub(r"[^\w\-]+", " ", words, flags=re.UNICODE)
    return " ".join(words.split())[:limit]


def relpos(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


class Preview:
    __slots__ = ("path", "label", "backdrop", "detail", "embed", "height")

    def __init__(self, path, label="", backdrop=None, detail=False, embed=False, height=380):
        self.path = path
        self.label = label
        self.backdrop = backdrop
        self.detail = detail
        self.embed = embed
        self.height = height


class CodePreview:
    __slots__ = ("label", "text")

    def __init__(self, label, text):
        self.label = label
        self.text = text


class Card:
    def __init__(self, id_, title, group="", blurb="", tags=None,
                 sources=None, previews=None, code=None, links=None):
        self.id = id_
        self.title = title
        self.group = group
        self.blurb = blurb
        self.tags = list(tags or [])
        self.sources = list(sources or [])
        self.previews = list(previews or [])
        self.code = list(code or [])
        self.links = list(links or [])


def make_preview(spec, label=""):
    if isinstance(spec, str):
        return Preview(ROOT / spec, label)
    return Preview(
        ROOT / spec["src"], spec.get("label", label),
        spec.get("backdrop"), bool(spec.get("detail")),
        bool(spec.get("embed")), int(spec.get("height", 380)),
    )


def glob_labels(pattern: str, hit: Path) -> str:
    """Derive a preview label from '*' capture groups in the glob."""
    rx = re.escape(pattern).replace(r"\*", "(.*)")
    m = re.fullmatch(rx, relpos(hit))
    if not m:
        return hit.stem
    return " / ".join(g.replace("-", " ").replace("_", " ") for g in m.groups() if g)


def collect_cards(cfg: dict, blurbs: dict[str, str]) -> list[Card]:
    cards: list[Card] = []

    for group_cfg in cfg.get("groups", []):
        group = group_cfg.get("label", "")
        source_files: set[Path] = set()
        for pattern in group_cfg.get("sources", []):
            source_files.update(p for p in ROOT.glob(pattern) if p.is_file())
        artifact_specs = group_cfg.get("artifacts", [])
        for source in sorted(source_files, key=lambda p: natural_key(p.name)):
            stem = source.stem
            rel_source = relpos(source)
            previews = []
            for spec in artifact_specs:
                pattern = spec["glob"] if isinstance(spec, dict) else spec
                hits = sorted(ROOT.glob(pattern.format(stem=stem)),
                              key=lambda p: natural_key(p.name))
                for hit in hits:
                    if hit.is_file():
                        if isinstance(spec, dict):
                            previews.append(Preview(
                                hit, spec.get("label", ""), spec.get("backdrop"),
                                bool(spec.get("detail")), bool(spec.get("embed")),
                                int(spec.get("height", 380))))
                        else:
                            previews.append(Preview(hit))
            title = group_cfg.get("titles", {}).get(stem) or humanize(stem)
            cards.append(Card(stem, title, group=group,
                              blurb=blurbs.get(rel_source, ""),
                              sources=[source], previews=previews))

    for item in cfg.get("items", []):
        if not item.get("id"):
            fail("an item is missing an id")
        sources = [ROOT / s for s in item.get("sources", [])]
        previews = [make_preview(p) for p in item.get("previews", [])]
        for spec in item.get("previewGlobs", []):
            pattern = spec["glob"] if isinstance(spec, dict) else spec
            for hit in sorted(ROOT.glob(pattern), key=lambda p: natural_key(p.name)):
                if hit.is_file():
                    label = glob_labels(pattern, hit) if isinstance(spec, dict) else ""
                    previews.append(Preview(
                        hit, label,
                        spec.get("backdrop") if isinstance(spec, dict) else None))
        code_previews = []
        for code in item.get("code", []):
            path = ROOT / code["src"]
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                continue
            code_previews.append(CodePreview(code.get("label", path.name), text))
        cards.append(Card(
            item["id"], item.get("title") or humanize(item["id"]),
            group=item.get("group", ""), blurb=item.get("blurb", ""),
            tags=item.get("tags", []), sources=sources, previews=previews,
            code=code_previews, links=item.get("links", []),
        ))

    seen: set[str] = set()
    unique: list[Card] = []
    for card in cards:
        if card.id in seen:
            continue
        seen.add(card.id)
        unique.append(card)
    unique.sort(key=lambda c: (natural_key(c.group), natural_key(c.id)))
    return unique


# --------------------------------------------------------------------------
# page

CSS = """
:root{color-scheme:light;
  --bg:#fbfaf8;--panel:#fff;--ink:#16181d;--muted:#5f6570;--faint:#626b77;
  --line:#e3e1dc;--accent:#2b6cb0;--chip:#f1f4f8;--code:#f6f5f2;
  --shadow:0 1px 2px rgba(20,22,26,.05),0 8px 24px -12px rgba(20,22,26,.18);}
@media (prefers-color-scheme:dark){:root:not([data-theme=light]){color-scheme:dark;
  --bg:#0f1114;--panel:#161a1f;--ink:#e8eaee;--muted:#a2abb8;--faint:#8b96a6;
  --line:#262c34;--accent:#63b3ed;--chip:#1d232a;--code:#12161a;
  --shadow:0 1px 2px rgba(0,0,0,.4),0 10px 30px -14px rgba(0,0,0,.7);}}
:root[data-theme=dark]{color-scheme:dark;
  --bg:#0f1114;--panel:#161a1f;--ink:#e8eaee;--muted:#a2abb8;--faint:#8b96a6;
  --line:#262c34;--accent:#63b3ed;--chip:#1d232a;--code:#12161a;
  --shadow:0 1px 2px rgba(0,0,0,.4),0 10px 30px -14px rgba(0,0,0,.7);}
:root[data-theme=light]{color-scheme:light;
  --bg:#fbfaf8;--panel:#fff;--ink:#16181d;--muted:#5f6570;--faint:#626b77;
  --line:#e3e1dc;--accent:#2b6cb0;--chip:#f1f4f8;--code:#f6f5f2;}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--ink);
  font:15px/1.6 ui-sans-serif,-apple-system,"Segoe UI",Inter,system-ui,sans-serif;
  -webkit-font-smoothing:antialiased;transition:background-color .25s ease,color .25s ease}
.wrap{max-width:1240px;margin:0 auto;padding:48px 28px 90px}
header{border-bottom:1px solid var(--line);padding-bottom:24px;margin-bottom:28px}
h1{margin:0;font-size:30px;letter-spacing:-.02em;font-weight:650}
.tagline{margin:10px 0 0;color:var(--muted);max-width:74ch}
.pill-row{display:flex;gap:10px;flex-wrap:wrap;margin-top:18px}
.pill{font:12px/1 ui-monospace,SFMono-Regular,Menlo,monospace;color:var(--muted);
  background:var(--chip);border:1px solid var(--line);border-radius:999px;padding:7px 12px}
.pill b{color:var(--ink)}
.controls{position:sticky;top:0;z-index:5;display:flex;flex-wrap:wrap;gap:10px;
  align-items:center;padding:12px 0;margin-bottom:26px;background:var(--bg);
  box-shadow:0 1px 0 var(--line)}
.controls input[type=search]{flex:1 1 220px;max-width:420px;border:1px solid var(--line);
  border-radius:10px;background:var(--panel);color:var(--ink);padding:9px 13px;
  font:inherit;font-size:14px}
.controls input:focus-visible,.chip:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.chips{display:flex;gap:6px;flex-wrap:wrap}
.chip{font:12px/1 ui-monospace,Menlo,monospace;color:var(--muted);cursor:pointer;
  background:var(--chip);border:1px solid var(--line);border-radius:999px;padding:7px 12px}
.chip span{color:var(--faint);margin-left:4px}
.chip[aria-pressed=true]{background:var(--accent);border-color:var(--accent);color:var(--bg)}
.chip[aria-pressed=true] span{color:var(--bg)}
#count{margin-left:auto;color:var(--muted);font:12px/1 ui-monospace,Menlo,monospace;white-space:nowrap}
.theme-btn{border:1px solid var(--line);background:var(--panel);color:var(--ink);
  border-radius:999px;padding:7px 16px;font:inherit;font-size:13px;cursor:pointer}
.theme-btn:hover,.theme-btn:focus-visible{border-color:var(--accent);color:var(--accent);outline:none}
.theme-btn::after{content:"Light"}
:root[data-theme=dark] .theme-btn::after{content:"Dark"}
.grid{display:grid;gap:24px;grid-template-columns:repeat(auto-fill,minmax(min(100%,360px),1fr))}
.card{background:var(--panel);border:1px solid var(--line);border-radius:14px;overflow:hidden;
  box-shadow:var(--shadow);display:flex;flex-direction:column;
  transition:background-color .25s ease,border-color .25s ease}
.shots{display:flex;flex-direction:column;gap:10px;padding:14px 14px 0}
.shot{position:relative;display:flex;align-items:center;justify-content:center;
  min-height:160px;border:1px solid var(--line);border-radius:10px;overflow:hidden;background:var(--bg)}
.shot img{display:block;width:100%;height:auto;max-height:460px;object-fit:contain}
.shot iframe{display:block;width:100%;border:0;background:var(--panel)}
.shot.checker{background-color:#f2f1ee;background-image:
  linear-gradient(45deg,#e2e0dc 25%,transparent 25%,transparent 75%,#e2e0dc 75%),
  linear-gradient(45deg,#e2e0dc 25%,transparent 25%,transparent 75%,#e2e0dc 75%);
  background-size:16px 16px;background-position:0 0,8px 8px}
.shot.light{background:#fff}
.shot.dark{background:#11151a}
.shot figcaption{position:absolute;left:8px;top:8px;font:10px/1 ui-monospace,Menlo,monospace;
  color:var(--muted);background:var(--panel);border:1px solid var(--line);
  border-radius:999px;padding:3px 8px;opacity:.94;z-index:1}
.shot figcaption.flag{left:auto;right:8px}
.proof{padding:0 14px}
.proof summary{position:static;border:0;padding:8px 0 2px}
.missing{color:var(--faint);font-style:italic;font-size:13px;padding:40px 10px;text-align:center}
.body{padding:16px 18px 18px;display:flex;flex-direction:column;gap:9px;flex:1}
.idx{font:11px/1 ui-monospace,Menlo,monospace;color:var(--faint);
  letter-spacing:.08em;text-transform:uppercase}
h2{margin:0;font-size:16px;font-weight:620;letter-spacing:-.01em}
.blurb{margin:0;color:var(--muted);font-size:13.5px}
.tags{display:flex;flex-wrap:wrap;gap:6px}
.tag{font:11px/1 ui-monospace,Menlo,monospace;color:var(--muted);
  background:var(--chip);border-radius:5px;padding:4px 7px}
.links{display:flex;flex-wrap:wrap;gap:14px;margin-top:2px}
.links a{color:var(--accent);font-size:12.5px;text-decoration:none}
.links a:hover,.links a:focus-visible{text-decoration:underline}
.card details{margin-top:auto;border-top:1px solid var(--line);padding-top:10px}
.card details+details{border-top:0;margin-top:4px}
.card details.proof{border-top:0;margin-top:0;padding:0}
summary{cursor:pointer;font:12px/1 ui-monospace,Menlo,monospace;color:var(--accent);
  list-style:none;user-select:none}
summary::-webkit-details-marker{display:none}
summary::before{content:"\\25b8 ";display:inline-block}
details[open] summary::before{content:"\\25be "}
pre{margin:10px 0 0;background:var(--code);border:1px solid var(--line);border-radius:8px;
  padding:12px 14px;overflow:auto;max-height:380px;
  font:11.5px/1.55 ui-monospace,Menlo,monospace;color:var(--ink)}
.codeblock{margin:0 14px}
.codeblock pre{max-height:260px;margin:6px 0 0}
#empty{display:none;margin:40px 0;color:var(--muted)}
footer{margin-top:52px;padding-top:20px;border-top:1px solid var(--line);color:var(--faint);font-size:13px}
footer code{background:var(--chip);border-radius:4px;padding:2px 6px;font:12px Menlo,monospace}
.sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0)}
@media (prefers-reduced-motion:reduce){*,*::before,*::after{transition:none!important;animation:none!important}}
"""

BOOT = (
    "(function(){var s=null;try{s=localStorage.getItem('plot-gallery-theme');}catch(e){}"
    "var d=s==='dark'||s==='light'?s==='dark':matchMedia('(prefers-color-scheme: dark)').matches;"
    "document.documentElement.dataset.theme=d?'dark':'light';})();"
)

SCRIPT = r"""
(function(){
var root=document.documentElement,
btn=document.querySelector('[data-action=theme]'),
q=document.getElementById('q'),count=document.getElementById('count'),
empty=document.getElementById('empty'),
cards=[].slice.call(document.querySelectorAll('.card')),
chips=[].slice.call(document.querySelectorAll('.chip')),group='';
function apply(){
var t=q.value.trim().toLowerCase(),n=0;
cards.forEach(function(c){
var ok=(!group||c.dataset.group===group)&&(!t||(c.dataset.search||'').toLowerCase().indexOf(t)>=0);
c.style.display=ok?'':'none';if(ok)n++;});
count.textContent=n+' / '+cards.length+' shown';
empty.style.display=n?'none':'block';
chips.forEach(function(ch){ch.setAttribute('aria-pressed',String(ch.dataset.group===group));});
}
btn.addEventListener('click',function(){
var next=root.dataset.theme==='dark'?'light':'dark';
root.dataset.theme=next;try{localStorage.setItem('plot-gallery-theme',next);}catch(e){}
});
chips.forEach(function(ch){ch.addEventListener('click',function(){
var g=ch.dataset.group;group=(g===group)?'':g;apply();});});
q.addEventListener('input',apply);apply();
})();
"""


def esc(s: str) -> str:
    return html.escape(str(s), quote=True)


def cap_lines(text: str, limit: int) -> str:
    lines = text.rstrip("\n").split("\n")
    if len(lines) > limit:
        lines = lines[:limit] + [f"... ({len(lines) - limit} more lines)"]
    return "\n".join(lines)


def source_block(path: Path) -> str:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return ""
    return (f"<details><summary>{esc(relpos(path))}</summary>"
            f"<pre><code>{esc(cap_lines(text, MAX_SOURCE_LINES))}</code></pre></details>")


def render_card(card: Card, ordinal: int, default_backdrop: str) -> str:
    images = [p for p in card.previews if p.path.suffix.lower() in IMAGE_EXT and not p.detail]
    proofs = [p for p in card.previews if p.path.suffix.lower() in IMAGE_EXT and p.detail]
    frames = [p for p in card.previews if p.path.suffix.lower() in (".html", ".htm")]

    shots: list[str] = []
    for preview in images:
        backdrop = preview.backdrop or default_backdrop
        cap = f"<figcaption>{esc(preview.label)}</figcaption>" if preview.label else ""
        shots.append(
            f'<figure class="shot {backdrop}">{cap}'
            f'<img src="{esc(relpos(preview.path))}" loading="lazy" '
            f'alt="{esc(card.title + (" — " + preview.label if preview.label else ""))}"></figure>')
    for preview in frames:
        cap = f"<figcaption>{esc(preview.label)}</figcaption>" if preview.label else ""
        shots.append(
            f'<figure class="shot">{cap}'
            f'<iframe src="{esc(relpos(preview.path))}" title="{esc(card.title)}" '
            f'loading="lazy" height="{preview.height}"></iframe></figure>')
    if not shots and not card.code and not proofs:
        shots.append('<figure class="shot"><span class="missing">no rendered preview — run the rebuild</span></figure>')

    proof_html = ""
    if proofs:
        items = "".join(
            f'<figure class="shot {p.backdrop or "light"}">'
            f'<figcaption class="flag">{esc(p.label or "proof")}</figcaption>'
            f'<img src="{esc(relpos(p.path))}" loading="lazy" alt="{esc(card.title)} proof"></figure>'
            for p in proofs)
        proof_html = f'<details class="proof"><summary>Raster proof ({len(proofs)})</summary>{items}</details>'

    code_html = "".join(
        f'<details class="codeblock" open><summary>{esc(c.label)}</summary>'
        f"<pre><code>{esc(cap_lines(c.text, MAX_CODE_LINES))}</code></pre></details>"
        for c in card.code)

    tags = "".join(f'<span class="tag">{esc(t)}</span>' for t in card.tags)
    sources = "".join(source_block(p) for p in card.sources if p.is_file())
    link_html = "".join(
        f'<a href="{esc(link["href"])}">{esc(link.get("label", link["href"]))}</a>'
        for link in card.links)
    search_bits = [card.title, card.blurb, card.group, " ".join(card.tags)]
    for source in card.sources:
        if source.is_file():
            search_bits.append(content_words(source.read_text(encoding="utf-8", errors="ignore")))
    eyebrow = f"{ordinal:02d}" + (f" · {esc(card.group)}" if card.group else "")
    return (
        f'<article class="card" data-group="{esc(card.group)}" data-search="{esc(" ".join(search_bits).casefold())}">'
        f'<div class="shots">{"".join(shots)}</div>'
        f'{proof_html}{code_html}'
        f'<div class="body">'
        f'<div class="idx">{eyebrow}</div>'
        f'<h2>{esc(card.title)}</h2>'
        + (f'<p class="blurb">{esc(card.blurb)}</p>' if card.blurb else "")
        + (f'<div class="tags">{tags}</div>' if tags else "")
        + (f'<nav class="links" aria-label="Links">{link_html}</nav>' if link_html else "")
        + sources
        + '</div></article>'
    )


# --------------------------------------------------------------------------
# wrap mode (colored terminal fragments -> standalone pages)

WRAP_CSS = """
body{margin:0;padding:24px;background:#0f1114}
pre{margin:0;font:13px/1.55 ui-monospace,SFMono-Regular,Menlo,monospace;
  color:#c9d1d9;white-space:pre;overflow:auto;tab-size:4}
@media (prefers-color-scheme:light){:root:not([data-theme=light]) body{background:#0f1114}}
"""


def wrap_page(stem: str, variant: str, text_mode: bool, fragment: str) -> str:
    title = stem + (f" · {variant}" if variant else "")
    if text_mode:
        body = "<pre>" + esc(fragment) + "</pre>"
        bg = "#fbfaf8"
        fg = "#16181d"
        css = ("body{margin:0;padding:24px;background:%s}pre{margin:0;"
               "font:13px/1.55 ui-monospace,Menlo,monospace;color:%s;white-space:pre;overflow:auto;tab-size:4}"
               % (bg, fg))
    else:
        # Colored fragment from `termdiag --color html`: already escaped HTML.
        body = fragment.strip()
        if not body.lstrip().lower().startswith("<pre"):
            body = "<pre>" + body + "</pre>"
        css = WRAP_CSS
    return (
        '<!doctype html><html lang="en"><head><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width,initial-scale=1">'
        f"<title>{esc(title)}</title><style>{css}</style></head><body>{body}</body></html>\n"
    )


def build(cfg: dict, check: bool) -> int:
    tracked = tracked_files()
    blurbs = readme_blurbs()
    cards = collect_cards(cfg, blurbs)
    if not cards:
        fail("no examples discovered — check gallery.json groups/items")

    missing: list[str] = []
    uncommitted: list[str] = []
    rendered = 0
    for card in cards:
        existing = [p for p in card.previews if p.path.exists()]
        missing.extend(relpos(p.path) for p in card.previews if not p.path.exists())
        card.previews = existing
        if existing or card.code:
            rendered += 1
        for preview in existing:
            rp = relpos(preview.path)
            if tracked and rp not in tracked:
                uncommitted.append(rp)
    for path in sorted(set(missing)):
        print(f"gallery: missing preview (skipped): {path}", file=sys.stderr)
    for path in sorted(set(uncommitted)):
        print(f"gallery: preview not committed (blank on fresh clone): {path}", file=sys.stderr)

    groups: dict[str, int] = {}
    for card in cards:
        if card.group:
            groups[card.group] = groups.get(card.group, 0) + 1
    ordered = sorted(groups.items(), key=lambda kv: (-kv[1], kv[0].casefold()))
    chips = ""
    if len(ordered) >= 2 and len(ordered) * 2 <= len(cards):
        buttons = [f'<button class="chip" type="button" data-group="" aria-pressed="true">All <span>{len(cards)}</span></button>']
        for name, count in ordered:
            buttons.append(f'<button class="chip" type="button" data-group="{esc(name)}">{esc(name)} <span>{count}</span></button>')
        chips = f'<div class="chips" role="group" aria-label="Filter by group">{"".join(buttons)}</div>'

    cards_html = "\n".join(render_card(card, i, cfg.get("backdrop", "checker"))
                           for i, card in enumerate(cards, 1))
    pills = [f'<span class="pill"><b>{len(cards)}</b> examples</span>',
             f'<span class="pill"><b>{rendered}</b> rendered</span>']
    page = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<script>{BOOT}</script>
<title>{esc(cfg["title"])} — gallery</title>
<style>{CSS}</style>
</head>
<body>
<div class="wrap">
  <header>
    <h1>{esc(cfg["title"])}</h1>
    <p class="tagline">{esc(cfg.get("tagline", ""))}</p>
    <div class="pill-row">{"".join(pills)}</div>
  </header>
  <section class="controls" aria-label="Gallery filters">
    <label class="sr" for="q">Filter examples</label>
    <input id="q" type="search" placeholder="Filter by title, tag, group, or source content…" autocomplete="off">
    {chips}
    <output id="count" aria-live="polite">{len(cards)} / {len(cards)} shown</output>
    <button class="theme-btn" type="button" data-action="theme" aria-label="Switch colour theme" aria-pressed="false"></button>
  </section>
  <main class="grid">
{cards_html}
  </main>
  <p id="empty">No examples match the current filter.</p>
  <footer>Generated by <code>tools/gallery.py</code> — rebuild with <code>{esc(cfg.get("rebuild", "just rust-gallery"))}</code>. Previews are relative paths committed with the repo.</footer>
</div>
<script>{SCRIPT}</script>
</body>
</html>
"""

    output = ROOT / cfg.get("output", "gallery.html")
    if check:
        current = output.read_text(encoding="utf-8") if output.exists() else None
        if current != page:
            print(f"gallery drift: {output.name} is stale; rerun: "
                  + cfg.get("rebuild", "just rust-gallery"), file=sys.stderr)
            return 1
        print(f"gallery.html current ({len(cards)} examples, {rendered} rendered)")
        return 0
    output.write_text(page, encoding="utf-8")
    note = f", {len(set(uncommitted))} preview(s) not committed" if uncommitted else ""
    print(f"gallery.html: {len(cards)} examples, {rendered} rendered{note}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="fail unless gallery.html is current")
    sub = parser.add_subparsers(dest="command")
    w = sub.add_parser("wrap", help="wrap an HTML/pre fragment on stdin")
    w.add_argument("--stem", required=True)
    w.add_argument("--variant", default="")
    w.add_argument("--text", action="store_true")
    w.add_argument("-o", "--output", required=True)
    args = parser.parse_args()

    if args.command == "wrap":
        fragment = sys.stdin.read()
        page = wrap_page(args.stem, args.variant, args.text, fragment)
        out = Path(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        tmp = out.with_name("." + out.name + ".tmp")
        tmp.write_text(page, encoding="utf-8")
        tmp.replace(out)
        return 0

    if not CONFIG.exists():
        fail(f"missing {CONFIG.relative_to(ROOT)}")
    cfg = json.loads(CONFIG.read_text(encoding="utf-8"))
    return build(cfg, args.check)


if __name__ == "__main__":
    sys.exit(main())
