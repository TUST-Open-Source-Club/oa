#!/usr/bin/env bash
# 本地质量门禁（与 CI 对齐）
#
# 用法：
#   scripts/check.sh          完整：fmt + clippy + test + 覆盖率 + 前端 typecheck/test
#   scripts/check.sh --fast   跳过覆盖率与前端 build
#
# 前提：Rust 1.98+、Node 22+、pnpm 11；Rust 集成测试需要 PG/Redis（scripts/test-env.sh start）。
set -euo pipefail

FAST=0
[ "${1:-}" = "--fast" ] && FAST=1

cd "$(dirname "$0")/.."

echo "==> cargo fmt"
cargo fmt --all -- --check

echo "==> cargo clippy（-D warnings）"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test（整仓）"
cargo test --workspace

if [ "$FAST" -eq 0 ]; then
  echo "==> 覆盖率（行 ≥ 80%）"
  cargo llvm-cov --workspace --fail-under-lines 80 \
    --ignore-filename-regex 'src/main.rs' --lcov --output-path lcov.info
fi

echo "==> pnpm 依赖检查"
pnpm install --frozen-lockfile

echo "==> 前端 typecheck（packages/*）"
pnpm --filter './packages/*' typecheck

echo "==> 前端 test（含覆盖率门禁）"
pnpm -r test

echo "全部检查通过"
