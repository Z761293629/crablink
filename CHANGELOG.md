# Changelog

## 0.6.1

web UI 的小改动，server、API 和 CLI 没有变化。

### 升级说明

- **从 0.6 升级**：换上新镜像重启即可，没有数据库迁移。

### 新功能

- **撕票**：登录、接受邀请和新建 Link 成功时，票的下半截（新建 Link 是登机牌的存根）沿齿孔线从一端撕开，留下毛糙的纤维边，然后脱落。密码错误、Code 已被占用这类被拒绝的提交只撕开一部分并停在那里，下次成功时接着撕完。跟随系统的「减少动态效果」设置时不撕。(#76)

### 修复

- **齿孔缺口**：票和登机牌两侧的半圆缺口原来是用背景色画上去的，颜色和背景对不上，卡片的阴影也在缺口处断开。现在缺口是真的挖空，阴影沿缺口的边走。(#76)
- **密码错误时重试会闪**：再次点登录时，上一次的错误提示不再先消失，所以按钮不会上下跳一下。接受邀请和新建 Link 同样修复。(#76)

## 0.6.0

web UI 改版，server、API 和 CLI 没有变化。

### 升级说明

- **从 0.5 升级**：换上新镜像重启即可，没有数据库迁移，CLI 不用重新 `login`。

### 新功能

- **web UI 改版：登机牌和航班信息屏**：Link 页是一块航班信息屏，选中的 Link 显示为一张登机牌：从 Domain 飞往 Target 的航线，二维码在可撕下的存根上，下方是这个 Link 最近 7 天 Click 的翻牌屏和排名第一的 Referrer、国家、设备。Clicks、Workspace、Domain、设置和实例管理页用同一套样式；改 Target 时卡片高度不变。新建 Link 时存根撕下，新行滑入。Inter 和 JetBrains Mono 字体随 server 自带，不再依赖系统字体；跟随系统的「减少动态效果」设置时关闭动画。(#72)
- **右上角一个菜单**：切换 Workspace、进入 Workspace 的设置和 Domain、账户、切换语言和退出都放进页头右上角的同一个菜单，语言改为两段式切换，不再是下拉框。(#72)

## 0.5.0

### 升级说明

- **从 0.4 升级**：换上新镜像重启即可，新增的表（Click、Tag 等）在启动时自动迁移，已有数据保留；CLI 不用重新 `login`。注意下面的「行为变化」：跳转要求 `Host` 是 Default Domain 或已批准的 Domain，`crablink ls` 的第一列从 Code 变为 Short URL（脚本请改用 `--json`）。
- **从 0.2 / 0.3 升级**：先读 0.4.0 的破坏性改动：新实例从空的 Postgres 开始，Link 需要重新创建，CLI 要用 API Key 重新 `login`。

### 新功能

- **Click**：记时间、Referrer 的 host、国家、设备类型和浏览器，不存 IP 和完整 User-Agent（[ADR-0006](docs/adr/0006-clicks-store-no-ip.md)）。国家用镜像自带的 DB-IP Lite 数据库查（`CRABLINK_COUNTRY_DB`）；`CRABLINK_TRUST_PROXY=true` 时访客地址取 `X-Forwarded-For` 第一项（没有这个头时取 TCP 连接的对端地址），`deploy/compose.yaml` 已设好。Click 在跳转响应发出后才写库，不拖慢跳转。`GET /api/workspaces/{ws}/links/{code}/clicks?from=&to=&interval=hour|day` 返回按小时 / 天分桶的 Click 数，以及按 Referrer、国家、设备、浏览器的分布。(#64)
- **web UI**：同一个 server 在 `/app` 提供的单页应用，用 email 和密码登录、退出，接受邀请设密码，修改自己的密码，看 Workspace 的 Link；英文和简体中文（默认跟随浏览器，可切换并记住），API 错误按 `kind` 翻译；跟随系统浅色 / 深色主题，适配手机；页头用 logo 文件，带 favicon。`web/dist` 在构建时打进 server 二进制，没构建也能编译，`/app` 会说明。(#60)
- **web UI 管理 Workspace**：新建、改名、删除 Workspace；Owner 加成员、升降角色、移除成员，Member 可以退出；Owner 建 API Key（`secret` 只显示一次）、吊销 API Key。(#62)
- **web UI 管理 Domain 和实例**：Owner 添加、移除自定义 Domain，看到审批状态和 DNS 指向；Instance Admin 生成邀请链接、批准或拒绝 Domain、停用 / 启用 User、设为或撤销 Instance Admin、重置密码。(#63)
- **Click 图表和 Workspace Click**：`GET /api/workspaces/{ws}/clicks?from=&to=&interval=` 返回整个 Workspace 的 Click 分桶（已删除 Link 的 Click 也计入，过去的总数不会因删除变少）和区间内 Click 最多的 10 个 Link；web UI 里每个 Link 有 Click 图表和 Referrer、国家、设备、浏览器分布，另有 Workspace Clicks 页。(#65)
- **CSV 导入导出**：`GET /api/workspaces/{ws}/links.csv` 导出 Code、Domain、Target、Expiry、Tags、Click Count；`POST` 同格式的 CSV 逐行建 Link，不改已有 Link，空 Code 用 Generated Code，忽略 Click Count，不是原子的：返回每行建好的 Short URL 或跳过的原因（`kind` 和 message）。web UI 的 Link 页可导出和导入。(#67)
- **自定义 Domain**：Owner 用 `/api/workspaces/{ws}/domains` 添加（pending）、列出、移除自定义 Domain；Instance Admin 用 `/api/admin/domains` 批准或拒绝。Link 可建在 Default Domain 或已批准的 Domain 上（`domain` 字段，CLI `crablink create --domain`），同一个 Code 可存在于不同 Domain；单个 Link 用 `?domain=` 定位。移除 Domain 后其 Link 变为已删除，再被任何 Workspace 添加时这些 Code 仍被占用。`GET /internal/domain-allowed?domain=` 供 Caddy on-demand TLS 询问，`deploy/Caddyfile` 已配好。(#59)
- **web UI 管理 Link**：在 `/app` 里创建 Link（Generated 或 Custom Code，可设 Expiry）、编辑 Target 和 Expiry、确认后删除，一键复制 Short URL，下载 SVG 或 PNG 二维码；分页加载，切换自己所在的 Workspace。(#61)
- **Tag 和搜索**：Link 可带任意多个 Tag（Workspace 内唯一，1-64 个字符、不含逗号，不影响跳转）：创建时 `tags`，`PATCH` 的 `tags` 整体替换；`GET /api/workspaces/{ws}/tags` 列出 Workspace 里有 Link 在用的 Tag；Link 列表加 `tag`（按 Tag 过滤）和 `q`（Code 或 Target 包含该文本，不区分大小写），照常按 cursor 分页。CLI 新增 `crablink create --tag`（可重复）、`crablink ls --tag` 和 `--search`；web UI 可编辑 Link 的 Tag、按 Tag 过滤、搜索。(#66)

### 行为变化

- **Click Count 只数人**：每次真人跳转记一个 Click，Click Count 就是 Click 的个数；爬虫和链接预览（Slack 等，按 User-Agent 识别）照常 302，但不再计数。(#64)
- **跳转按 `Host` 找 Domain**：`GET /{code}` 只在 `Host` 是 Default Domain（`CRABLINK_BASE_URL` 的 host 和端口）或已批准的 Domain 时跳转，其余 404；用别的地址（比如 IP）直接访问 server 不再跳转。(#59)
- **CLI 和 TUI 显示带 Domain 的 Short URL**：`crablink ls` 的第一列、TUI 的列表和删除确认从 Code 改为 `domain/code`，不同 Domain 上的同名 Code 能区分开。(#47)
- **最后一个 Instance Admin**：`PATCH /api/admin/users/{id}` 不再一律拒绝对自己停用或撤销，而是拒绝任何会让实例没有启用的 Instance Admin 的修改，得到 409（`last_instance_admin`）；还有别的 Instance Admin 时可以撤销自己。(#63)

## 0.4.0

多用户的地基：Postgres、User 登录、Workspace、API Key。所有破坏性改动都集中在这一版，升级前请读完下面一节。

### 破坏性改动

- **只支持 Postgres，数据不迁移**：server 改用 Postgres 存储（[ADR-0007](docs/adr/0007-postgres-only.md)），`CRABLINK_DATABASE_URL` 必填且是 Postgres URL；SQLite 数据不会迁移过来，新实例从空库开始，Link 需要重新创建。`compose.yaml` 同时起 server 和 `postgres:18`。(#53)
- **`CRABLINK_TOKEN` 移除**：程序和 CLI 改用 Workspace 的 API Key（`Authorization: Bearer <API Key>`）。仍设置着 `CRABLINK_TOKEN` 时 server 只在启动时打一条警告，照常启动；旧 token 当作未知 Bearer 值，得到 401（`unauthorized`），提示用 API Key 重新登录。(#55)
- **API 路径按 Workspace 划分**：`/api/links…` 移到 `/api/workspaces/{ws}/links…`（`{ws}` 是 Workspace 的 slug）。旧路径带旧 token 得到上面的 401，带 API Key 或 session 得到 404。这些路径接受该 Workspace 成员的 session cookie 或它的 API Key，其他凭据 403（`forbidden`）。(#55)
- **CLI 需要用 API Key 重新 `login`**：`crablink login` 改为输入 API Key，向 `/api/me` 问出它的 Workspace，与服务器地址一起存进配置；0.2 / 0.3 配置里的 `token` 不再生效。新增 `--api-key` / `--workspace` 参数和 `CRABLINK_API_KEY` / `CRABLINK_WORKSPACE` 环境变量。0.3 的 CLI 对 0.4 的 server 只会得到 401，提示重新登录。(#55)
- **`CRABLINK_BASE_URL` 的路径不再进 Short URL**：启动时用 `CRABLINK_BASE_URL` 的 host（和端口）建 Default Domain，Short URL 由 Link 的 Domain 拼出，是 `scheme://host/{code}`；`https://example.com/s` 这类带路径的地址，路径部分不再出现在 Short URL 里。(#55)

### 新功能

- **User 登录**：`POST /api/session` 用 email 和密码登录，设置 HttpOnly、Secure、SameSite=Lax 的 session cookie，30 天过期；`DELETE /api/session` 退出。带 cookie 的写请求必须带与 Host 一致的 `Origin`。`POST /api/me/password` 改密码，并结束该 User 的其他 session。密码用 argon2id 哈希，email 不区分大小写唯一。(#54)
- **`crablink-server admin create --email`**：建第一个 Instance Admin，密码从提示或 stdin 读入。(#54)
- **`GET /api/me`**：session 返回 `{ "user": … }`，API Key 返回 `{ "workspace": … }`；CLI 靠它认出 API Key 所属的 Workspace。(#54, #55)
- **Workspace 与 API Key**：每个 User 首次登录时得到一个个人 Workspace（自己是 Owner）。Owner 建、列出、吊销 API Key，`secret` 只在创建时返回一次，服务器只存哈希；API Key 属于 Workspace，创建者离开后照样可用。(#55)
- **成员与角色**：`GET /api/workspaces` 带上角色；新建、改名、删除 Workspace（删除后其 Link 变为已删除，Code 仍被占用，API Key 失效）；Owner 按 email 加成员、升降角色、移除成员，Member 可以退出。Member 只能管理 Link；最后一个 Owner 不能退出、被移除或降级（409 `last_owner`）。(#56)
- **邀请与 User 管理**：Instance Admin 用 `POST /api/admin/invites` 生成一次性邀请链接（7 天有效，可指定加入的 Workspace），被邀请人用 `POST /api/invites/accept` 设密码、建 User 并登录；`GET /api/admin/users` 列出 User，`PATCH /api/admin/users/{id}` 停用 / 启用或设为 Instance Admin，`POST /api/admin/users/{id}/password` 重置为随机密码（只返回一次）。停用或重置密码都会结束该 User 的所有 session。`/api/admin` 下除 Instance Admin 的 session 外一律 403。(#57)
- **Click**：记时间、Referrer 的 host、国家、设备类型和浏览器，不存 IP 和完整 User-Agent（[ADR-0006](docs/adr/0006-clicks-store-no-ip.md)）。 `.env` 读。仓库根目录的 `compose.yaml` 只用于本地试用。(#58)

## 0.3.0

没有破坏性改动：0.2 的 CLI 可以继续连 0.3 的 server，数据库和 `CRABLINK_TOKEN` 都不用动。

- **API 错误带上 `kind`**：管理 API 的错误 body 从 `{ "error": "..." }` 变为 `{ "kind": "...", "error": "..." }`，`kind` 是稳定的 snake_case 值（如 `code_taken`、`link_not_found`、`unauthorized`），脚本可以据此分支。HTTP status 和英文 message 不变。(#49)
- **CLI `--json`**：`crablink create`、`ls`、`get`、`edit` 加 `--json`，原样打印 API 的 Link JSON；`ls --json` 输出跨所有分页的一个数组。出错时仍只写 stderr。(#50)
- **QR 码**：`GET /api/links/{code}/qr?format=svg|png` 返回 Short URL 的 QR 码，已过期的 Link 返回 410（`link_expired`），不给 QR 码；`crablink qr <code>` 在终端显示，`-o file.png|file.svg` 存成文件；TUI 里选中 Link 按 `v` 弹出 QR 码。(#51)
