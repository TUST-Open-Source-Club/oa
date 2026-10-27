#!/usr/bin/env bash
# 本地质量门禁（与 CI 对齐）
#
# 用法：
#   scripts/check.sh          完整：fmt + clippy + test + 覆盖率 + 前端 typecheck/test
#   scripts/check.sh --fast   跳过覆盖率
#
# 前提：Rust 1.98+、Node 22+、pnpm 11。
# Rust 集成测试依赖：
#   scripts/test-env.sh start          # PostgreSQL（55432）+ Redis（56379）
#   libs/scripts/s3-test-env.sh start  # MinIO（59000），跑 S3 后端集成测试
set -euo pipefail

FAST=0
[ "${1:-}" = "--fast" ] && FAST=1

cd "$(dirname "$0")/.."

export S3_TEST_ENDPOINT="${S3_TEST_ENDPOINT:-http://127.0.0.1:59000}"
export S3_TEST_REGION="${S3_TEST_REGION:-us-east-1}"
export S3_TEST_BUCKET="${S3_TEST_BUCKET:-club-oa-test}"
export S3_TEST_ACCESS_KEY="${S3_TEST_ACCESS_KEY:-minioadmin}"
export S3_TEST_SECRET_KEY="${S3_TEST_SECRET_KEY:-minioadmin}"

echo "==> cargo fmt"
cargo fmt --all -- --check

echo "==> cargo clippy（-D warnings）"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test（整仓，含 S3 后端集成测试）"
cargo test --workspace --features club-storage/s3

if [ "$FAST" -eq 0 ]; then
  echo "==> 覆盖率（行 + 函数 ≥ 80%）"
  # migrations 的 down() 不会被正常启动流程调用；按 requirements 19.3 / 15.3 显式排除
  cargo llvm-cov --workspace --features club-storage/s3 \
    --fail-under-lines 80 --fail-under-functions 80 \
    --ignore-filename-regex '(src/main\.rs|/migration)' \
    --lcov --output-path lcov.info
fi

echo "==> pnpm 依赖检查"
pnpm install --frozen-lockfile

echo "==> 前端 typecheck（含 apps/web）"
pnpm -r typecheck

echo "==> 前端 test（含覆盖率门禁）"
pnpm -r test

echo "==> 前端构建"
pnpm --filter club-oa-web build

echo "全部检查通过"
