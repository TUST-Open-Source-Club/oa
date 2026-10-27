# electron（桌面端壳）

- 技术：Electron（Chromium 内嵌，屏幕共享/会议全平台一致）+ electron-updater
- 需求：见 `docs/requirements.md` 13.2（DESK-001 ~ DESK-014）与 13.1.1
- 已完成：主窗口、托盘与未读角标（macOS Dock/Linux 任务栏 + 托盘提示）、深链（`cluboa://` 与 `?server=`）、服务器设置页（OIDC discovery 校验）、contextBridge（`window.clubOA`）、本地通知、自动更新接线
- 待办：
  1. `pnpm install` 后本地验证（需要 Electron 运行环境）
  2. 桌面通知点击深链跳转具体会话
  3. 打包签名：macOS Developer ID + 公证、Windows 代码签名（见 13.8 / Q21）
