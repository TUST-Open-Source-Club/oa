# harmony-shell（HarmonyOS NEXT PWA 壳）

- 技术：ArkTS + ArkWeb（Web 组件）+ 华为 Push Kit
- 需求：见 `docs/requirements.md` 13.4（HARM-001 ~ HARM-009）与 13.1.1（服务器地址可配置）
- 已完成：工程骨架、服务器设置页（OIDC discovery 校验）、ArkWeb 容器（JS/DOM Storage/媒体、权限放行、ClubOA 桥）
- 待办：
  1. Push Kit 接入（需 AGC 项目 + 签名证书；服务端调用 Push Kit REST API）
  2. 扫码（scanCore）、文件选择、深链参数解析
  3. 使用 DevEco Studio 编译验证（本仓库暂无 HarmonyOS SDK）
