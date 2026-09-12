#!/usr/bin/env bash
# 将 .gitmodules 中的本地路径来源替换为远程仓库地址。
# 用法: scripts/set-submodule-remotes.sh git@github.com:your-org
set -euo pipefail

BASE="${1:?用法: $0 <base-url，例如 git@github.com:your-org>}"

declare -a PATHS=(
  "libs"
  "services/auth" "services/im" "services/task" "services/doc"
  "services/meeting" "services/event" "services/drive" "services/notify"
  "apps/web" "apps/app"
  "packages"
)

declare -A NAMES=(
  ["libs"]="club-oa-libs"
  ["services/auth"]="club-oa-auth"
  ["services/im"]="club-oa-im"
  ["services/task"]="club-oa-task"
  ["services/doc"]="club-oa-doc"
  ["services/meeting"]="club-oa-meeting"
  ["services/event"]="club-oa-event"
  ["services/drive"]="club-oa-drive"
  ["services/notify"]="club-oa-notify"
  ["apps/web"]="club-oa-web"
  ["apps/app"]="club-oa-app"
  ["packages"]="club-oa-fe-libs"
)

for path in "${PATHS[@]}"; do
  name="${NAMES[$path]}"
  if git config -f .gitmodules "submodule.$path.url" >/dev/null 2>&1; then
    git config -f .gitmodules "submodule.$path.url" "$BASE/$name.git"
  fi
done

git submodule sync
echo "已更新 .gitmodules 为远程地址。下一步："
echo "  git add .gitmodules && git commit -m 'chore: 切换子模块到远程仓库'"
echo "  git submodule update --init --recursive"
