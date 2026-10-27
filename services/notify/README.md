# club-oa-notify

社团 OA 通知服务（Rust + axum，ntfy/APNs/邮件）（单仓库组件：`services/notify/`）。

- 依赖共享库：`libs/crates/*`（由根 `Cargo.toml` 的 `[workspace.dependencies]` 统一声明）
- 质量要求：TDD，行/语句/函数覆盖率 >= 80%（分支 >= 70%）

## 许可证

本项目采用 **AGPL-3.0-or-later** 许可证，详见 [LICENSE](../../LICENSE)。
