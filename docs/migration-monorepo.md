# 从多仓库（git submodule）迁移到 monorepo

| 项目 | 内容 |
| --- | --- |
| 迁移日期 | 2026-10-27 |
| 迁移前形态 | 主仓库 `oa` + 12 个组件仓库，通过 `.gitmodules` 挂载（1 层子模块，组件内再嵌套 `libs` / `packages` 子模块） |
| 迁移后形态 | 单仓库 monorepo：Rust 单 Cargo workspace（libs + services）、前端单 pnpm workspace（apps + packages） |
| 历史策略 | **快照导入**：每个组件一次导入提交，commit message 记录原仓库地址与 SHA；组件历史保留在各自原仓库 |
| 迁移基线 | 主仓库 `10bedad`（`chore: 同步子模块…`） |

## 1. 组件来源与快照 SHA

按此表可在原仓库中查看任一文件的完整历史（`git log <SHA> -- path`）。

| 组件 | 原仓库 | 导入路径 | 快照 SHA |
| --- | --- | --- | --- |
| 共享 Rust 库 | `TUST-Open-Source-Club/oa-libs` | `libs/` | `24a69d86d6cd59669adb6c770e6f26dc7ffe3147` |
| 认证服务 | `TUST-Open-Source-Club/oa-auth` | `services/auth/` | `ed7d19f804025d5dfb18c1aaab83e24ef029f1ae` |
| 即时通讯 | `TUST-Open-Source-Club/oa-im` | `services/im/` | `8d3fdab8d6f8a242963d248929b9d4de62c09fde` |
| 任务看板 | `TUST-Open-Source-Club/oa-task` | `services/task/` | `aef28c92695605a98d931edb78cc4a2b67d5b5b1` |
| 文档 | `TUST-Open-Source-Club/oa-doc` | `services/doc/` | `bf5c7d31b0c7a90798749d97be2e6da243ce5971` |
| 会议（占位） | `TUST-Open-Source-Club/oa-meeting` | `services/meeting/` | `d687d9b78f547fc6d1680afcdd6bd5d5b56a4dca` |
| 活动报名 | `TUST-Open-Source-Club/oa-event` | `services/event/` | `27bcbc1e95490785005780685c75b715beea5e22` |
| 网盘 | `TUST-Open-Source-Club/oa-drive` | `services/drive/` | `520a1a6399efe693669dcbd00c3e99c1688ed31d` |
| 通知 | `TUST-Open-Source-Club/oa-notify` | `services/notify/` | `9c74dd94ce97cdfad541c21439de5deb14855957` |
| Web 门户 | `TUST-Open-Source-Club/oa-web` | `apps/web/` | `52688143333e1f66165eb633e56086232bdf34aa` |
| 客户端壳 | `TUST-Open-Source-Club/oa-app` | `apps/app/` | `ebdaac6f5ffa6386201ba52b9ace69dc6be364e3` |
| 前端共享库 | `TUST-Open-Source-Club/oa-fe-libs` | `packages/` | `cd6194ce0867967375dc4dfa554c42a1b73c5655` |

## 2. 结构变化

### 2.1 Rust：多工程 → 单 workspace

| 迁移前 | 迁移后 |
| --- | --- |
| 每个服务是独立的 Cargo 工程（各自的 `Cargo.toml` + `Cargo.lock`），通过 `path = "libs/crates/*"` 依赖各自嵌套的 `libs` 子模块 | 根 `Cargo.toml` 定义唯一 workspace：成员 = `libs/crates/*` + `services/*`；服务以 `{ workspace = true }` 继承共享库与第三方依赖 |
| 12 份 `Cargo.lock`，同名依赖版本可能漂移 | 根目录一份 `Cargo.lock`，依赖版本/特性在 `[workspace.dependencies]` 单点声明 |
| `libs/Cargo.toml` 自身又是一个 workspace | 并入根 workspace，crate 的 `version/edition/license` 从 `[workspace.package]` 继承 |
| 构建单个服务：`cargo build --manifest-path services/auth/Cargo.toml` | `cargo build -p auth-service`；整仓：`cargo check/test --workspace` |

顺带收敛：`sea-orm-migration` 统一关闭默认 features（服务内嵌迁移不需要 CLI，去掉 `clap`/`dotenvy` 依赖）；`[workspace.lints]` 统一 `unsafe_code = "forbid"`。

### 2.2 前端：三个独立安装 → 单 pnpm workspace

| 迁移前 | 迁移后 |
| --- | --- |
| `apps/web` 用 `link:packages/core` 指向自己的 `packages` 子模块；`packages`、`apps/app/electron` 各自独立 `pnpm install` | 根 `pnpm-workspace.yaml` 管理 `apps/web`、`apps/app/electron`、`packages/*`；Web 依赖改为 `workspace:*` |
| 3 份 `pnpm-lock.yaml` | 根目录一份 `pnpm-lock.yaml` |
| 组件内 `pnpm-workspace.yaml` 各自维护 `onlyBuiltDependencies` | 统一到根 `pnpm-workspace.yaml` 的 `allowBuilds`（pnpm 11 语义） |

### 2.3 构建与部署

- 8 个 `Dockerfile`（7 个 Rust 服务 + Web）全部改为**以仓库根目录为构建上下文**（Rust 镜像需要根 `Cargo.lock` + `libs/`，Web 镜像需要 `packages/`），构建命令用 `-p <crate>` 只编译目标服务。
- `deploy/docker-compose.yml`、`deploy/test/docker-compose.yml` 的 `build.context` 改为 `..` / `../..` 并显式指定 `dockerfile`。
- 新增根 `.dockerignore`（排除 `node_modules`、`target`、`.git`、`.repos` 等）。
- CI 合并为单一工作流 `.github/workflows/ci.yml`：`rust` 作业（fmt / clippy / test / 覆盖率）+ `web` 作业（typecheck / test / build），不再需要 `submodules: recursive` 检出。

### 2.4 仓库级元数据收敛

删除各组件仓库自带的 `.gitmodules`、`.github/workflows/ci.yml`、`.gitignore`、`.dockerignore`、`LICENSE`（内容与根 `LICENSE` 完全一致，已校验 SHA256），以及嵌套的 `libs/`、`packages/` 子模块目录；忽略规则、许可证、CI 统一在根目录维护。

## 3. 迁移中修复的既有问题

合并为单 workspace 后，整仓编译/测试首次把各仓库之间的**版本漂移**暴露出来（各组件仓库此前 CI 已是红色，与本次迁移无关）：

| 问题 | 位置 | 处理 |
| --- | --- | --- |
| `Claims` 新增 `account_type` / `bot_permissions` 后，测试里的构造未同步（E0063） | `services/{im,task,doc,event,drive,notify}/tests/`（7 处） | 补 `account_type: None` / `bot_permissions: None` |
| `missing_docs`（`-D warnings` 下失败） | `services/auth/src/repo.rs`（`search_users`）、`libs/crates/auth-sdk/src/lib.rs`（`jwks`、`guard` 模块注释错位） | 补文档注释并纠正注释归属 |
| `clippy::duplicated_attributes` | `services/event/src/repo.rs` | 删除重复的 `#[allow]` |
| `clippy::too_many_arguments` | `services/event/src/repo.rs::create_registration` | 显式 `#[allow]` + 说明理由 |
| `clippy::needless_question_mark` | `services/task/src/routes.rs::task_snapshot` | 去掉多余的 `Ok(…?)` |
| `clippy::unnecessary_to_owned` | `services/auth/tests/account_flow.rs` | 去掉多余的 `&` 与 `.to_string()` |
| 迁移脚本里硬编码 `CREATE SCHEMA IF NOT EXISTS <服务名>`，并行跑测试时 5 个用例同时创建同名 schema 触发 `23505 duplicate key`（`pg_namespace_nspname_index`） | 7 个服务的第一份迁移 | 移除该语句：schema 早已由 `connect_with_schema`（bootstrap 连接）创建并设置 `search_path`，迁移只负责建表 |
| `apps/web` 的 `nuxt typecheck` 有 30 处严格模式类型错误（`noUncheckedIndexedAccess` 下的数组下标、事件/表单负载类型、Nitro `$fetch` 泛型）。原 `oa-web` CI 只跑 `test` + `build`，从未跑过 `typecheck` | `components/{FormBuilder,UserPicker,MarkdownEditor.client}.vue`、`pages/{docs,drive,events,im,tasks}.vue`、`server/utils/auth-service.ts`、`stores/notifications.spec.ts` | 加判空/类型谓词、抽出 `utils/form-schema.ts`（`toFormSchema` 收敛未知 JSON）、`$fetch` 边界处收敛类型；`pnpm -r typecheck` 已纳入 CI |
| 报名表单 schema 在页面里用 `as` 强转后传给设计器，类型不安全 | `pages/events.vue` | 改为 `toFormSchema()` 运行时规整 + `FormSchema` 类型，新增 5 个单测 |
| `problemOf` 只认带 `detail`/`code` 的载荷，仅有 `title` 的 ProblemDetails 会被丢弃 | `apps/web/utils/api-error.ts` | 增加 `title` 判定，补 6 个单测 |

## 4. 迁移后整仓验证（本地，2026-10-27）

| 检查项 | 命令 | 结果 |
| --- | --- | --- |
| 格式 | `cargo fmt --all -- --check` | 通过（首次整仓 `cargo fmt` 顺带修复了部分组件此前未格式化的代码） |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| Rust 测试 | `cargo test --workspace --features club-storage/s3`（真实 PG 55432 + Redis 56379 + s3mock 59000） | 全部通过：约 210 个用例，含 7 个服务集成测试、S3 后端往返/预签名测试 |
| 覆盖率 | `cargo llvm-cov --workspace --features club-storage/s3 --fail-under-lines 80 --fail-under-functions 80 --ignore-filename-regex '(src/main\.rs\|/migration)'` | 行 **85.11%**、函数 **81.91%**、区域 81.92%（均 ≥ 80） |
| 前端安装 | `pnpm install --frozen-lockfile` | 通过（单 `pnpm-lock.yaml`） |
| 前端类型检查 | `pnpm -r typecheck` | 通过：`packages/core`、`packages/ui`、`apps/web`（原 30 处错误全部消除） |
| 前端测试 | `pnpm -r test` | 通过：`packages/core` 7、`packages/ui` 11、`apps/web` 31 个用例；web 语句 98.1%、行 100%、函数 100% |
| 前端构建 | `pnpm --filter club-oa-web build` | 通过（Nuxt SSR 产物 2.64 MB） |
| 镜像构建 | `docker build -f services/auth/Dockerfile .`、`docker build -f apps/web/Dockerfile .` | 均通过（根上下文） |
| 编排校验 | `docker compose config`（生产 + 测试两套） | 通过 |

> 覆盖率口径：排除 `src/main.rs`（启动装配）与 `src/migration*`（迁移的 `down()` 不会在启动流程中执行），
> 与需求 19.3 / 架构 15.3 的排除项一致；S3 后端集成测试在 CI 中由 s3mock 服务提供，因此 `storage/s3.rs` 计入覆盖率。

## 5. 后续操作建议

1. **归档旧组件仓库**（GitHub → Settings → Archive），保留 tag 与历史，避免有人继续向旧仓库提交。
2. **更新本地工作副本**：旧克隆里 `services/*` 是空目录，请重新克隆本仓库，或 `git submodule deinit -f --all && rm -rf .git/modules && git pull`。
3. **清理分支保护/CI 配置**：旧仓库的 required checks 不再有意义；本仓库的 `CI / Rust …`、`CI / 前端 …` 作为新的合并门禁。
4. **发布流程**：镜像构建改为在根目录 `docker build -f services/<svc>/Dockerfile .`；版本标签统一在主仓库打（`v0.x.y`）。
