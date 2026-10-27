# club-oa-libs

社团 OA 共享 Rust 库，单仓库中位于 `libs/`，是根 Cargo workspace 的成员。

| crate | 说明 |
| --- | --- |
| `crates/common` | 统一错误（RFC 7807）、分页、UUIDv7、校验 |
| `crates/auth-sdk` | JWT claims / 签发 / 验签、JWKS、axum 提取器、服务间令牌、Bot 权限守卫 |
| `crates/storage` | 存储抽象：S3（已有对象存储）/ 本地磁盘 |
| `crates/bus` | Redis Streams 事件总线与 outbox、变更日志（change_log） |

## 使用方式

服务通过根 `Cargo.toml` 的 workspace 依赖引用，不再在各自清单里写路径：

```toml
# 根 Cargo.toml
[workspace.dependencies]
club-common = { version = "0.1.0", path = "libs/crates/common" }
```

```toml
# services/<svc>/Cargo.toml
[dependencies]
club-common.workspace = true
```

需要额外特性时在成员清单里补充，例如 `club-storage = { workspace = true, features = ["s3"] }`。

## 测试与覆盖率

```bash
cargo test -p club-common -p club-auth-sdk -p club-storage -p club-bus
cargo llvm-cov --workspace --fail-under-lines 80
```

> S3 集成测试在未设置 `S3_TEST_ENDPOINT` 时自动跳过；需要时用 `libs/scripts/s3-test-env.sh start` 起本地 MinIO。

## 许可证

本项目采用 **AGPL-3.0-or-later** 许可证，详见 [LICENSE](../LICENSE)。
