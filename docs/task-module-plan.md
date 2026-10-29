# 任务模块改造计划（对标 Planka）

> 状态：待确认（2026-10-30）。目标：把现有「项目内单看板」升级成 Planka 式的多板、标签、筛选、卡片抽屉与实时协作。

## 一、现状（已经有的）

| 层 | 已有能力 |
|---|---|
| 数据 | `projects`（含归档）、`project_members`（admin/member）、`board_columns`、`tasks`（多负责人、开始/截止、优先级、列内 position、状态 active/done/terminated）、`task_assignees`、`task_attachments`、`subtasks`、`comments`、`change_log`（审计+撤销） |
| 接口 | 项目 CRUD/成员、列 CRUD、任务 CRUD/移动/领取/状态、子任务勾选、附件、评论、变更记录与撤销、我的任务 |
| 通知 | `task.assigned` / `task.completed` / `task.terminated` / `task.comment.added` 已进 outbox |
| 前端 | `tasks.vue`：看板列 + 拖拽、任务弹窗（负责人/时间/优先级/描述/子任务/附件/变更）、项目成员管理、我的任务 |

## 二、与 Planka 的差距

| Planka 能力 | 我们 | 差距 |
|---|---|---|
| 项目（Project）→ 多个看板（Board）→ 列 | 项目 = 单看板 | **缺 boards 层** |
| 标签（Labels，看板级彩色标签） | 无 | 缺 |
| 筛选（成员/标签/截止/关键词） | 无 | 缺 |
| 卡片详情抽屉 + 活动流 + 清单进度 | Modal | 交互形态待升级 |
| 卡片封面 / 附件缩略图 | 仅附件列表 | 缺封面 |
| 看板成员与角色（admin/editor/viewer） | 项目成员两级 | 需看板级权限 |
| 实时同步（socket.io） | 无（手动刷新） | 缺 |
| 通知（指派/到期/评论 @） | 指派/完成/评论已有 | 缺到期提醒与 @ |
| 归档与回收站 | 项目归档 | 任务/列无归档 |
| 快捷键、快速添加 | 无 | 缺 |
| 依赖关系、WIP 限制、泳道、工时 | 无 | P2 |

## 三、分期

### P0 · 看板体验对齐（先让日常好用）

1. ✅ **引入 boards 层**：项目 → 看板 → 列 → 卡片；迁移 v5 给每个项目补默认看板，接口与前端兼容（项目页出现看板切换）。
2. **标签系统**：`labels`（看板级，名称/颜色）+ `card_labels`；卡片上显示标签点/条，详情里勾选。
3. **筛选与搜索**：按成员/标签/截止（逾期、今天、本周）/关键词过滤，看板顶部状态条。
4. **卡片详情抽屉**：从 Modal 改为右侧抽屉（保留 URL 直达 `?card=`），含描述、清单、附件、评论、活动流、标签、负责人、时间。
5. **卡片封面**：附件中的图片可设为封面，卡片顶部缩略图。
6. **归档**：卡片/列归档到看板「已归档」，可恢复。
7. **快速操作**：列底快捷加卡、键盘（N 新建、/ 搜索、Esc 关闭）。

### P1 · 协作与自动化

8. **看板权限**：`board_members(role: admin/editor/viewer)`，项目管理员可管理；viewer 只读。
9. **实时同步**：看板变更推送（WS `/ws/task` 或 2s 游标轮询），多人拖拽不互相覆盖。
10. **通知增强**：到期前提醒（每天一次）、评论 `@` 提及、列变更提醒（可关）。
11. **模板**：卡片模板、清单模板；「从模板新建卡片」。
12. **我的任务视图**：按截止/项目/标签分组，日历月视图。

### P2 · 进阶

13. 卡片依赖（blocks / blocked by）与关键路径提示
14. 列 WIP 限制、泳道（按负责人/标签分组）
15. 工时记录与燃尽图
16. 自动化规则（列变更触发：指派/加标签/通知）
17. 导入导出（CSV / JSON / Planka 迁移）

## 四、数据模型增量（草案）

```sql
boards(id, project_id, name, position, archived_at, created_at)
columns += board_id（迁移后 column 唯一归属看板）
labels(id, board_id, name, color, created_at)
card_labels(task_id, label_id, PK)
board_members(board_id, user_id, role, joined_at, PK)
tasks += cover_attachment_id, archived_at
columns += archived_at
card_templates(id, board_id, name, title, description_md, created_at)
task_dependencies(task_id, blocked_by_id, PK)
task_time_entries(id, task_id, user_id, minutes, note, created_at)
```

## 五、API 草案

```
GET/POST   /projects/{id}/boards
PATCH/DELETE /boards/{id}
GET/POST   /boards/{id}/columns
GET/POST   /boards/{id}/labels
PATCH/DELETE /labels/{id}
POST/DELETE /tasks/{id}/labels/{labelId}
GET        /boards/{id}/cards?filters=...
POST       /tasks/{id}/archive | /restore
GET/POST   /boards/{id}/members
GET        /boards/{id}/templates
POST       /tasks/{id}/cover
GET        /tasks/{id}/activity
```

## 六、前端信息架构（Planka 风格）

- 顶栏：项目切换 · 看板切换 · 搜索 · 筛选（成员/标签/截止）· 成员头像组 · 设置
- 主体：横向列（列头：名称/计数/WIP/更多），卡片（标签条、封面、标题、截止、负责人头像、清单进度）
- 右侧：卡片抽屉（标题/描述/清单/附件/评论/活动）
- 空状态与加载骨架、拖拽占位、暗色适配

## 七、节奏与验收

- 每步 TDD（真实 PG 集成测试）+ 前端 typecheck/test/build + 截图预览后再铺开
- 迁移按功能拆分：boards → labels → archive → board_members → templates
- 兼容策略：迁移时把每个项目现有列挂到「默认看板」，旧接口保留一个版本

## 八、待拍板

1. **是否引入项目 → 多看板**（Planka 有；改动最大但是对标核心）
2. **实时同步**：WS 推送（体验好、要新增通道与前端重连）还是 2s 游标轮询（简单可靠）
3. **卡片详情**：右侧抽屉（Planka 风格，改变现有 Modal 交互）还是保留 Modal
4. **标签与封面**是否都进 P0

## 已完成（P0 第 1 项 · boards 层）

- 迁移 v5：`boards(id, project_id, name, position, archived_at)`；`board_columns.board_id` 回填并置非空；
- 接口：`GET/POST /projects/{id}/boards`、`PATCH/DELETE /boards/{id}`（删除需空看板）、
  `GET/POST /boards/{id}/columns`、`GET /boards/{id}/tasks`、`POST /columns/{id}/tasks`；
  旧 `/projects/{id}/columns` 仍可用，落在默认看板；
- 前端：项目旁新增看板下拉与「新建看板」，列与任务按看板加载；
- 测试：`boards_flow.rs` 3 条；task-service 18 条全绿；
- 截图：`docs/ui-preview/task-boards.png`。

下一步 P0：标签 → 筛选 → 卡片抽屉 → 封面 → 归档。
