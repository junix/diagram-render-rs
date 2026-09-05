# data/frozen — one-time evidence (NEVER regenerated)

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
