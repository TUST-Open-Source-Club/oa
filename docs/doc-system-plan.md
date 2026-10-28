# 文档系统改造计划（对标语雀）

> 状态：已批准（2026-10-28）。决策：实时协同放 P2 单独评估；评论用块级锚点；图片 doc 自存小图、大文件走 drive。

## P0 · 基础体验

- [x] 目录树操作：重命名 / 移动 / 同级上移下移 / 软删（整棵子树）；面包屑已有
- [x] 文档级可见性与分享：`private / space`，分享链接（token + 可选密码 + 有效期 + 只读）
- [x] 搜索：标题 + 正文 `pg_trgm`，范围 = 我可读的空间
- [ ] 导出 / 导入：Markdown、HTML；导入单篇 Markdown
- [ ] 编辑器增强：自动保存 + 冲突提示、字数统计、图片粘贴上传、表格渲染

## P1 · 协作

- [ ] 评论：块级锚点、回复、解决/未解决，`@` 接 notify
- [ ] 版本 diff（任选两版）+ 恢复
- [ ] 关注文档 → 正文变更通知（走 notify 偏好）
- [ ] 权限细化：空间角色 owner/admin/editor/reader + 文档级覆盖

## P2 · 进阶

- [ ] 实时协同编辑（Yjs/CRDT + `/ws/doc`），单独评估
- [ ] 模板、反向链接、阅读/编辑统计、导出归档

## 数据模型增量

```sql
nodes += visibility(text default 'space'), share_token, share_password_hash,
         share_expires_at, deleted_at(已有)
node_comments(id, node_id, parent_id, author_id, anchor jsonb, body, resolved_at, created_at)
node_stars(user_id, node_id, created_at)
node_follows(user_id, node_id, created_at)
索引：gin(nodes.title gin_trgm_ops), gin(nodes.content_md gin_trgm_ops)
```

## API 草案

```
PATCH  /spaces/{id}/nodes/{node_id}        { title?, parentId?, position? }   ✅
DELETE /spaces/{id}/nodes/{node_id}         软删整棵子树                        ✅
POST   /spaces/{id}/nodes/{node_id}/restore 恢复整棵子树                        ✅
POST   /spaces/{id}/nodes/{node_id}/share   ✅
GET    /public/{token}                        ✅
GET    /search?q=                        ✅
GET    /spaces/{id}/nodes/{node_id}/export?format=md|html
POST   /spaces/{id}/import
GET/POST /spaces/{id}/nodes/{node_id}/comments
PATCH  /comments/{id}
POST   /spaces/{id}/nodes/{node_id}/star
GET    /stars
```

## 已完成（P0 第 1 项）

迁移：复用已有 `nodes.deleted_at`，无新表。

接口：
- `PATCH /spaces/{id}/nodes/{nodeId}`：`title` / `parentId`（显式 null = 移到根）/ `position`；
  校验标题长度、父节点必须为同空间文件夹、禁止移动到自身子孙（422 `DOC_TREE_CYCLE`）；
  移动且未指定排序时自动排到新父级末尾。
- `DELETE`：软删节点及全部子孙（`deleted_at`）。
- `POST .../restore`：恢复整棵子树；父节点仍删除时 422 `DOC_PARENT_DELETED`。

前端：树节点右键菜单（重命名 / 移动到… / 上移 / 下移 / 删除）、重命名与移动弹窗；
截图 `docs/ui-preview/docs-tree-menu.png`。

测试：`services/doc/tests/tree_flow.rs` 4 条。

## 已完成（P0 第 2 项 · 可见性 + 分享）

迁移 v3：`nodes.visibility`（`space` 默认 / `private`）、`share_token`、`share_password_hash`、
`share_expires_at`、`share_permission`，`share_token` 唯一部分索引。

接口：
- `PATCH .../nodes/{nodeId}` 增加 `visibility`；`viewer` 在树里看不到 private，直接读取 403；
- `POST .../nodes/{nodeId}/share`（仅编辑者）：生成/轮换链接，可选密码与过期时间，
  密码用 `sha256(token || password)`，token 即 128 位随机盐；
- `GET .../share` 查看当前分享；`DELETE .../share` 关闭；
- `GET /api/v1/doc/public/{token}?password=`：免登录只读，缺少密码 401、
  密码错误 401、过期 410 `DOC_SHARE_EXPIRED`、关闭/不存在 404。

`AppError` 新增 `Gone`（410）变体。

前端：文档顶栏「分享」弹窗（可见性切换、生成/更新/关闭链接、密码、有效期、复制）；
公开页 `/s/doc/{token}`（只读渲染 + 密码框 + 过期提示），BFF 新增 `/api/doc/public/[token]`。

测试：`services/doc/tests/share_flow.rs` 5 条。
截图：`docs/ui-preview/docs-share-modal.png`、`docs-public-page.png`。

## 已完成（P0 第 3 项 · 搜索）

迁移 v4：启用 `pg_trgm`，给 `nodes(title)` 与 `nodes(content_md)` 建 GIN 索引
（operator class 按扩展所在 schema 限定，失败用 SAVEPOINT 降级为顺序扫描）。

接口：`GET /api/v1/doc/search?q=&limit=`，检索我可读空间内的页面（标题或正文），
viewer 自动排除 `private`，已删除不出现，按更新时间倒序，返回带 40 字上下文 `snippet`；
关键词 1~200 字（422 `DOC_VALIDATION`），`limit` 默认 20 / 上限 50。

前端：文档树顶部搜索框改为服务端检索（防抖 300ms），结果展示空间名/标题/摘要，
点击自动切换空间并打开文档。

测试：`services/doc/tests/search_flow.rs` 4 条。
截图：`docs/ui-preview/docs-search.png`。
