# IM 功能规格（对齐 Matrix 核心能力）

| 项目 | 内容 |
| --- | --- |
| 状态 | 已确认范围，待实现 |
| 决策日期 | 2026-10-28 |
| 基线 | Matrix Client-Server API 核心子集（不含联邦 / E2EE / VoIP） |
| 关联 | `services/im/`、`apps/web/pages/im.vue`、`docs/architecture.md` 第 4 节 |

## 1. 范围与决策

| 决策点 | 结论 |
| --- | --- |
| `@所有人` 权限 | **群主 + 管理员**（成员不可用；单聊不出现该选项） |
| 线程（threads） | **纳入 P1**：消息可挂到根消息下，详情面板按线程聚合 |
| 消息编辑历史 | **保留**：编辑写入 `message_revisions`，可查看与审计，不覆盖原文 |

不做的部分：联邦、端到端加密、VoIP、Matrix 身份体系（沿用现有 auth 账号）、跨服务器桥接。

## 2. P0（本次交付）

| # | 功能 | 验收标准 |
| --- | --- | --- |
| 1 | 单聊 | 同一对用户重复发起返回已有会话；单聊不出现 `@所有人`、不可退群 |
| 2 | 拉群 | 群主/管理员可拉人；被拉人收到系统消息；成员上限可配；退群/被踢后不可再读新消息 |
| 3 | `@` 提及 | 输入 `@` 弹出成员选择器；消息落库 `mentions`；被 @ 者消息高亮 + 未读优先 + 通知文案「在群里@了你」 |
| 4 | `@所有人` | 仅群主/管理员可发；渲染为独立徽标；一轮 `@所有人` 对每个成员只提醒一次 |

接口增量（P0）：

- `POST /conversations/{id}/messages` 增加 `mentions: string[]`、`replyTo?: string`、`type`；
- `GET /conversations/{id}/messages` 返回 `mentions` / `replyTo` / `editedAt`；
- 服务端校验：`mentions` 必须属于会话成员；含 `@all` 时校验调用者是群主/管理员；
- WS 推送消息体带 `mentions`，客户端据此高亮。

数据模型增量（P0）：

- `messages`：`mentions uuid[]`、`reply_to uuid null`、`type text`、`edited_at timestamptz null`；
- 索引：`(conversation_id, seq)`（已有）、`gin(mentions)`。

## 3. P1（紧随其后）

| 功能 | 要点 |
| --- | --- |
| 消息编辑 | 写 `message_revisions`（编辑前后各一版），消息体标 `editedAt`，前端显示「已编辑」并可查历史 |
| 引用回复 | `replyTo` 已在 P0 落库，P1 补前端引用块与跳转 |
| 线程 | `thread_root_id`，线程详情面板、根消息显示「N 条回复」 |
| 表情回应 | `message_reactions`（message_id, user_id, emoji）聚合计数 |
| 已读回执 | 会话内成员读到的最新 seq，输入框上方显示「已读 N 人」 |
| 正在输入 | WS 事件 `typing`，3s 超时 |
| 媒体消息 | 图片/文件/语音，复用 drive 存储或独立 uploads |
| 搜索 | 会话内 + 全量，Postgres `pg_trgm` |
| 通知规则 | 仅 @我 / 全部 / 静音，落在 `notification_preferences` 之上 |

## 4. P2

多端同步游标（替代 4s 轮询）、设备/会话管理、消息导出与审计、置顶与收藏。

## 5. 实现顺序（每步独立提交 + 测试）

1. **数据模型 + 接口**：迁移（mentions/reply_to/type/edited_at）、发送校验、返回字段；Rust 集成测试覆盖「@ 非成员被拒」「成员发 @所有人 被拒」「群主/管理员放行」。
2. **前端**：@ 选择器（含 @所有人 项，按角色显示）、消息高亮、未读优先；Vitest + 截图。
3. **通知链路**：notify 消费者识别 mentions，文案与去重；集成测试。
4. **P1 第一批**：消息编辑（含历史）、引用回复、线程。

## 6. 权限矩阵（P0 生效）

| 操作 | 群主 | 管理员 | 成员 |
| --- | --- | --- | --- |
| 发消息 / `@` 成员 | ✅ | ✅ | ✅ |
| `@所有人` | ✅ | ✅ | ❌ |
| 拉人 / 踢人 | ✅ | ✅ | ❌ |
| 改群设置 / 禁言 | ✅ | ✅ | ❌ |
| 转让群主 / 解散 | ✅ | ❌ | ❌ |
| 编辑/撤回自己的消息 | ✅ | ✅ | ✅ |
