# Club OA（主仓库）

社团一体化 OA 系统的主仓库：存放需求/架构/设计文档、部署编排与子模块引用。各组件以 **git submodule** 挂载，独立开发、独立发布。

## 仓库结构

```
club-oa/                  # 主仓库：docs + deploy + scripts + 子模块
├── libs/                 # 共享 Rust 库（club-oa-libs）
├── services/             # 后端服务，每个服务一个仓库
│   ├── auth/             #   club-oa-auth
│   ├── im/               #   club-oa-im
│   ├── task/ doc/ meeting/ event/ drive/ notify/
├── apps/
│   ├── web/              # Web 门户（club-oa-web，Nuxt 3）
│   └── app/              # 多端客户端（club-oa-app，Tauri v2）
├── packages/             # 前端共享库（club-oa-fe-libs：ui/core）【待创建】
├── deploy/               # docker-compose / nginx / 初始化脚本
├── scripts/              # 开发与运维脚本
└── docs/                 # requirements.md / architecture.md / design-system.md
```

技术栈：Rust（axum + **SeaORM**）+ Vue 3 / Nuxt 3 / Tailwind CSS + Tauri v2；PostgreSQL + Redis + ntfy + nginx + Stalwart 邮件。

## 克隆与初始化

```bash
git clone <主仓库地址> club-oa && cd club-oa
# 本机开发时子模块来源为本地路径，需要允许 file 协议
git -c protocol.file.allow=always submodule update --init --recursive
```

> 远程托管就绪后，执行 `scripts/set-submodule-remotes.sh git@github.com:<org>` 将 `.gitmodules` 切换为远程地址。

## 开发

```bash
# 共享库
cargo test --manifest-path libs/Cargo.toml
cargo llvm-cov --manifest-path libs/Cargo.toml --workspace --fail-under-lines 80

# 后端服务（示例：auth）
cargo test --manifest-path services/auth/Cargo.toml
cargo run  --manifest-path services/auth/Cargo.toml

# 前端
pnpm -C apps/web install && pnpm -C apps/web dev
```

## 质量门禁

TDD 开发，行/语句/函数覆盖率 ≥ 80%（分支 ≥ 70%），CI 不达标禁止合并。详见 `docs/requirements.md` 19.3。

## 文档

- 需求基线：`docs/requirements.md`
- 架构设计：`docs/architecture.md`
- 设计系统：`docs/design-system.md`
