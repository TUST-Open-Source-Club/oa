# club-oa-web

社团 OA Web 门户（Nuxt 3 + Vue 3 + Tailwind CSS），单仓库组件：`apps/web/`。

- 依赖共享库：`packages/ui`、`packages/core`（pnpm workspace 协议 `workspace:*`）
- 开发：仓库根目录 `pnpm install` 后执行 `pnpm --filter club-oa-web dev`
- 构建：`pnpm --filter club-oa-web build`
- 镜像：仓库根目录执行 `docker build -f apps/web/Dockerfile -t club-oa/web:local .`

## 许可证

本项目采用 **AGPL-3.0-or-later** 许可证，详见 [LICENSE](../../LICENSE)。
