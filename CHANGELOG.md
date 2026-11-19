# Changelog

## 0.4.0

多用户的地基：Postgres、User 登录、Workspace、API Key。所有破坏性改动都集中在这一版，升级前请读完下面一节。

### 破坏性改动

- **只支持 Postgres，数据不迁移**：server 改用 Postgres 存储（[ADR-0007](docs/adr/0007-postgres-only.md)），`CRABLINK_DATABASE_URL` 必填且是 Postgres URL；SQLite 数据不会迁移过来，新实例从空库开始，Link 需要重新创建。`compose.yaml` 同时起 server 和 `postgres:18`。(#53)
- **`CRABLINK_TOKEN` 移除**：程序和 CLI 改用 Workspace 的 API Key（`Authorization: Bearer <API Key>`）。仍设置着 `CRABLINK_TOKEN` 时 server 只在启动时打一条警告，照常启动；旧 token 当作未知 Bearer 值，得到 401（`unauthorized`），提示用 API Key 重新登录。(#55)
- **API 路径按 Workspace 划分**：`/api/links…` 移到 `/api/workspaces/{ws}/links…`（`{ws}` 是 Workspace 的 slug）。旧路径带旧 token 得到上面的 401，带 API Key 或 session 得到 404。这些路径接受该 Workspace 成员的 session cookie 或它的 API Key，其他凭据 403（`forbidden`）。(#55)
- **CLI 需要用 API Key 重新 `login`**：`crablink login` 改为输入 API Key，向 `/api/me` 问出它的 Workspace，与服务器地址一起存进配置；0.2 / 0.3 配置里的 `token` 不再生效。新增 `--api-key` / `--workspace` 参数和 `CRABLINK_API_KEY` / `CRABLINK_WORKSPACE` 环境变量。0.3 的 CLI 对 0.4 的 server 只会得到 401，提示重新登录。(#55)
- **Click Count 只数人**：每次真人跳转记一个 Click，Click Count 就是 Click 的个数；爬虫和链接预览（Slack 等，按 User-Agent 识别）照常 302，但不再计数。(#64)
- **`CRABLINK_BASE_URL` 的路径不再进 Short URL**：启动时用 `CRABLINK_BASE_URL` 的 host（和端口）建 Default Domain，Short URL 由 Link 的 Domain 拼出，是 `scheme://host/{code}`；`https://example.com/s` 这类带路径的地址，路径部分不再出现在 Short URL 里。(#55)

### 新功能

- **User 登录**：`POST /api/session` 用 email 和密码登录，设置 HttpOnly、Secure、SameSite=Lax 的 session cookie，30 天过期；`DELETE /api/session` 退出。带 cookie 的写请求必须带与 Host 一致的 `Origin`。`POST /api/me/password` 改密码，并结束该 User 的其他 session。密码用 argon2id 哈希，email 不区分大小写唯一。(#54)
- **`crablink-server admin create --email`**：建第一个 Instance Admin，密码从提示或 stdin 读入。(#54)
- **`GET /api/me`**：session 返回 `{ "user": … }`，API Key 返回 `{ "workspace": … }`；CLI 靠它认出 API Key 所属的 Workspace。(#54, #55)
- **Workspace 与 API Key**：每个 User 首次登录时得到一个个人 Workspace（自己是 Owner）。Owner 建、列出、吊销 API Key，`secret` 只在创建时返回一次，服务器只存哈希；API Key 属于 Workspace，创建者离开后照样可用。(#55)
- **成员与角色**：`GET /api/workspaces` 带上角色；新建、改名、删除 Workspace（删除后其 Link 变为已删除，Code 仍被占用，API Key 失效）；Owner 按 email 加成员、升降角色、移除成员，Member 可以退出。Member 只能管理 Link；最后一个 Owner 不能退出、被移除或降级（409 `last_owner`）。(#56)
- **邀请与 User 管理**：Instance Admin 用 `POST /api/admin/invites` 生成一次性邀请链接（7 天有效，可指定加入的 Workspace），被邀请人用 `POST /api/invites/accept` 设密码、建 User 并登录；`GET /api/admin/users` 列出 User，`PATCH /api/admin/users/{id}` 停用 / 启用或设为 Instance Admin，`POST /api/admin/users/{id}/password` 重置为随机密码（只返回一次）。停用或重置密码都会结束该 User 的所有 session。`/api/admin` 下除 Instance Admin 的 session 外一律 403。(#57)
- **Click**：记时间、Referrer 的 host、国家、设备类型和浏览器，不存 IP 和完整 User-Agent（[ADR-0006](docs/adr/0006-clicks-store-no-ip.md)）。国家用镜像自带的 DB-IP Lite 数据库查（`CRABLINK_COUNTRY_DB`）；`CRABLINK_TRUST_PROXY=true` 时访客地址取 `X-Forwarded-For` 第一项，`deploy/compose.yaml` 已设好。`GET /api/workspaces/{ws}/links/{code}/clicks?from=&to=&interval=hour|day` 返回按小时 / 天分桶的 Click 数，以及按 Referrer、国家、设备、浏览器的分布。(#64)
- **生产部署**：`deploy/` 提供对外部署用的 `compose.yaml` 和 `Caddyfile`，Caddy 终止 HTTPS 并自动续证，server 和 Postgres 不对外开端口，域名和数据库密码从 `.env` 读。仓库根目录的 `compose.yaml` 只用于本地试用。(#58)

## 0.3.0

没有破坏性改动：0.2 的 CLI 可以继续连 0.3 的 server，数据库和 `CRABLINK_TOKEN` 都不用动。

- **API 错误带上 `kind`**：管理 API 的错误 body 从 `{ "error": "..." }` 变为 `{ "kind": "...", "error": "..." }`，`kind` 是稳定的 snake_case 值（如 `code_taken`、`link_not_found`、`unauthorized`），脚本可以据此分支。HTTP status 和英文 message 不变。(#49)
- **CLI `--json`**：`crablink create`、`ls`、`get`、`edit` 加 `--json`，原样打印 API 的 Link JSON；`ls --json` 输出跨所有分页的一个数组。出错时仍只写 stderr。(#50)
- **QR 码**：`GET /api/links/{code}/qr?format=svg|png` 返回 Short URL 的 QR 码，已过期的 Link 返回 410（`link_expired`），不给 QR 码；`crablink qr <code>` 在终端显示，`-o file.png|file.svg` 存成文件；TUI 里选中 Link 按 `v` 弹出 QR 码。(#51)
