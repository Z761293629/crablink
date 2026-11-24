# HTTP API

Link 属于 Workspace，建在 Default Domain 或该 Workspace 已批准的自定义 Domain 上；Code 在其 Domain 内唯一，删除后也不再复用。`{ws}` 是 Workspace 的 slug，`/api/workspaces/{ws}/links…` 接受该 Workspace 成员的 session cookie 或它的 API Key，其他凭据一律 403（`forbidden`）。单个 Link 用 Code 加 `?domain=<Domain>` 定位，省略 `domain` 即 Default Domain。

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/{code}` | 按 `Host` 找 Domain 再 302 跳转；未知或未批准的 Domain、不存在的 Code 404；已删除或已过期 410 |
| `GET` | `/internal/domain-allowed?domain=` | 无需凭据，供 Caddy 询问：Default Domain 和已批准的 Domain 200，其余 404 |
| `POST` | `/api/workspaces/{ws}/links` | 创建 Link：`{ target, code?, expires_at?, domain?, tags? }`；`domain` 不可用 400（`domain_not_approved`）；`code` 是路由用词（`api`、`admin`、`app`、`health`，不分大小写），或在 Default Domain 上是 Reserved Code（`CRABLINK_RESERVED_CODES`），400（`code_reserved`）；返回的 Link 带 `domain`、`short_url` 和 `tags` |
| `GET` | `/api/workspaces/{ws}/links?tag=&q=` | 列表（所有 Domain），新的在前，游标分页；`tag` 按 Tag 过滤，`q` 搜 Code 或 Target（不区分大小写） |
| `GET` | `/api/workspaces/{ws}/links.csv` | 导出 CSV：Code、Domain、Target、Expiry、Tags（逗号分隔）、Click Count |
| `POST` | `/api/workspaces/{ws}/links.csv` | 导入同格式的 CSV，逐行建 Link（不改已有 Link，空 Code 用 Generated Code，忽略 Click Count）；返回每行的 `short_url` 或 `skipped: { kind, error }` |
| `GET` | `/api/workspaces/{ws}/tags` | Workspace 里有 Link 在用的 Tag |
| `GET` | `/api/workspaces/{ws}/links/{code}?domain=` | 查看单个 Link |
| `PATCH` | `/api/workspaces/{ws}/links/{code}?domain=` | 修改 `target` / `expires_at` / `tags`（整体替换） |
| `DELETE` | `/api/workspaces/{ws}/links/{code}?domain=` | 删除（不可恢复） |
| `GET` | `/api/workspaces/{ws}/links/{code}/qr?format=svg\|png&domain=` | Short URL 的 QR 码，默认 SVG；已过期 410（`link_expired`） |
| `GET` | `/api/workspaces/{ws}/links/{code}/clicks?from=&to=&interval=hour\|day&domain=` | `[from, to)` 内的 Click：`series` 按 UTC 小时 / 天分桶（默认天，空桶也列出，最多 1000 个），`referrers` / `countries` / `devices` / `browsers` 为 `[{ value, count }]`，多的在前，未知为 `null` |
| `GET` | `/api/workspaces/{ws}/clicks?from=&to=&interval=hour\|day` | 整个 Workspace 的 Click（含已删除 Link 的）按桶统计，以及区间内 Click 最多的 10 个 Link：`top_links: [{ code, domain, short_url, count }]` |
| `GET` | `/api/workspaces/{ws}/domains` | 列出 Default Domain 和本 Workspace 的自定义 Domain：`[{ domain, status, default }]`，`status` 为 `pending` / `approved` / `rejected` |
| `POST` | `/api/workspaces/{ws}/domains` | Owner 添加自定义 Domain：`{ domain }`，状态 pending；不是合法 host 名 400（`invalid_domain`），是 Default Domain 或别的 Workspace 的 409（`domain_taken`） |
| `DELETE` | `/api/workspaces/{ws}/domains/{domain}` | Owner 移除 Domain：其 Link 变为已删除，Code 仍被占用；不属于本 Workspace 404（`domain_not_found`） |
| `GET` | `/api/admin/domains` | Instance Admin 列出各 Workspace 的自定义 Domain：`[{ domain, status, workspace_slug }]` |
| `PATCH` | `/api/admin/domains/{domain}` | Instance Admin 批准或拒绝：`{ status }` |
| `POST` | `/api/workspaces/{ws}/api-keys` | Owner 建 API Key：`{ name }`；`secret` 只在这次返回，服务器只存哈希 |
| `GET` | `/api/workspaces/{ws}/api-keys` | Owner 列出未吊销的 API Key：`name`、`prefix`、`created_at`、`last_used_at` |
| `DELETE` | `/api/workspaces/{ws}/api-keys/{id}` | Owner 吊销 API Key，立即失效 |
| `POST` / `DELETE` | `/api/session` | 用 `{ email, password }` 登录（设置 session cookie）/ 退出 |
| `GET` | `/api/me` | session：`{ "user": … }`；API Key：`{ "workspace": … }` |
| `POST` | `/api/me/password` | 修改密码 |
| `GET` | `/api/workspaces` | 当前 User 所在的 Workspace 及其角色：`[{ slug, name, role }]`，`role` 为 `owner` / `member` |
| `POST` | `/api/workspaces` | 新建 Workspace：`{ name }`，slug 由名字生成，创建者成为 Owner |
| `PATCH` | `/api/workspaces/{ws}` | Owner 改名：`{ name }`，slug 不变 |
| `DELETE` | `/api/workspaces/{ws}` | Owner 删除 Workspace：其 Link 变为已删除（Code 仍被占用，Short URL 410），API Key 随之失效（401） |
| `GET` | `/api/workspaces/{ws}/members` | Owner 列出成员：`[{ email, role }]` |
| `POST` | `/api/workspaces/{ws}/members` | Owner 按 email 把已有 User 加为 Member：`{ email }`；无此 User 404（`user_not_found`），已在其中 409（`already_member`） |
| `PATCH` | `/api/workspaces/{ws}/members/{email}` | Owner 升降角色：`{ role }` |
| `DELETE` | `/api/workspaces/{ws}/members/{email}` | Owner 移除成员；Member 只能移除自己（退出） |
| `POST` | `/api/admin/invites` | Instance Admin 邀请：`{ email, workspace? }`，返回一次性 `url`（7 天有效）；email 已注册 409（`email_taken`） |
| `POST` | `/api/invites/accept` | 无需凭据：`{ token, password }` 建 User 并登录；`invite_not_found` / `invite_used` / `invite_expired` |
| `GET` | `/api/admin/users` | Instance Admin 列出 User：`[{ id, email, instance_admin, disabled }]` |
| `PATCH` | `/api/admin/users/{id}` | Instance Admin 改 `disabled` / `instance_admin`；会让实例没有启用的 Instance Admin 时 409（`last_instance_admin`）；停用即结束其 session |
| `POST` | `/api/admin/users/{id}/password` | Instance Admin 重置为随机密码：`{ password }` 只返回这一次，并结束其 session |

Member 只能管理 Link；成员、API Key 和 Workspace 设置只归 Owner，Member 或 API Key 去做得到 403（`forbidden`）。`/api/admin` 只接受 Instance Admin 的 session，其他凭据 403。最后一个 Owner 不能退出、被移除或降级，得到 409（`last_owner`）。

程序用 `Authorization: Bearer <API Key>`；API Key 属于 Workspace，创建它的人离开后照样可用。带 session cookie 的写请求必须带与 Host 一致的 `Origin`。未知或已吊销的 Bearer 值（包括 0.2 / 0.3 的 token）得到 401，提示用 API Key 重新登录。
