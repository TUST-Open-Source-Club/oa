# club-oa-app（客户端壳集合）

单仓库组件：`apps/app/`。

| 目录 | 形态 | 技术 |
| --- | --- | --- |
| `electron/` | 桌面端（Windows/macOS/Linux） | Electron + 共享 Vue 前端 |
| `android-shell/` | Android App（PWA 壳） | Kotlin + WebView + 厂商推送 SDK |
| `harmony-shell/` | HarmonyOS NEXT App（PWA 壳，规划中） | ArkTS + ArkWeb + Push Kit |

共同要求（见需求 13.1.1）：**服务器地址不硬编码**，首次启动配置并校验 OIDC discovery，支持随时切换。

## 与 monorepo 的关系

- `electron/` 是根 pnpm workspace 的成员（包名 `club-oa-electron`），依赖在仓库根目录统一安装。
- 桌面端开发 / 打包：

  ```bash
  pnpm install                        # 根目录执行一次
  pnpm --filter club-oa-electron dev
  pnpm --filter club-oa-electron dist
  ```

- `electron` 的安装脚本（下载运行时）需被允许执行，根 `pnpm-workspace.yaml` 已通过 `allowBuilds` 放行；
  若本地被跳过，可执行 `pnpm --filter club-oa-electron rebuild electron`。
- `android-shell/`、`harmony-shell/` 使用各自的原生构建工具（Gradle / DevEco），不参与 pnpm workspace 安装。
