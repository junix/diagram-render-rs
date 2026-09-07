# diagram-render-rs 可审计技术长图 · 交付树说明

本树是引擎仓 `diagram-render-rs` 的「可审计技术长图」交付物：一张 1200 CSS px
宽、零 JavaScript、零外链、自包含的中文长图（`index.html`），页面上的每一个
数字都来自 `data/frozen/` 中一次性冻结的真实实测，并可由确定性重建层与四道
门禁复核。

位置铁律：本树只存在于
`docs/infographics/diagram-render-rs-explainer/`；引擎仓其余部分只读（唯一
容许的 porcelain 变化是本树的未跟踪文件）；全部构建与临时产物落在
`/tmp/ign-drr/`，引擎仓不产生任何 `target/` 残留。

## 快照锚点

| 项 | 值 |
|---|---|
| 引擎 HEAD | `b38ba079257a530691b8d2c700586fee5fb810ef` |
| 冻结时间 (UTC) | 2026-09-05T14:20:03Z |
| 冻结时引擎 porcelain | 仅 `?? docs/`（本树自身，未跟踪） |
| rustc / cargo | 1.98.0 (Homebrew) |
| Cargo.lock sha256 | 见 `data/frozen/engine-snapshot.txt` |

引擎守卫（`tools/common.py: guard_engine`）两态区分，防「提交后自咬」：

- **frozen**：HEAD 仍等于上表值，且 porcelain 除本树外干净（本树条目无论
  未跟踪还是已跟踪修改均容忍）。此时若快照被就地改动（HEAD 相同但树脏），
  判「证据漂移」，硬失败。
- **evolved**：HEAD 已越过冻结值。这是交付提交后的常态（交付提交本身就会
  前进 HEAD），不是证据漂移：警告并给出冻结工作树配方（见下）。证据仍受
  保护——所有重建环节与冻结层做字节哈希对照，任何失配硬失败。
- B1 六禁令语料钉在冻结 `FROZEN_HEAD` 上（`git ls-tree`/`git show` 只读
  取证），不随引擎演进而变；交付树自身路径结构上进不了语料（冻结快照早于
  本树）。`rebuild.py` 要求 frozen 态（ evolved 态直接拒绝并指向配方），
  因为重建层的契约是与冻结层逐字节一致，只有冻结快照能产出。

## 目录结构

```
diagram-render-rs-explainer/
  index.html            长图页面（1200 CSS px，零 JS，零外链）
  panels/               4 张数据驱动 SVG 面板（由 tools/page.py 生成）
  renders/              full@2x.png / grayscale.png / thumb.png
  data/frozen/          一次性冻结证据（13 个文件 + artifacts/ 14 份产物）
  data/rebuild/         确定性重建层（重跑逐字节一致）
  data/audit/           提交后门禁实跑记录（指纹豁免，B4 不校验）
  tools/                本树全部工具（纯标准库 Python 3）
  fingerprints.sha256   全树产物 sha256 清单（不含清单自身与 data/audit/）
  VERIFICATION.md       验证记录：数字锚点 / 裁定 / 门禁 / 偏差
```

## 环境变量

| 变量 | 缺省 | 作用 |
|---|---|---|
| `DRR_ENGINE` | 无 | 引擎仓路径；命令行 `--engine` 优先于它 |
| `DRR_CARGO_TARGET_DIR` | `/tmp/ign-drr/target-rebuild` | 重建层 cargo 构建目录；绝不写引擎 `target/` |
| `DRR_CHROME` | `~/Library/Caches/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-mac-arm64/chrome-headless-shell` | 截图浏览器：固定版 chrome-headless-shell（舰队配方，非系统 Chrome） |
| `DRR_MAGICK` | `/opt/homebrew/bin/magick` | PNG 拼接 / 灰度 / 缩略 / 去元数据（`-strip`） |
| `DRR_SVG_LINTER` | `svg-linter`（PATH 查找） | B2 门禁的 SVG 检查器 |
| `PYTHONDONTWRITEBYTECODE` | 建议恒为 `1` | 保证树内零 `.pyc` |

工具自定位：所有工具都接受显式 `--tree`（推荐，真空复跑即依赖它）；缺省取
`tools/` 的上级目录，且该目录名必须恰为 `diagram-render-rs-explainer`，否则
拒绝运行。

## 工具清单（tools/，均为纯标准库 Python 3）

| 工具 | 一次性 / 可重跑 | 作用 |
|---|---|---|
| `freeze.py` | 一次性（拒绝覆盖） | 冻结引擎快照、离线发布构建全量转录、全量测试、代码行数、CLI 表面、七格式真实渲染转录与产物、结构化统计、PNG 双跑对照、二次构建对照、特性矩阵统计 |
| `freeze_probes.py` | 一次性（拒绝覆盖） | 护栏探针：缩放闸门上下界、单边画幅上限、像素总量上限、精确宽度、默认透明、XML 转义、unsafe 扫描、依赖清单 |
| `freeze_docs.py` | 一次性（拒绝覆盖） | 引擎公开 README 的字节快照（架构叙述的锚点） |
| `page.py` | 可重跑（逐字节稳定） | 从冻结证据生成 `panels/*.svg` 与 `index.html`；页面上没有硬编码数字 |
| `screenshot.py` | 可重跑 | CDP 驱动 chrome-headless-shell 分片截图、断言、magick 拼接与三件产物 |
| `rebuild.py` | 可重跑 | 完整确定性链：引擎校验 → 离线重建二进制（哈希对齐冻结值）→ 重渲染七样例（哈希对齐冻结产物）→ 重生成页面与截图 → 重写指纹清单 |
| `gates.py` | 可重跑 | 四道交付门禁（见下） |

## 复核方法

冻结工作树配方（引擎 HEAD 已越过 `b38ba07…` 时的正确复核姿势；交付
提交后即处于此态）：

```sh
# 1) 冻结工作树：把冻结快照检出为独立工作树（只增加 .git/worktrees
#    管理数据，不动主检出）
git -C ~/projects/plot/diagram-render-rs worktree add \
    /tmp/ign-drr/frozen-engine b38ba079257a530691b8d2c700586fee5fb810ef

# 2) 完整确定性重建 + 四道门禁（--tree 仍指向主检出里的交付树）
cd <交付树>
PYTHONDONTWRITEBYTECODE=1 python3 tools/rebuild.py \
    --engine /tmp/ign-drr/frozen-engine --tree .
PYTHONDONTWRITEBYTECODE=1 python3 tools/gates.py \
    --engine /tmp/ign-drr/frozen-engine --tree .

# 3) 用后清理
git -C ~/projects/plot/diagram-render-rs worktree remove \
    /tmp/ign-drr/frozen-engine
```

引擎仍停在冻结 HEAD 时（或上面的工作树里），等价于：

```sh
# 完整确定性重建（引擎校验 + 离线构建 + 重渲染 + 页面 + 截图 + 指纹）
PYTHONDONTWRITEBYTECODE=1 python3 tools/rebuild.py \
    --engine ~/projects/plot/diagram-render-rs --tree .

# 四道门禁（B1 六禁令扫描 / B2 svg-linter / B3 真空复跑 / B4 指纹机检）
PYTHONDONTWRITEBYTECODE=1 python3 tools/gates.py \
    --engine ~/projects/plot/diagram-render-rs --tree .
```

对已演进的引擎仓不建工作树的降级跑法：
`gates.py --engine ~/projects/plot/diagram-render-rs --tree . --skip-vacuum`
—— prelude/B1（语料钉冻结 HEAD）/B2/B4 照常可跑；B3 真空复跑与
`rebuild.py` 需要冻结快照，走上面的工作树配方。提交后的实跑结果记录进
`data/audit/post-commit.md`（指纹豁免，见该文件说明）。

- 冻结层（`data/frozen/`）一次写入永不改写：任何冻结工具在目标文件已存在时
  直接拒绝运行；复核只走重建层与门禁，绝不回头改数。
- 重建层转录做归一化：`<ts>` 时间戳、`<dur>` 时长、`<work>` 临时目录路径；
  其余字节保持原样。双跑必须逐字节一致。
- 构建全程 `cargo --offline`（依赖取自本机缓存，零网络）；`CARGO_TARGET_DIR`
  恒指 `/tmp`。

## 渲染配方（本机舰队稳定版）

- 浏览器：固定版 chrome-headless-shell（见 `DRR_CHROME`）；不用
  `--headless=new`；附加 `--disable-gpu --force-color-profile=srgb`。
- 分片：视口 1200×800、`deviceScaleFactor 2`；每片先 `scrollTo` 再读回
  `scrollY` 断言，然后截屏；`magick -append` 纵向拼接。
- 断言：拼接图必须恰为 2400×(页面 CSS 高×2)；产物一律 `-strip` 并加
  `-define png:exclude-chunk=time`（ImageMagick 的 PNG 编码器在 `-strip`
  下仍会写 tIME 时间戳块），保证双跑字节一致。

门禁结果、页面数字逐项锚点、允许项裁定与偏差披露全部记录在
`VERIFICATION.md`。
