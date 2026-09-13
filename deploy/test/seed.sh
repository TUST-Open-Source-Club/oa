#!/bin/sh
# 社团 OA 测试环境种子数据：账号 + IM + 任务 + 文档 + 活动 + 网盘 + 通知
# 依赖：curl、jq（由 compose 在容器内自动安装）
set -eu

GW="${GATEWAY_URL:-http://gateway}"
ADMIN_EMAIL="${ADMIN_EMAIL:-admin@club.test}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:-Admin12345!}"
USER_PASSWORD="${USER_PASSWORD:-Member12345!}"

log() { printf '[seed] %s\n' "$*"; }
die() { printf '[seed] 失败：%s\n' "$*" >&2; exit 1; }

# api METHOD PATH [TOKEN] [JSON_BODY]
api() {
  method="$1"; path="$2"; token="${3:-}"; body="${4:-}"
  if [ -n "$body" ] && [ -n "$token" ]; then
    curl -sS -X "$method" "$GW$path" -H 'content-type: application/json' -H "authorization: Bearer $token" -d "$body"
  elif [ -n "$body" ]; then
    curl -sS -X "$method" "$GW$path" -H 'content-type: application/json' -d "$body"
  elif [ -n "$token" ]; then
    curl -sS -X "$method" "$GW$path" -H "authorization: Bearer $token"
  else
    curl -sS -X "$method" "$GW$path"
  fi
}

# id_of JSON [jq 表达式]：提取 id，缺失则报错
id_of() {
  value=$(printf '%s' "$1" | jq -r "${2:-.id} // .id // empty")
  [ -n "$value" ] || die "响应缺少 id：$(printf '%s' "$1" | head -c 300)"
  printf '%s' "$value"
}

# ------------------------- 管理员登录 -------------------------
log "管理员登录 $ADMIN_EMAIL"
ADMIN_TOKEN=$(api POST /api/v1/auth/login "" "{\"identifier\":\"$ADMIN_EMAIL\",\"password\":\"$ADMIN_PASSWORD\"}" | jq -r '.accessToken // empty')
[ -n "$ADMIN_TOKEN" ] || die "管理员登录失败（检查 BOOTSTRAP_ADMIN_* 或是否首次启动）"
ADMIN_ID=$(api GET /api/v1/auth/me "$ADMIN_TOKEN" | jq -r '.id')
log "管理员 ID：$ADMIN_ID"

# ------------------------- 演示账号 -------------------------
# ensure_user EMAIL USERNAME NICKNAME，激活后返回 user id
ensure_user() {
  email="$1"; username="$2"; nickname="$3"
  resp=$(api POST /api/v1/auth/admin/users "$ADMIN_TOKEN" \
    "{\"email\":\"$email\",\"username\":\"$username\",\"nickname\":\"$nickname\",\"department\":\"测试部\"}")
  token=$(printf '%s' "$resp" | jq -r '.devActivationToken // empty')
  if [ -n "$token" ]; then
    api POST /api/v1/auth/users/activate "" "{\"token\":\"$token\",\"password\":\"$USER_PASSWORD\"}" >/dev/null
    log "已创建并激活 $username"
  else
    log "$username 已存在，跳过创建"
  fi
  api POST /api/v1/auth/login "" "{\"identifier\":\"$username\",\"password\":\"$USER_PASSWORD\"}" \
    | jq -r '.user.id // empty'
}

log "准备演示账号"
MEMBER1_ID=$(ensure_user member1@club.test member1 "张小萌")
MEMBER2_ID=$(ensure_user member2@club.test member2 "李思远")
[ -n "$MEMBER1_ID" ] || die "member1 登录失败"
[ -n "$MEMBER2_ID" ] || die "member2 登录失败"
MEMBER_TOKEN=$(api POST /api/v1/auth/login "" '{"identifier":"member1","password":"Member12345!"}' | jq -r '.accessToken')

# ------------------------- IM：群聊与消息 -------------------------
log "创建 IM 群聊与消息"
conv=$(api POST /api/v1/im/conversations "$ADMIN_TOKEN" \
  "{\"type\":\"group\",\"memberIds\":[\"$MEMBER1_ID\",\"$MEMBER2_ID\"],\"name\":\"理事会\"}")
CONV_ID=$(id_of "$conv" '.id // .conversation.id')
api POST "/api/v1/im/conversations/$CONV_ID/messages" "$ADMIN_TOKEN" \
  '{"type":"text","content":{"text":"欢迎加入理事会，这里是测试群。"}}' >/dev/null
api POST "/api/v1/im/conversations/$CONV_ID/messages" "$MEMBER_TOKEN" \
  '{"type":"text","content":{"text":"收到，已熟悉系统。"}}' >/dev/null

# ------------------------- 任务：项目/看板/任务 -------------------------
log "创建任务项目与看板"
proj=$(api POST /api/v1/task/projects "$ADMIN_TOKEN" '{"name":"迎新活动筹备","description":"2026 秋季迎新（测试数据）"}')
PROJ_ID=$(id_of "$proj")
COL_TODO=$(id_of "$(api POST "/api/v1/task/projects/$PROJ_ID/columns" "$ADMIN_TOKEN" '{"name":"待办"}')")
COL_DOING=$(id_of "$(api POST "/api/v1/task/projects/$PROJ_ID/columns" "$ADMIN_TOKEN" '{"name":"进行中"}')")
COL_DONE=$(id_of "$(api POST "/api/v1/task/projects/$PROJ_ID/columns" "$ADMIN_TOKEN" '{"name":"已完成"}')")
api POST "/api/v1/task/projects/$PROJ_ID/tasks" "$ADMIN_TOKEN" \
  "{\"columnId\":\"$COL_TODO\",\"title\":\"联系活动场地\",\"descriptionMd\":\"确认 9 月中旬可用\",\"assigneeId\":\"$MEMBER1_ID\",\"priority\":\"high\"}" >/dev/null
api POST "/api/v1/task/projects/$PROJ_ID/tasks" "$ADMIN_TOKEN" \
  "{\"columnId\":\"$COL_DOING\",\"title\":\"设计迎新海报\",\"assigneeId\":\"$MEMBER2_ID\",\"priority\":\"normal\"}" >/dev/null
api POST "/api/v1/task/projects/$PROJ_ID/tasks" "$ADMIN_TOKEN" \
  "{\"columnId\":\"$COL_DONE\",\"title\":\"确定活动时间\",\"priority\":\"low\"}" >/dev/null

# ------------------------- 文档：空间/目录/页面 -------------------------
log "创建文档空间与页面"
space=$(api POST /api/v1/doc/spaces "$ADMIN_TOKEN" '{"name":"社团知识库","type":"team"}')
SPACE_ID=$(id_of "$space")
folder=$(api POST "/api/v1/doc/spaces/$SPACE_ID/nodes" "$ADMIN_TOKEN" '{"kind":"folder","title":"规章制度"}')
FOLDER_ID=$(id_of "$folder")
page=$(api POST "/api/v1/doc/spaces/$SPACE_ID/nodes" "$ADMIN_TOKEN" \
  "{\"kind\":\"page\",\"title\":\"新成员指南\",\"parentId\":\"$FOLDER_ID\"}")
PAGE_ID=$(id_of "$page")
api PUT "/api/v1/doc/spaces/$SPACE_ID/nodes/$PAGE_ID/content" "$ADMIN_TOKEN" \
  '{"contentMd":"# 新成员指南\n\n欢迎加入社团！\n\n1. 先阅读章程\n2. 加入 IM 群聊\n3. 领取迎新任务","baseVersion":0}' >/dev/null

# ------------------------- 网盘：空间与目录 -------------------------
log "创建网盘空间与目录"
dspace=$(api POST /api/v1/drive/spaces "$ADMIN_TOKEN" '{"name":"公共资料","type":"team"}')
DSPACE_ID=$(id_of "$dspace")
api POST "/api/v1/drive/spaces/$DSPACE_ID/folders" "$ADMIN_TOKEN" '{"name":"活动海报"}' >/dev/null

# ------------------------- 活动：创建活动 -------------------------
log "创建测试活动"
api POST /api/v1/event/events "$ADMIN_TOKEN" \
  '{"slug":"demo-2026","title":"2026 秋季迎新（测试）","descriptionMd":"欢迎新同学，现场有社团介绍与破冰活动。","capacity":50,"waitlistEnabled":true,"needReview":false,"emailVerify":false}' >/dev/null

# ------------------------- 通知：等待 outbox -> notify 消费 -------------------------
log "等待通知生成（outbox → Redis Streams → notify）"
sleep 3
UNREAD=$(api GET /api/v1/notify/notifications/unread-count "$MEMBER_TOKEN" | jq -r '.count // 0')
log "member1 当前未读通知：$UNREAD"

log "完成 ✅"
log "管理员：$ADMIN_EMAIL / $ADMIN_PASSWORD"
log "成员1：member1@club.test / $USER_PASSWORD"
log "成员2：member2@club.test / $USER_PASSWORD"
log "门户：http://localhost:8088"
