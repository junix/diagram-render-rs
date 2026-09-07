# data/audit — 提交后门禁实跑记录（指纹豁免）

本目录是提交后（post-commit）门禁实跑记录的专用位置：
`fingerprints.sha256` 与 B4 指纹机检对本目录下文件一律豁免
（见 `tools/rebuild.py` 第 6 步与 `tools/gates.py` B4 的排除表）。
理由（audit-batteries §7 两条定点规则）：若实跑记录进入指纹，
每次记录都会让指纹失配，迫使再一次提交、再一次实跑——没有定点。

引擎仓在交付提交后 HEAD 必然越过冻结快照 `b38ba07…`。复核配方：

```sh
# 1) 冻结工作树（只读复核冻结证据的正确姿势）
git -C ~/projects/plot/diagram-render-rs worktree add \
    /tmp/ign-drr/frozen-engine b38ba079257a530691b8d2c700586fee5fb810ef

# 2) 完整重建 + 四电池（--tree 指向主检出里的交付树）
PYTHONDONTWRITEBYTECODE=1 python3 tools/rebuild.py \
    --engine /tmp/ign-drr/frozen-engine --tree .
PYTHONDONTWRITEBYTECODE=1 python3 tools/gates.py \
    --engine /tmp/ign-drr/frozen-engine --tree .

# 3) 用后即扫
git -C ~/projects/plot/diagram-render-rs worktree remove \
    /tmp/ign-drr/frozen-engine
```

不建工作树时的降级跑法（对已演进的引擎仓直接跑）：
`gates.py --engine <引擎仓> --skip-vacuum` —— B1 语料钉在冻结
`FROZEN_HEAD`（git ls-tree/show 只读取证），B2/B4 与 prelude 照常；
B3 真空与 rebuild.py 需要冻结快照，必须走上面的工作树配方。

## 实跑记录

| 日期 | 交付 HEAD | 命令 | 结果 |
|---|---|---|---|
| 2026-09-06 | （本轮 refine 尚未提交；待主会话提交后回填） | 待回填 | 待回填 |
