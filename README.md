# Club OA（单仓库 monorepo）

社团一体化 OA 系统：**一个仓库**承载共享库、后端服务、Web 门户、多端客户端壳，以及文档与部署编排。
2026-10 起由「主仓库 + 12 个 git submodule」合并为 monorepo，迁移过程见 [docs/migration-monorepo.md](docs/migration-monorepo.md)。

## 仓库结构

```
club-oa/
├── Cargo.toml                # Rust 工作区：libs/crates/* + services/*
├── Cargo.lock                # 全仓库唯一 Rust 锁文件
├── package.json              # 前端工作区根（统一脚本入口）
├── pnpm-workspace.yaml       # 前端工作区：apps/web、apps/app/electron、packages/*
├── pnpm-lock.yaml            # 全仓库唯一前端锁文件
├── libs/                     # 共享 Rust 库（common / auth-sdk / bus / storage）
├── services/                 # 后端服务：一个目录 = 一个二进制 = 一个容器 = 一个 schema
│   ├── auth/ im/ task/ doc/ meeting/ event/ drive/ notify/
├── apps/
│   ├── web/                  # Web 门户（Nuxt 3 + BFF）
│   └── app/                  # 客户端壳集合（electron / android-shell / harmony-shell）
├── packages/                 # 前端共享库（@club-oa/ui、@club-oa/core）
├── deploy/                   # docker-compose / nginx / postgres 初始化 / 测试编排
├── scripts/                  # 本地测试依赖、质量门禁脚本
└── docs/                     # requirements / architecture / migration
```

架构原则仍是微服务：每个服务独立容器、独立 schema，服务之间只通过 HTTP 与事件总线通信；共享代码只能来自 `libs/`、`packages/`。monorepo 只改变代码组织与版本对齐方式，不改变部署边界。

技术栈：Rust（axum + SeaORM + sea-orm-migration）+ Vue 3 / Nuxt 3 / Tailwind CSS + Electron / Android / HarmonyOS 壳；
PostgreSQL + Redis + ntfy + nginx + Stalwart 邮件。

## 环境要求

| 工具 | 版本 | 说明 |
| --- | --- | --- |
| Rust | 1.98+ | 工作区统一 toolchain，`cargo` 直接用 |
| Node.js | 22+ | 前端与 Electron 壳 |
| pnpm | 11 | 由根 `packageManager` 指定，`corepack enable` 即可 |
| Docker | 任意近期版本 | 跑集成测试与 compose 部署 |

```bash
git clone https://github.com/TUST-Open-Source-Club/oa.git club-oa
cd club-oa
pnpm install          # 前端依赖（含 packages/ui、packages/core）
```

## 开发

### Rust：整仓或单包

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                 # 整仓测试（需要本地 PG/Redis，见下）

cargo run -p auth-service              # 只跑某个服务
cargo test -p auth-service             # 只测某个服务
cargo test -p club-auth-sdk            # 只测某个共享库
```

集成测试默认连接：

- PostgreSQL：`postgres://postgres:postgres@127.0.0.1:55432/club_oa`（`TEST_DATABASE_URL` 可覆盖）
- Redis：`redis://127.0.0.1:56379`（`REDIS_TEST_URL` 可覆盖）

一条命令拉起测试依赖（容器名 `club-oa-pg-test` / `club-oa-redis-test`）：

```bash
scripts/test-env.sh start   # 启动并等待就绪
scripts/test-env.sh stop    # 停止
```

> S3 后端集成测试在未设置 `S3_TEST_ENDPOINT` 时自动跳过；需要时用 `libs/scripts/s3-test-env.sh start` 起本地 MinIO。

### 前端

```bash
pnpm --filter club-oa-web dev       # Web 门户（Nuxt dev server）
pnpm --filter club-oa-web build
pnpm typecheck                      # 共享库类型检查（packages/*）
pnpm -r test                        # 全部前端包单测 + 覆盖率门禁
pnpm --filter club-oa-electron dev  # 桌面端壳（首次需 pnpm --filter club-oa-electron rebuild electron）
```

> `pnpm --filter club-oa-web typecheck`（`nuxt typecheck`）目前有约 30 处存量类型错误（严格模式下的数组下标/事件负载），
> 尚未纳入门禁，待清理后启用；现状与后续动作见 [docs/migration-monorepo.md](docs/migration-monorepo.md) 第 3 节。

### 质量门禁（本地一键）

```bash
scripts/check.sh            # fmt + clippy + test + 覆盖率 + 前端 typecheck/test
scripts/check.sh --fast     # 跳过覆盖率
```

## 部署与联调

```bash
# 一体化测试环境（Postgres/Redis/ntfy + 8 个服务 + 门户 + 网关）
cd deploy/test && docker compose --profile seed up --build

# 生产编排
cd deploy && cp .env.example .env && docker compose up -d --build
```

8 个镜像（7 个 Rust 服务 + Web）均以**仓库根目录**为构建上下文（Rust 服务需要根 `Cargo.lock` 与 `libs/`）：

```bash
docker build -f services/auth/Dockerfile -t club-oa/auth:local .
docker build -f apps/web/Dockerfile     -t club-oa/web:local  .
```

## 质量要求

TDD 开发，行/语句/函数覆盖率 ≥ 80%（分支 ≥ 70%），CI 不达标禁止合并，详见 `docs/requirements.md` 19.3。
CI（`.github/workflows/ci.yml`）执行：`cargo fmt` / `clippy -D warnings` / `cargo test --workspace` /
`cargo llvm-cov`（行 ≥ 80%）/ 前端 `typecheck` + `test` + `build`。

## 文档

- 需求基线：[docs/requirements.md](docs/requirements.md)
- 架构设计：[docs/architecture.md](docs/architecture.md)
- 子模块合并记录：[docs/migration-monorepo.md](docs/migration-monorepo.md)

## 许可证

本项目采用 **AGPL-3.0-or-later** 许可证，详见 [LICENSE](LICENSE)。
