# club-oa/packages

前端共享库，单仓库中位于 `packages/`，由根 `pnpm-workspace.yaml` 统一管理。

| 包 | 说明 |
| --- | --- |
| `ui`（`@club-oa/ui`） | 设计系统组件库 + Tailwind token（Web 与客户端共用） |
| `core`（`@club-oa/core`） | API SDK、Pinia stores、WS 实时、平台适配层 |

## 使用方式

各 App 通过 workspace 协议引用，不需要 `link:` 或写死版本号：

```json
"dependencies": {
  "@club-oa/ui": "workspace:*",
  "@club-oa/core": "workspace:*"
}
```

## 测试

```bash
pnpm --filter './packages/*' test
pnpm --filter './packages/*' typecheck
```

## 许可证

本项目采用 **AGPL-3.0-or-later** 许可证，详见 [LICENSE](../LICENSE)。
