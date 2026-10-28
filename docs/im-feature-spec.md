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

### 7.1 消息编辑（已完成）

接口：

- `PATCH /conversations/{cid}/messages/{mid}`，请求体 `{ "content": {...} }`，返回更新后的消息；
- `GET /conversations/{cid}/messages/{mid}/revisions`，返回版本快照（`version` 倒序）。

规则：

- 仅发送者本人可编辑；群主/管理员同样不能改他人消息（403 `IM_FORBIDDEN`）；
- 已撤回消息不可编辑（400 `IM_MESSAGE_RECALLED`）；
- 正文与原内容一致时幂等返回，不写版本、不更新 `editedAt`；
- 首次编辑把原内容补录为 `version=1`（编辑者=发送者、时间=发送时间），新正文为 `version=2`，之后每次编辑版本号 +1；
- 编辑不改变 `seq`，不重排会话；消息列表接口返回 `editedAt`，前端显示「已编辑」并可查看历史。

数据：`message_revisions(id, message_id, version, content, editor_id, created_at)`，索引 `(message_id, version)`；写入在同一事务内完成并锁消息行，避免并发版本号冲突。

覆盖用例：`services/im/tests/edit_flow.rs`（8 条：本人编辑、原内容补录、多次编辑追加、非发送者 403、已撤回 400、空内容 422、非成员 403、无变化不写版本）。

### 7.2 引用回复（已完成）

- 前端右键消息「回复」→ 输入框上方显示被引用消息（可取消），发送时带 `replyToId`；
- 气泡内渲染引用块（发送者 + 摘要），点击滚动到原消息并短暂高亮；
- 原消息已撤回时显示「该消息已撤回」，未加载到窗口内显示「原消息不可用」；
- 服务端已有校验（引用不存在的消息、跨会话引用 → 422 `IM_VALIDATION`），
  本步补回归用例：`services/im/tests/reply_flow.rs`（落库回读、跨会话拒绝）。

### 7.3 线程（已完成）

- `messages.thread_root_id`（迁移 v5，部分索引 `WHERE thread_root_id IS NOT NULL`）；
- 发送时带 `threadRootId`：根须在同一会话（422 `IM_VALIDATION`），且根不能是线程回复（400 `IM_THREAD_NESTED`）；
- `GET /conversations/{cid}/messages` 只返回主消息流，根消息带 `threadReplyCount`；
- `GET /conversations/{cid}/messages/{mid}/thread` 返回该根的全部回复（按 `seq` 升序，仅成员可读）；
- 前端：根消息显示「N 条回复」，右侧线程面板展示根预览 + 回复列表 + 回复输入框，会话列表展示回复数；
- 用例：`services/im/tests/thread_flow.rs`（4 条：隐藏与计数、跨会话/不存在根、嵌套拒绝、成员权限）。

### 7.4 表情回应（已完成）

- `message_reactions(message_id, user_id, emoji, created_at)`，联合主键，点第二次即取消；
- `POST /conversations/{cid}/messages/{mid}/reactions`（body `{emoji}`）切换并返回聚合 `[{emoji, count, mine}]`；
- 消息列表与线程回复都带 `reactions`；仅成员可回应，已撤回消息 400，空/含空白 emoji 422；
- WS 广播 `message_reactions`；前端气泡下展示表情胶囊（自己回应过高亮），右键菜单提供快捷表情；
- 用例：`services/im/tests/reactions_flow.rs`（3 条）。

### 7.5 已读回执（前端补齐）

- 后端已有 `GET /conversations/{cid}/messages/{mid}/receipts`（成员 → 是否读到该消息）；
- 前端在群聊输入框上方显示「已读 N/M」：取自己最近一条已加载消息的回执，
  随 4s 轮询刷新；单聊与无自己消息时不显示。

### 7.6 消息搜索（已完成）

- `GET /conversations/{cid}/messages/search?q=&limit=`：会话内搜索；
- `GET /search/messages?q=&limit=`：跨会话搜索，仅覆盖调用者是成员的会话；
- 匹配 `content->>'text'` 的大小写不敏感子串，`%`/`_` 按字面转义（Postgres 默认反斜杠转义），
  已撤回消息不参与；结果按 `seq` 倒序，带 `conversationId` 便于跳转；
- 关键词去空白后不能为空、≤ 100 字（422 `IM_VALIDATION`）；`limit` 默认 20、上限 50；
- 迁移 v7 尝试启用 `pg_trgm` + GIN 索引（`(content->>'text') gin_trgm_ops`，
  operator class 按 `pg_extension.extnamespace` 限定以兼容多 schema；无扩展/无权限时降级为顺序扫描）；
- 前端：会话列表顶部搜索入口 + 防抖 300ms，结果展示会话名/正文/发送者，
  点击跳到对应会话并滚动高亮（结果不在已加载窗口时按 `beforeSeq` 补拉）；
- 用例：`services/im/tests/search_flow.rs`（4 条）。

### 7.7 正在输入（已完成）

- `POST /conversations/{cid}/typing`：输入心跳（204，仅成员）；
- 会话列表返回 `typingUserIds`（会话内近 5 秒心跳，含自己，前端过滤）；
- 注册表在进程内存（单实例；多实例需换 Redis），`touch` 时顺带清理过期项；
- 前端：输入框节流 2 秒上报一次，会话头部与列表预览显示「X 正在输入…」；
- 说明：当前页面用 4s 轮询而非 WS，所以走 HTTP 心跳而非 spec 里的 WS `typing` 事件；
- 用例：`services/im/tests/typing_flow.rs`（2 条）+ `TypingRegistry` 过期单测。

### 7.8 会话通知规则（已完成）

- 迁移 v8：`conversation_members.notify_level`（`all` / `mentions` / `mute`，默认 `all`）；
- `POST /conversations/{cid}/notify` body `{level}`：设置**自己**在该会话的级别；
  会话列表返回 `notifyLevel`；非法级别 422，非成员 403；
- 发送消息时按每位成员的级别裁剪 notify 提醒对象：`mute` 不提醒、`mentions` 仅在自己被 @
  或消息含 `@所有人` 时提醒（`domain::should_notify`，含单测）；
- 前端：会话右键菜单「通知」三项（全部 / 仅 @我 / 静音），当前项打勾；
- 用例：`services/im/tests/notify_level_flow.rs`（2 条）。

### 7.9 媒体消息（已完成）

- 迁移 v9：`im_files(id, uploader_id, name, mime, size, data bytea, created_at)`，
  小文件（≤5MB）存库，避免 IM 依赖额外对象存储；大文件仍走 drive；
- `POST /uploads`（JSON `{name, mime, dataBase64}`）上传，返回附件元数据 + 签名 URL；
- `GET /files/{id}?token=` 用 HS256（服务内部密钥，1 小时）签名下载，**不需要 Authorization**，
  便于 `<img>`/`<a download>` 直接引用；签名与文件 ID 不匹配 401；
- 消息 `type` 为 `image`/`file`，`content.attachments` 保存附件元数据，随消息与历史回读；
- 前端：输入框回形针按钮选择文件（≤4MB 提前拦截），图片消息在气泡内直接渲染，
  其他附件显示为可下载文件名卡片；BFF 新增 `/api/im/files/[id]` 转发签名下载；
- 用例：`services/im/tests/media_flow.rs`（3 条：上传/签名下载/越权签名/校验、图片消息回读）。
