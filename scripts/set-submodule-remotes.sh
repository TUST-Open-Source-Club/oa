#!/usr/bin/env bash
# 将 .gitmodules 中的本地路径来源统一替换为远程仓库地址。
# 用法: scripts/set-submodule-remotes.sh https://github.com/your-org
# 注意：macOS 自带 bash 3.2 不支持关联数组，因此使用 case 映射。
set -euo pipefail

BASE="${1:?用法: $0 <base-url，例如 https://github.com/your-org>}"

# 子模块路径 → 仓库名
repo_name() {
  case "$1" in
    libs) echo "oa-libs" ;;
    services/auth) echo "oa-auth" ;;
    services/im) echo "oa-im" ;;
    services/task) echo "oa-task" ;;
    services/doc) echo "oa-doc" ;;
    services/meeting) echo "oa-meeting" ;;
    services/event) echo "oa-event" ;;
    services/drive) echo "oa-drive" ;;
    services/notify) echo "oa-notify" ;;
    apps/web) echo "oa-web" ;;
    apps/app) echo "oa-app" ;;
    packages) echo "oa-fe-libs" ;;
    *) return 1 ;;
  esac
}

for path in libs \
  services/auth services/im services/task services/doc \
  services/meeting services/event services/drive services/notify \
  apps/web apps/app packages; do
  if git config -f .gitmodules "submodule.$path.url" >/dev/null 2>&1; then
    git config -f .gitmodules "submodule.$path.url" "$BASE/$(repo_name "$path").git"
  fi
done

git submodule sync
echo "已更新 .gitmodules 为远程地址。下一步："
echo "  git add .gitmodules && git commit -m 'chore: 切换子模块到远程仓库'"
echo "  git submodule update --init --recursive"
