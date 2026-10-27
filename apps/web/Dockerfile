# 构建阶段：Nuxt SSR 产物；构建上下文 = 仓库根目录（monorepo 布局）
#   docker build -f apps/web/Dockerfile -t club-oa/web:local .
FROM node:22-alpine AS builder
RUN corepack enable
WORKDIR /app
# 只复制工作区清单与 web 依赖闭包（packages/*），最大化缓存命中
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY packages ./packages
COPY apps/web ./apps/web
COPY apps/app/electron/package.json ./apps/app/electron/package.json
RUN pnpm install --frozen-lockfile --filter club-oa-web...
RUN pnpm --filter club-oa-web build

# 运行阶段
FROM node:22-alpine
WORKDIR /app
ENV NODE_ENV=production
COPY --from=builder /app/apps/web/.output ./.output
EXPOSE 3000
CMD ["node", ".output/server/index.mjs"]
