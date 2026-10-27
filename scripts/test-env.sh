#!/usr/bin/env bash
# 本地测试依赖：PostgreSQL（55432）+ Redis（56379）
#
# 用法：
#   scripts/test-env.sh start   启动并等待就绪
#   scripts/test-env.sh stop    停止并删除容器
#   scripts/test-env.sh status  查看状态
#
# 端口与凭据与测试默认值一致，可用环境变量覆盖：
#   PG_PORT（默认 55432）、REDIS_PORT（默认 56379）
set -euo pipefail

PG_NAME=club-oa-pg-test
REDIS_NAME=club-oa-redis-test
PG_PORT="${PG_PORT:-55432}"
REDIS_PORT="${REDIS_PORT:-56379}"

wait_for() {
  local what="$1" check="$2"
  for _ in $(seq 1 60); do
    if eval "$check" >/dev/null 2>&1; then
      echo "  ${what} 就绪"
      return 0
    fi
    sleep 1
  done
  echo "错误：${what} 未在 60s 内就绪" >&2
  return 1
}

start() {
  docker rm -f "$PG_NAME" "$REDIS_NAME" >/dev/null 2>&1 || true
  echo "启动 PostgreSQL（127.0.0.1:${PG_PORT}）与 Redis（127.0.0.1:${REDIS_PORT}）…"
  docker run -d --name "$PG_NAME" \
    -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=club_oa \
    -p "${PG_PORT}:5432" postgres:16-alpine >/dev/null
  docker run -d --name "$REDIS_NAME" -p "${REDIS_PORT}:6379" redis:7-alpine >/dev/null
  wait_for "PostgreSQL" "docker exec $PG_NAME pg_isready -U postgres"
  wait_for "Redis" "docker exec $REDIS_NAME redis-cli ping"
  cat <<EOF
测试依赖已就绪：
  TEST_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:${PG_PORT}/club_oa
  REDIS_TEST_URL=redis://127.0.0.1:${REDIS_PORT}
EOF
}

stop() {
  docker rm -f "$PG_NAME" "$REDIS_NAME" >/dev/null 2>&1 || true
  echo "测试依赖已停止"
}

case "${1:-start}" in
  start) start ;;
  stop) stop ;;
  status) docker ps --filter "name=${PG_NAME}" --filter "name=${REDIS_NAME}" --format '{{.Names}}\t{{.Status}}' ;;
  *) echo "用法：$0 [start|stop|status]" >&2; exit 1 ;;
esac
