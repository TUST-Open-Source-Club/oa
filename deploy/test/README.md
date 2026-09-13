# 测试环境（一键 docker compose）

只依赖本 compose 即可验证**不依赖第三方服务**的全部功能；演示数据自动写入。

## 启动

```bash
cd deploy/test
docker compose --profile seed up --build     # 首次构建较久（8 个 Rust 服务 + Nuxt）
docker compose logs -f seed                  # 查看种子数据写入结果
```

- 门户：http://localhost:8088
- 账号：`admin@club.test / Admin12345!`、`member1@club.test / Member12345!`、`member2@club.test / Member12345!`
- 重置（清库并重灌）：`docker compose down -v && docker compose --profile seed up --build`
- 只重灌数据（保留库）：`docker compose run --rm seed`

## 种子数据

| 模块 | 内容 |
| --- | --- |
| 账号 | 管理员（bootstrap）+ member1 / member2（已激活） |
| IM | 群聊「理事会」+ 两条消息 |
| 任务 | 项目「迎新活动筹备」+ 3 列 + 3 任务（含指派，触发通知） |
| 文档 | 空间「社团知识库」+ 目录 + 页面「新成员指南」（含正文） |
| 网盘 | 空间「公共资料」+ 目录「活动海报」 |
| 活动 | 「2026 秋季迎新（测试）」，无需审核/邮箱验证 |
| 通知 | 由任务指派事件经 outbox → Redis Streams → notify 生成 |

## 明确不可测（需第三方）

| 能力 | 替代 |
| --- | --- |
| 邮件发送（激活、验证码） | `DEV_MODE=true` 时激活令牌/验证码直接返回（种子脚本已用） |
| S3 直传/分片 | drive 使用本地存储（上传下载、分享可用） |
| 厂商推送 / APNs / FCM | notify 日志降级（通知落库、未读计数正常） |
| OnlyOffice 在线编辑 | 未包含（WOPI Host 接口仍可被任意 WOPI 客户端调用） |
| Stalwart 邮箱服务 | 同邮件行 |

## 说明

- `AUTH_ISSUER=http://gateway`：容器内服务经网关取 JWKS；浏览器访问 `localhost:8088` 不受影响。
- 网关同时暴露 `/api/v1/{auth,im,task,event,doc,drive,notify}`、`/ws/`（IM WebSocket）与 `/ntfy/`。
- 所有数据卷以 `test_` 前缀命名，与生产编排隔离。
