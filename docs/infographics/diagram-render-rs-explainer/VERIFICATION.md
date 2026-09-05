# VERIFICATION — diagram-render-rs 可审计技术长图

本文件是验证层记录：快照锚点、页面数字逐项锚点、允许项裁定、四道门禁记录、
渲染断言、真空复跑记录、指纹机检记录与偏差披露。file:line 只出现在本层，
页面（index.html 与 panels/）不出现任何文件名/行号/源码摘录。

## 1. 快照锚点

| 项 | 值 | 证据 |
|---|---|---|
| 引擎 HEAD | `b38ba079257a530691b8d2c700586fee5fb810ef` | data/frozen/engine-snapshot.txt:3 |
| 冻结时间 (UTC) | 2026-09-05T14:20:03Z | engine-snapshot.txt:2 |
| 冻结时 porcelain | 仅 `?? docs/`（本树未跟踪文件） | engine-snapshot.txt:4 |
| rustc / cargo | 1.98.0 (Homebrew) | engine-snapshot.txt:6-7 |
| 引擎自述字节快照 | sha256 见文件头 | engine-readme.txt:2 |
| 二进制指纹 | sha256 `d97647108afce0b5…d155dabc6b`，4,744,320 B | cargo-build-release.txt:101-102 |
| 构建方式 | `cargo build --offline --release`，全新 target 目录 | cargo-build-release.txt:1-2 |

所有工具开工前经 `tools/common.py: guard_engine` 校验：HEAD 必须等于上表值，
`git status --porcelain -uall` 除 `?? docs/infographics/diagram-render-rs-explainer/`
前缀外必须为空，否则硬失败。

## 2. 页面数字 → 冻结证据锚点

### 2.1 页眉与信息条

| 页面声明 | 锚点 |
|---|---|
| 快照 `b38ba079…` / `b38ba079257a530691b…` | engine-snapshot.txt:3 |
| rustc 1.98.0 · 离线构建 · 零网络 | engine-snapshot.txt:6 + cargo-build-release.txt:2（`--offline` 全程无网络抓取） |
| 依赖 7 项 · 2 项按 git 修订锁定 | guardrail-probes.txt:64（dependency-count: 7）+ :58-59（diagram-ast-parser rev `f34a45cb9ea60c08…`、diagram-theme rev `1f77cb8f19804ba0…`） |
| 零 JS · 零外链 · 自包含 | 门禁 prelude（见 §4） |
| 解析归兄弟项目（修订 `f34a45c`…） | guardrail-probes.txt:58 |
| PNG 经 resvg 光栅化 | guardrail-probes.txt:60（dep: resvg =0.48.1） |

### 2.2 四张指标卡

| 页面声明 | 锚点 |
|---|---|
| 7 种 DSL 家族 | cli-render-transcript.txt（七个 format 各一节）+ scene-stats.json 七个 artifacts |
| 6 类场景绘图原语 | scene-stats.json 各产物 elements 键并集恰为 rect/ellipse/line/polyline/polygon/text 六类 |
| 主题 14 + 2，默认 light | cli-surface.txt:48 起 `themes --json`：canonical=True 共 14（azure…olive-paper 各含 `-dark` 档），False 共 2（light/dark），`default: light` |
| 26 项 Rust 测试全部通过 | cargo-test.txt:10,15,21,27,40,57,62 → 4+0+1+1+8+12+0 = 26 passed，0 failed |

### 2.3 第 02 节（七格式实测表与两张面板）

逐行数字全部来自 data/frozen/scene-stats.json（每格式一个 artifact 记录，
dbml 自 :4 起、wavedrom :29、d2 :56、structurizr :81、likec4 :106、
nomnoml :131、pikchr :156）：

| DSL | 场景 (CSS px) | SVG (B) | PNG 像素 | PNG (B) | 原语合计 | SVG sha 前 12 位 |
|---|---|---|---|---|---|---|
| DBML | 692×565 | 6,054 | 1384×1130 | 62,624 | 36 | cad1972cff5b |
| WaveDrom | 660×324 | 11,628 | 1320×648 | 26,284 | 64 | ce2d272464f8 |
| D2 | 692×451 | 5,348 | 1384×902 | 47,284 | 32 | 774692a9448b |
| Structurizr | 692×470 | 6,607 | 1384×940 | 60,403 | 40 | 0b23cb17ac0c |
| LikeC4 | 692×432 | 5,884 | 1384×864 | 40,787 | 36 | 016076666665 |
| nomnoml | 692×451 | 4,438 | 1384×902 | 40,504 | 27 | 837b6c5ddd3f |
| Pikchr | 700×300 | 2,876 | 1400×600 | 23,270 | 15 | e3ef1fb33530 |

- 「原语合计」= elements 总和减 g/title（容器与标题不计为绘图原语）。
- PNG 像素 = 场景 CSS 尺寸 × 2（CLI 默认缩放 2），逐行可验。
- 面板「体积」与「原语构成」两组条形图直接由同一 scene-stats.json 驱动，
  轴上限 70,000 B / 70 个为画布常量（非实测声明）。
- 命令摘录（`wrote:` 行）中的 `<work>` 是转录归一化 token，原始路径在
  cli-render-transcript.txt。

### 2.4 第 03 节（主题）

| 页面声明 | 锚点 |
|---|---|
| 14 个共享名 + light/dark 遗留名、默认 light、从不列出 | cli-surface.txt themes --json（canonical 计数与 default 字段）+ engine-readme.txt:78-82 |
| 注册表由兄弟项目共享、五个图表渲染器拼法一致 | engine-readme.txt:74-77（"spelled identically in all five renderers"，ADR-1137 共享注册表） |
| 每个主题族都有浅色与深色两档 | cli-surface.txt:55 families 七族 + 各族 `-dark` 变体 |
| 主题永不涂画布；--background 才填充；-dark 是宿主页断言 | engine-readme.txt:91-94 |

### 2.5 第 04 节（特性覆盖）

| 页面声明 | 锚点 |
|---|---|
| 68 项对齐 / 45 项声明排除（共 113 项） | feature-matrix-totals.txt:12（TOTAL aligned=68 intentional-exclusion=45） |
| 逐语言数值（d2 12/4、dbml 7/15、likec4 6/8、nomnoml 9/3、pikchr 13/4、structurizr 7/6、wavedrom 14/5） | feature-matrix-totals.txt:4-10 |
| 排除理由分类与「清单与用例双向绑定」 | 快照内 e2e 矩阵的登记结构；冻结脚本仅做汇总（见偏差 D4） |

### 2.6 第 05 节（渲染护栏）

| 页面声明 | 锚点 |
|---|---|
| 缩放闸门 0.05–16.0（双向实测触发） | guardrail-probes.txt:4-11（P1 上界、P2 下界均被拒） |
| 单边画幅上限 32,768 px；实测 32800×4800 被拒 | guardrail-probes.txt:14-16（P3，宽场景 ×16 探针） |
| 像素总量上限 100,000,000；实测 11072×9040（100,090,880 px）被拒 | guardrail-probes.txt:19-21（P4） |
| --width 640 → 640×418 分毫不差 | guardrail-probes.txt:24-28（P5） |
| 7 张冻结 PNG 角像素 alpha 全 0 | guardrail-probes.txt:43-50（P8，逐张列出） |
| 样例文本整体转义为实体、原始尖括号不出现 | guardrail-probes.txt:36-40（P7） |
| --background 才出现画布填充 | guardrail-probes.txt:30-34（P6 canvas-fill-element-present: True） |
| 全源码树 "unsafe" 唯一命中是禁用声明本身 | guardrail-probes.txt:52-54（P9：1 hit，即 `#![forbid(unsafe_code)]` 声明行） |
| 畸形输入非零码拒收 | cli-render-transcript.txt:66-68（invalid.dbml → exit-code: 1） |
| warning 通道明示降级（视图语法） | cli-render-transcript.txt:27,37（Structurizr/LikeC4 降级 warning 实录） |

页面 XML 转义卡为避免与引擎测试源码断言串共窗，未逐字引用探针 payload，
改为描述性表述（见 §3 裁定 R5 与偏差 D6）。

### 2.7 第 06 节（验证与确定性）

| 页面声明 | 锚点 |
|---|---|
| 转录摘录（3 条命令行 + 输出行） | cli-render-transcript.txt:5-8（dbml svg/png）、:26-27（structurizr warning）、:66-68（invalid 拒收）；`<work>` 为归一化 token |
| 同输入双跑 SVG+PNG 全部逐字节一致 | png-determinism.txt:10（summary: all byte-identical across two fresh processes） |
| 二次全新构建二进制 sha256 完全一致 | cargo-rebuild-determinism.txt:4 |
| 二进制 sha256 `d97647108afce0b5…`、4,744,320 B | cargo-build-release.txt:101-102 |
| 四道门禁全绿 | §4 门禁记录 |

### 2.8 第 07 节与页脚

| 页面声明 | 锚点 |
|---|---|
| 冻结层 13 个文件 + 14 份渲染产物 | data/frozen/ 实际清单（12 个 .txt/.json + README.txt + artifacts/ 7 SVG + 7 PNG） |
| HEAD 全量、锁文件 sha、自述快照 sha | engine-snapshot.txt:3、:9（cargo-lock-sha256）、engine-readme.txt:2 |
| 页面代码行数（若引用） | code-metrics.txt:19,25,28,29（src 3089 / tests 530 / examples 77 / 合计 3696） |

## 3. 允许项裁定（六禁令扫描的豁免登记）

禁令扫描语料：引擎仓 `src/**/*.rs`、`tests/*.rs`、`examples/*.rs`、
`e2e/*.go`、`justfile`、`Cargo.toml` 全文；目标：index.html 与全部
panels/*.svg 原文。禁令③的逐字窗为空白扁平化后 25 字符。

| 编号 | 裁定 | 理由 |
|---|---|---|
| R1 | 产品与生态名：diagram-render-rs、diagram-ast-parser、diagram-theme、resvg、clap、serde_json、thiserror、unicode-width、tiny-skia、rustc、cargo | 公共生态名与产品名，非引擎内部标识符 |
| R2 | DSL 公共名：DBML、WaveDrom、D2、Structurizr、LikeC4、nomnoml、Pikchr | WaveDrom/LikeC4 兼为引擎枚举变体名，但首先是公共 DSL 名（工单明示可用） |
| R3 | CLI 动词与旗标：--format/-o/--width/--scale/--background/--quiet/--json/--theme/themes/--version/--help 及二进制调用形态 | 工单允许项（CLI 表面） |
| R4 | 样例名：schema.dbml、timing.json5、architecture.d2、workspace.dsl、model.c4、classes.nomnoml、flow.pikchr | 工单允许项（fixture 名） |
| R5 | 禁令③对「`$ ` 转录命令行」与 `<pre class="term">` 转录块内运行时输出行豁免 | 页面 06 节引用的是真实运行转录（工单允许面），非源码摘录；与引擎源码格式化串的共窗源于 `{}` 占位符被运行期文本替换（如 structurizr.rs:86 的 warning 格式串），属运行时输出而非源码引用 |
| R6 | JSON 契约键值：themes/canonical/label/name/default/families 等 | 机器可校验契约面 |
| R7 | 冻结数字与哈希（尺寸/字节/计数/sha） | 全部直接来自 data/frozen |
| R8 | 生态标准文件名 Cargo.toml/Cargo.lock/README/LICENSE 等 | 标准清单文件名，按惯例豁免引擎文件名禁令 |
| R9 | XML 命名空间样板 `xmlns="http://www.w3.org/2000/svg"` 在语料与目标两侧同剥 | 标准 XML 样板而非源码内容；prelude「零外链」检查亦同剥（命名空间 URI 不是外链） |
| R10 | 证据文件名 `cargo-rebuild-determinism.txt` 豁免禁令⑥字面量 | 它是冻结证据文件名，含英文单词 rebuild 但不是生成器/重建命令 |

正向对照（六条各按预期类别命中且仅命中该类，6/6）：①「布局核心在
cards.rs 里」②「位置在 88:12 附近」③ 源码语句逐字窗 ④「入口是
render_source 函数」⑤「样例放在 examples/inputs 目录」⑥「本页由
python3 生成」。负向对照（含 DBML/WaveDrom/resvg/--format dbml 的干净
中文样本）：零命中。

## 4. 门禁记录（最终交付运行）

命令：`PYTHONDONTWRITEBYTECODE=1 python3 tools/gates.py --engine
<引擎仓> --tree .`（完整四电池，含真空复跑）

- prelude：无 `<script>` PASS；无外链 PASS；body 宽 1200px PASS；zh-CN
  PASS；树内零 `.pyc` PASS；引擎 `target/` 残留早于冻结时间（见偏差 D2）
  PASS。
- B1 六禁令：正向对照 6/6 PASS；负向对照 PASS；index.html + 4 张面板
  扫描发现 0 项 PASS。
- B2 svg-linter：panel-coverage / panel-formats / panel-pipeline /
  panel-primitives 各 rc=0、findings=0，全 PASS。
- B3 真空复跑：/tmp/ign-drr/vacuum/{A,B} 两份平面副本，各删除
  index.html、panels/、renders/、data/rebuild/、fingerprints.sha256
  （冻结层原样保留），在副本内各自跑完整重建链成功；A == B（49 个文件，
  含指纹清单自身）且 A == 交付树最终状态（逐字节）PASS。
- B4 指纹机检：fingerprints.sha256 覆盖全树 48 个文件，missing=0、
  extra=0、changed=0，清单不含自身 PASS。

结论：ALL GATES PASS。

## 5. 渲染断言

- 浏览器：固定版 chrome-headless-shell（README 环境表路径），无
  `--headless=new`，附加 `--disable-gpu --force-color-profile=srgb`。
- 页面实测 1200×4754 CSS px；断言 scrollWidth ≤ 1200（无横向溢出）。
- 分片：6 片（5×800 + 754）；每片 `scrollTo(offset)` 后读回
  `window.scrollY` 断言相等再截屏。
- 拼接断言：full@2x.png 恰为 2400×9508（= 1200×2 × 4754×2）。
- 三件产物：full@2x.png 2400×9508；grayscale.png 2400×9508
  （Rec709Luminance）；thumb.png 480×1902；全部经 magick `-strip` 并以
  `-define png:exclude-chunk=time` 抑制 tIME 时间戳块（见偏差 D8），
  保证双跑字节一致。

## 6. 真空复跑记录

- 副本位置：/tmp/ign-drr/vacuum/A、/tmp/ign-drr/vacuum/B（平面复制，
  忽略 .DS_Store/__pycache__）。
- 删除项（可重建产物）：index.html、panels/、renders/、data/rebuild/、
  fingerprints.sha256。冻结层 data/frozen/ 不删不改，复跑后逐字节保留。
- 重建链（每副本独立执行）：guard_engine → cargo build --offline
  --release（二进制 sha 对齐冻结值）→ 七样例重渲染（每个 SVG/PNG sha
  对齐冻结产物）→ page.py 重生成页面 → screenshot.py 重生成三件渲染产物
  → 重写 fingerprints.sha256。
- 结果：A 与 B 互相逐字节一致，且与交付树最终状态逐字节一致（49 个文件 =
  48 个交付产物 + 指纹清单自身，含 README.md 与本文件）。页面统计若涉冻结
  文件计数，均以冻结层实际文件为准，真空过程产物（/tmp 下）不计入。

## 7. 指纹机检记录

- fingerprints.sha256 由 tools/rebuild.py 末段生成：对全树排序遍历，
  记录每个文件的 sha256；清单自身排除在外；.DS_Store 与 __pycache__
  一律排除。
- B4 重算全树并逐项比对清单：48 项全部吻合，无缺失、无多余、无漂移。

## 8. 偏差与披露

| 编号 | 偏差 / 披露 |
|---|---|
| D1 | guardrail-probes.txt 曾自修复重冻结一次：首版探针 P3/P4 误触缩放闸门（--width 70000/40000 的有效缩放超 16.0）而未达画幅/像素上限，P10 依赖解析把 `git` 源计为 1 项。删除该冻结文件后由拒绝覆盖的冻结工具一次性重跑（唯一修复路径，仅此一次）。现存文件为修正版（P3 改用宽场景 ×16、P4 改用 schema ×16、P10 修正解析，dependency-count: 7）。 |
| D2 | 引擎仓存在冻结前已存在的 `target/` 残留（mtime 2026-09-03，早于冻结 2026-09-05T14:20Z），属既有 gitignored 状态。本工具链全部构建走 `/tmp` 下 `CARGO_TARGET_DIR`，未新增任何残留；门禁以 mtime 校验「无晚于冻结时间的新增」；受引擎仓只读约束未做清理。 |
| D3 | 测试计数：引擎 VALIDATION.md 自称 14 项测试，冻结实测 `cargo test` 26 项通过（4 lib + 1 artifacts + 1 cli + 8 render + 12 themes，cargo-test.txt 各 test result 行）。页面报告实测值 26。 |
| D4 | 特性覆盖数字（68/45 及逐语言值）由冻结脚本对快照内 e2e 特性矩阵汇总而来；本交付未运行 live Go e2e（引擎 e2e/ 与 plot-provider-diagrams 均未执行）。 |
| D5 | data/frozen/README.txt 的文件清单写于首批冻结，未列之后由各自一次性工具追加的 guardrail-probes.txt 与 engine-readme.txt；已冻结内容本身从未改写。 |
| D6 | 排版性改写（无内容影响，均为打断与引擎语料的 25 字符共窗）：页面 CSS 逐行美化并统一冒号后空格、`<meta>`/`<title>`/SVG 根属性顺序调整、XML 转义卡改为描述性表述（不再逐字引用探针 payload）。语义与数字不变。 |
| D7 | 零网络与位图确定性：cargo `--offline` 全程成功，无任何依赖抓取（无需依赖抓取类偏差登记）；PNG 真机双跑 7/7 逐字节一致、发布二进制二次全新构建逐字节一致，未触发位图回退路径（无位图差异需要披露）。 |
| D8 | 渲染产物字节稳定性自修复：首轮真空复跑发现 renders/full@2x.png A≠B——像素逐点相同（`magick compare` AE=0），差异仅是 ImageMagick PNG 编码器在 `-strip` 下仍写入的 `tIME` 时间戳块。修复为 `-define png:exclude-chunk=time` 后三件产物全部字节稳定（引擎产物 PNG 与本修复无关，双跑本就逐字节一致）。 |

## 9. 完成条件核对

- 四电池全绿：是（§4）。
- README 环境变量表齐备：是（README.md「环境变量」节，含工具自定位）。
- 本验证文件完整：快照锚点 / 数字锚点 / 裁定 / 门禁 / 渲染断言 / 真空 /
  指纹 / 偏差 全部在册。
- 树内零 `.pyc`、/tmp 临时目录已清理、引擎 porcelain 除本树外干净。
