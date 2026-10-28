#!/usr/bin/env bash
# 启动/停止本地 S3 兼容服务（adobe/s3mock）供 S3 后端集成测试使用。
#
# 说明：MinIO 官方已停止分发社区版镜像与二进制（Docker Hub / Quay / dl.min.io
# 均不可用），改用同为 S3 兼容、覆盖预签名 GET/PUT 的 s3mock。
#
# 用法: ./scripts/s3-test-env.sh [start|stop]
set -euo pipefail
NAME=club-oa-s3mock-test
PORT="${S3_TEST_PORT:-59000}"
BUCKET="${S3_TEST_BUCKET:-club-oa-test}"
case "${1:-start}" in
  start)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    docker run -d --name "$NAME" -p "${PORT}:9090" \
      -e COM_ADOBE_TESTING_S3MOCK_STORE_INITIAL_BUCKETS="$BUCKET" \
      adobe/s3mock:latest >/dev/null
    for _ in $(seq 1 60); do
      curl -sf "http://127.0.0.1:${PORT}/" >/dev/null && break
      sleep 1
    done
    echo "s3mock 就绪: http://127.0.0.1:${PORT} (bucket: $BUCKET)"
    echo "S3_TEST_ENDPOINT=http://127.0.0.1:${PORT}"
    ;;
  stop)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    echo "s3mock 已停止"
    ;;
  *)
    echo "用法: $0 [start|stop]"; exit 1;;
esac
