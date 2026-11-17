# 活动 × 任务连动设计

> 目标：让「活动」不再只是一个报名页，而是能直接驱动筹备工作；让「任务」能标明属于哪场活动，并在活动页看到筹备进度。
>
> 现状：`event` 服务已有活动 CRUD、表单 schema、公开报名、审核/候补、签到、统计、Excel 导出；`task` 服务已有项目/多看板/列/卡片、标签、依赖、WIP、工时、泳道、自动化、导入导出。两者目前互不相识。

## 1. 设计目标

| 目标 | 说明 |
|---|---|
| 一场活动 = 一块筹备看板 | 创建活动时按模板生成任务清单；活动页直接看筹备进度 |
| 任务能归属活动 | 卡片显示「活动」徽章；任务列表可按活动筛选 |
| 活动节点驱动任务 | 报名截止、活动结束等状态变化自动生成/提醒任务 |
| 不引入跨服务强依赖 | 两库不建外键、不跨库事务；靠 BFF 编排 + 幂等补偿 |
| 权限不破窗 | 沿用 event 的活动管理员与 task 的看板成员模型，BFF 负责同步 |

## 2. 典型场景

1. **创建活动 → 生成筹备清单**：选「招新宣讲」模板，自动建看板「2026 秋季招新宣讲」，生成「场地确认（活动前 7 天）」「海报设计（前 5 天）」「物料采购（前 3 天）」「现场签到表（前 1 天）」等卡片。
2. **活动详情看筹备**：活动页「筹备任务」标签显示完成度、逾期数、按列分组；点卡片直接打开任务抽屉。
3. **任务反查活动**：任务看板卡片带活动徽章；任务页筛选「活动」。
4. **报名截止联动**：报名截止当天自动创建「审核报名名单」任务并指派给活动管理员。
5. **活动结束联动**：自动创建「导出报名数据」「整理照片」「复盘会议」任务。
6. **进度汇总**：活动列表每一行显示 `x/y 完成`、逾期红点。

## 3. 架构决策

### 3.1 关联方式：各存一个外部 ID（推荐）

| 方案 | 做法 | 评价 |
|---|---|---|
| A | event 库存 `task_ids` 数组 | 双重写、易漂移，不采用 |
| B | task 库存 `event_id`；event 库存 `task_board_id` | ✅ 查询简单、无跨库事务、天然幂等 |
| C | task 库建 `task_event_links` 链接表 | 多一张表，暂不需要 |

**推荐 B**：

- `tasks.event_id uuid NULL` + 索引 `(event_id)`：任务归属活动，支持过滤与徽章。
- `events.task_project_id uuid NULL`：活动对应的筹备项目（每活动一个）。
- `events.task_board_id uuid NULL`：活动对应的筹备看板（项目默认看板）。
- 两边都是**外部引用**，不建外键、不校验存在性，避免循环依赖；由 BFF 保证一致。

### 3.2 筹备看板的组织方式（已定：每活动一个独立项目）

| 方案 | 说明 | 取舍 |
|---|---|---|
| A. 单一「活动筹备」项目 + 每活动一个看板 | 一个项目承载所有活动筹备 | 成员管理集中，但活动间隔离弱 |
| **B. 每活动一个独立项目（✅ 用户已选）** | 活动即项目，默认看板命名为活动标题 | 隔离最强；每建活动自动拉活动管理员进项目 |

实现：生成清单时创建 `项目名 = 活动标题`，项目下默认看板重命名为活动标题；
活动管理员自动写为项目成员 + 看板 admin。

### 3.3 创建时自动建 vs 显式生成

推荐**创建活动后显式点「生成筹备清单」**：

- 避免误建空看板；
- 允许先建活动、后补清单；
- 接口幂等：已有 `taskBoardId` 时返回 409 或直接复用。

### 3.4 模板来源

推荐：**内置模板 + 活动内可改**

- 内置 3 套：招新 / 讲座 / 比赛（先写死在前端常量或 event 服务的初始化数据）；
- 生成后任务可自由增删改，模板只影响初始清单；
- P1 再支持自定义模板 CRUD（`event_checklist_templates`）。

## 4. 数据模型

### 4.1 event 服务（migration v4+）

```sql
ALTER TABLE events ADD COLUMN task_board_id uuid;
ALTER TABLE events ADD COLUMN checklist_template text;  -- 选用的模板 key，审计用
CREATE INDEX ix_events_task_board ON events (task_board_id);
```

P1 可选：

```sql
CREATE TABLE event_checklist_templates (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  items jsonb NOT NULL,          -- [{title, descriptionMd, dueOffsetDays, priority, labels}]
  created_at timestamptz NOT NULL
);
```

### 4.2 task 服务（migration v17+）

```sql
ALTER TABLE tasks ADD COLUMN event_id uuid;
CREATE INDEX ix_tasks_event ON tasks (event_id);
```

可选：卡片标签沿用看板标签，不新增活动专有字段。

## 5. 接口设计

### 5.1 task 服务

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/boards/{id}/tasks?eventId=` | 按活动过滤 |
| POST | `/boards/{id}/tasks/batch` | 批量建卡（数组），每张可带 `eventId`；返回创建数量与 ID |
| PATCH | `/tasks/{id}` | 增加 `eventId`（null 解绑） |

批量接口请求示例：

```json
{
  "eventId": "…",
  "tasks": [
    { "title": "场地确认", "columnId": "…", "dueAt": "2026-11-20T10:00:00Z", "priority": "high" }
  ]
}
```

### 5.2 BFF（apps/web/server/api，Nuxt）

| 方法 | 路径 | 编排 |
|---|---|---|
| POST | `/api/event/events` | 调 event 服务创建活动；若 `createChecklist`，再调 task 服务建看板+批量建卡，最后回写 `taskBoardId` |
| GET | `/api/event/events/{id}/tasks` | 读 `taskBoardId` → 调 task 服务看板任务 → 汇总进度与逾期 |
| POST | `/api/event/events/{id}/tasks` | 在活动看板建卡，自动带 `eventId`；同步 event 管理员为看板成员 |
| GET | `/api/event/events?withProgress=1` | 活动列表 + 批量筹备进度（并发读各看板任务统计） |

编排失败处理：活动已创建、看板创建失败 → 活动详情显示「筹备清单未生成，点击重试」，重试幂等（已有 `taskBoardId` 直接返回）。

### 5.3 状态联动（P1）

event 服务在状态变化时写 outbox：

| 事件 | 触发 | 消费动作 |
|---|---|---|
| `event.reg_closed` | 报名截止 | 在活动看板建「审核报名名单」并指派活动管理员 |
| `event.finished` | 活动结束 | 建「导出报名数据」「整理照片」「复盘会议」 |
| `event.opened` | 开放报名 | 通知活动管理员 + 任务负责人 |

消费方式：task 服务新增 `automation` 触发源（读 outbox 的 event 类型），或由 notify 服务转发；P1 实现，P0 先手动生成。

## 6. 权限

| 角色 | 活动 | 筹备任务 |
|---|---|---|
| 活动管理员（`event_admins`） | 管理活动、生成清单 | 同步为看板 admin（可改） |
| 普通成员 | 只读活动 | 按 task 现有看板权限 |
| 报名者（游客） | 公开页只读 | 不可见 |

- 生成看板时：BFF 把活动管理员写成 `board_members(role=admin)`；
- 活动增删协管 → 同步看板成员（P1，补偿任务）；
- 任务服务不校验 `eventId` 归属，BFF 只展示当前用户可见的看板任务。

## 7. 前端

### 7.1 活动详情（`apps/web/pages/events.vue`）

- 右侧/下侧新增「筹备任务」Tab：
  - 进度条：完成数/总数、逾期红点；
  - 按列分组卡片（复用任务卡片样式，点击用任务抽屉）；
  - 「生成筹备清单」按钮（未生成时）；
  - 「添加任务」快捷入口。
- 活动列表：每行显示筹备进度徽章。

### 7.2 任务页（`apps/web/pages/tasks.vue`）

- 卡片：活动徽章（`eventTitle`），点击跳活动详情；
- 筛选栏：新增「活动」下拉（数据来自关联活动的看板任务聚合，P0 可先只显示徽章、不做筛选）；
- 卡片抽屉：显示所属活动，可解绑。

### 7.3 创建活动向导

1. 基本信息（标题/slug/时间/地点/名额）；
2. 勾选「同时生成筹备清单」+ 选模板；
3. 预览将要生成的卡片（标题 + 截止偏移）；
4. 创建。

## 8. 通知

| 时机 | 接收人 | 渠道 |
|---|---|---|
| 活动开放报名 | 活动管理员 | 站内 + ntfy |
| 报名截止前 1 天 | 活动管理员 | 站内 + ntfy |
| 活动前 1 天 | 活动管理员 + 筹备任务负责人 | 站内 + ntfy |
| 任务逾期 | 任务负责人 | 已有到期提醒机制复用 |

复用 `club-bus` outbox 与 notify 服务；活动事件的 payload 与任务事件保持同一结构。

## 9. 分阶段交付

### P0（核心连动，约 3 个增量）

1. 迁移：`events.task_project_id / task_board_id / checklist_template`、`tasks.event_id` + 索引；
2. task 服务：`GET /boards/{id}/tasks?eventId=`、`PATCH /tasks/{id}` 支持 eventId；
3. BFF：创建活动 + 生成看板/批量建卡（内置模板）、活动详情任务聚合；
4. 前端：活动详情「筹备任务」、任务卡片活动徽章；
5. 测试：`event_link_flow`（生成清单、过滤、进度、幂等重试）。

### P1

- 模板 CRUD（`event_checklist_templates`）+ 批量建卡接口；
- 活动状态节点自动建卡（报名截止/结束）；
- 活动管理员 ↔ 看板成员同步；
- 活动列表进度汇总、任务页活动筛选。

### P2

- 复制活动（含任务结构）；
- 报名数据驱动任务（如按人数调整物资）；
- 活动甘特/日历联动、ICS。

## 10. 风险与边界

| 风险 | 处理 |
|---|---|
| 看板创建失败、活动已存在 | 活动页「重试生成」，幂等（`taskBoardId` 已存在直接复用） |
| 删除活动 | 不级联删看板/任务；看板归档，任务保留审计 |
| 权限双写不一致 | BFF 写入时重试 + 详情页展示「同步成员」按钮（P1） |
| 任务被移到其它看板 | `eventId` 仍保留，活动页按 `eventId` 全局查询兜底（P1） |
| 跨服务枚举 | 不建外键；`eventId` 仅作过滤与展示 |

## 11. 待确认决策（默认按推荐执行）

| # | 决策 | 推荐 |
|---|---|---|
| 1 | 筹备看板组织方式 | ✅ 每活动一个独立项目 |
| 2 | 生成清单时机 | ✅ 显式「生成筹备清单」，幂等可重试 |
| 3 | 模板范围 | ✅ 先内置 3 套（招新/讲座/比赛），P1 再做自定义 |
| 4 | 状态联动 | ✅ P1 做（报名截止/结束自动建卡） |
| 5 | 报名者可见范围 | ✅ 仅活动管理员可见筹备任务 |

## 实施进度（P0）

已完成：

- task 服务迁移 v17：`tasks.event_id` + 索引；`PATCH /tasks/{id}` 支持 `eventId`，
  **创建任务（`POST /columns/{id}/tasks`）也接受 `eventId`**；任务 DTO 带 `eventId`；
  `GET /boards/{id}/tasks?eventId=` 过滤；
- event 服务迁移 v3：`events.task_project_id / task_board_id / checklist_template`；
  `PUT /events/{id}/checklist`（活动管理员，幂等）；
- BFF：`POST /api/event/events/{id}/checklist`（独立项目 + 看板 + 模板卡片 + 回写）、
  `GET /api/event/events/{id}/tasks`（筹备进度）、`POST /api/event/events/{id}/tasks`；
- 模板：`apps/web/utils/event-templates.ts`（招新/讲座/比赛，按开始时间倒推截止）；
- 前端：活动页「筹备任务」卡片（进度、清单、生成按钮）、任务卡片活动徽章、
  `/tasks?board=&card=` 直达、`/events?id=` 直达。

测试：task `event_link_flow` 1 条、event `event_checklist_flow` 2 条、
前端 `event-templates.spec` 5 条。

P1 待做：状态联动自动建卡、模板 CRUD、活动管理员 ↔ 看板成员同步、活动列表进度汇总。

### 冒烟验证（本地全栈）

- 创建活动「招新宣讲会」→ `POST /api/event/events/{id}/checklist {templateKey: recruit}`
  → 新建项目 + 看板，生成 6 张卡片，`existed: false`；
- 重复调用 → `existed: true`，不重复建卡；
- 看板任务 6 张全部带 `eventId`；完成一张后 `GET /events/{id}/tasks` 进度变为 `1/6`；
- 活动列表返回 `taskBoardId` / `checklistTemplate`；
- 「讲座」模板 4 张卡片同样全带 `eventId`。
