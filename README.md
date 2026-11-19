<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="logo/masters/crablink-horizontal-reversed.svg">
    <img src="logo/masters/crablink-horizontal.svg" alt="crablink" width="360">
  </picture>
</p>

<p align="center">用 Rust 写的自托管短链服务：一个 server，一个 CLI，一个 Postgres。</p>

---

> **状态：server 与 CLI 已实现，0.4 起支持多用户与 Workspace，Web UI 尚未上线。** 规格见 [#1](https://github.com/Z761293629/crablink/issues/1) 与 [#47](https://github.com/Z761293629/crablink/issues/47)，打版本 tag 后 server 镜像自动发布到 GHCR，CLI 预编译二进制附在 GitHub Release 上。

## 特性

- **Link 可改 Target**：海报印出去了也能改跳转地址，Short URL 不变。
- **Code 永不复用**：删除的 Link 保留墓碑，旧二维码不会被指向别处（[ADR-0002](docs/adr/0002-codes-are-never-reused.md)）。
- **随机 Code 或 Custom Code**：默认 7 位随机 base62，不可枚举；也可以自定义 `/rust-meetup`。
- **可选 Expiry**：到期返回 410；把 Expiry 改回未来即可恢复。
- **Click Count**：每个 Link 一个总点击数。
- **多用户与 Workspace**：Link 属于 Workspace，成员分 Owner / Member；Instance Admin 邀请 User；程序用 Workspace 的 API Key（[ADR-0005](docs/adr/0005-workspaces-own-links-on-domains.md)）。
- **自托管**：`docker compose up` 起 server + Postgres（[ADR-0007](docs/adr/0007-postgres-only.md)）。

## 快速开始

### Server（Docker Compose）

server 把数据存在 Postgres 里（[ADR-0007](docs/adr/0007-postgres-only.md)）。仓库根目录的 `compose.yaml` 用于本地试用：从源码构建 server，同时起官方 `postgres:18` 镜像，数据库密码固定。对外部署请用下一节的 `deploy/`。

```bash
docker compose up -d
curl http://localhost:8080/health   # → ok
```

再建第一个 Instance Admin（密码从提示或 stdin 读入，至少 8 位）：

```bash
docker compose exec crablink crablink-server admin create --email you@example.com
```

server 只讲 HTTP，对外必须在前面放一个终止 HTTPS 的反向代理：session cookie 带 `Secure`，浏览器只经 HTTPS（或 localhost）发回它。

Postgres 只在 compose 网络内可达，数据在卷 `postgres` 里，删掉或重建容器数据都还在。

镜像公开发布在 `ghcr.io/z761293629/crablink-server`（`linux/amd64` 与 `linux/arm64`，Apple 芯片 Mac 原生运行），无需登录即可拉取。`latest` 跟随最新稳定版；生产环境建议钉住版本号，如 `ghcr.io/z761293629/crablink-server:0.4.0`，回滚只需换回旧版本号。`compose.yaml` 默认从源码构建（`build: .`），想用发布的镜像就把它换成 `image:`。已有 Postgres 的话也可以只跑这个镜像，用 `CRABLINK_DATABASE_URL` 指过去。

镜像以非 root 用户运行 `crablink-server`，自带 `HEALTHCHECK`（每 30 秒请求 `/health`），`docker ps` 能看到 `healthy`。缺少 `CRABLINK_BASE_URL` 或 `CRABLINK_DATABASE_URL` 时会打印 `crablink-server: CRABLINK_BASE_URL must be set and non-empty` 并以退出码 2 退出。

| 环境变量 | 必填 | 默认值 | 说明 |
|---|---|---|---|
| `CRABLINK_BASE_URL` | 是 | — | 对外地址，如 `https://crabl.ink`；启动时用它的 host（和端口）建 Default Domain，Short URL 是 `scheme://host/{code}`，路径部分不计入 |
| `CRABLINK_DATABASE_URL` | 是 | compose 里已设好 | Postgres URL，如 `postgres://user:pass@host:5432/crablink`；启动时自动建表 |
| `CRABLINK_LISTEN` | 否 | `0.0.0.0:8080` | 监听地址（改端口的话 `ports` 和健康检查端口要一起改） |
| `CRABLINK_TRUST_PROXY` | 否 | `false` | `true` 时访客地址取 `X-Forwarded-For` 的第一项（只在反向代理之后这样设，`deploy/compose.yaml` 已设好），否则取 TCP 连接的对端地址 |
| `CRABLINK_COUNTRY_DB` | 否 | 镜像里已设好 | Click 查国家用的 DB-IP Lite country 数据库（`.mmdb`）；镜像构建时自带当月版本。不设则 Click 不记国家，设了却读不了就退出 |

Click 只记时间、Referrer 的 host、国家、设备类型和浏览器，从不存 IP 和完整 User-Agent（[ADR-0006](docs/adr/0006-clicks-store-no-ip.md)）；爬虫和链接预览（Slack 等）照常跳转，但不算 Click。国家数据来自 [IP Geolocation by DB-IP](https://db-ip.com)（CC BY 4.0）。

### 生产部署（HTTPS）

`deploy/` 里是对外部署用的 `compose.yaml` 和 `Caddyfile`：Caddy 自动申请、续期 Let's Encrypt 证书并反代到 server；server 和 Postgres 都不对外开端口，只有 Caddy 监听 80 / 443；镜像钉在发布的版本号上；域名和数据库密码从 `.env` 读，缺了就拒绝启动。

1. 准备一台装好 Docker 的 Linux 主机，放行 80 和 443（在中国大陆的主机对外提供服务需先 ICP 备案）。
2. 把域名的 A（和 AAAA）记录指向这台主机。
3. 把 `deploy/compose.yaml` 和 `deploy/Caddyfile` 放到主机上同一个目录，在里面生成 `.env` 并启动：

```bash
printf 'CRABLINK_DOMAIN=crabl.ink\nPOSTGRES_PASSWORD=%s\n' "$(openssl rand -hex 24)" > .env
chmod 600 .env
docker compose up -d
curl https://crabl.ink/health   # → ok
docker compose exec crablink crablink-server admin create --email you@example.com
```

`POSTGRES_PASSWORD` 只在 Postgres 卷第一次初始化时写入，之后改 `.env` 不会改掉库里的密码。升级时改 `compose.yaml` 里的镜像版本号，再 `docker compose pull && docker compose up -d`，schema 在启动时自动升级。备份见下方「备份与恢复」，命令在同一目录里执行；把备份文件复制到这台主机以外。

### 邀请与管理 User

没有公开注册：Instance Admin 登录后生成邀请链接，发给对方。`workspace` 可省略，给了就让对方以 Member 加入这个 Workspace：

```bash
curl -c jar -H 'Origin: https://crabl.ink' -H 'Content-Type: application/json' \
  -d '{"email":"you@example.com","password":"…"}' https://crabl.ink/api/session
curl -b jar -H 'Origin: https://crabl.ink' -H 'Content-Type: application/json' \
  -d '{"email":"friend@example.com","workspace":"you"}' https://crabl.ink/api/admin/invites
# → {"url":"https://crabl.ink/app/invite#<token>","expires_at":"…"}
```

邀请 7 天有效、只能用一次。Web UI 上线前，对方取 `#` 后面的 token，自己设密码完成注册（同时登录）：

```bash
curl -c jar -H 'Content-Type: application/json' \
  -d '{"token":"<token>","password":"…"}' https://crabl.ink/api/invites/accept
```

Instance Admin 用 `/api/admin/users` 列出 User、停用 / 启用、设为 Instance Admin 或重置密码（见下方 HTTP API）；停用或重置密码会立即结束该 User 的所有 session。

### 备份与恢复

备份用 `pg_dump`，不用停服务：

```bash
docker compose exec -T postgres pg_dump -U crablink crablink > crablink.sql
```

恢复到一个空库：先停 server，重建数据库，导入，再启动：

```bash
docker compose stop crablink
docker compose exec postgres dropdb -U crablink crablink
docker compose exec postgres createdb -U crablink crablink
docker compose exec -T postgres psql -U crablink -d crablink -v ON_ERROR_STOP=1 < crablink.sql
docker compose start crablink
```

### 从 0.2 / 0.3 升级

0.4 的破坏性改动见 [CHANGELOG](CHANGELOG.md)。简言之：

- SQLite 数据不会迁移过来，新实例从空的 Postgres 开始，Link 需要重新创建。
- `CRABLINK_TOKEN` 已移除，仍设置着只会在启动时打一条警告；程序和 CLI 改用 Workspace 的 API Key（见下），旧 token 得到 401，提示用 API Key 重新登录。
- 管理 API 移到 `/api/workspaces/{ws}/…`；CLI 要升级到 0.4 并重新 `login`。
- `CRABLINK_BASE_URL` 里的路径不再出现在 Short URL 里。

### CLI

macOS（Apple 芯片）和 Linux 上用 Homebrew 安装，`brew upgrade` 升级，`brew uninstall crablink` 卸载：

```bash
brew install Z761293629/tap/crablink
```

不用 Homebrew 的话，从 [Releases](https://github.com/Z761293629/crablink/releases) 下载预编译二进制，无需安装 Rust。提供 `aarch64-apple-darwin`、`x86_64-unknown-linux-musl`、`aarch64-unknown-linux-musl`（Linux 版为静态链接），均未签名、未公证：

```bash
target=aarch64-apple-darwin   # 或 x86_64-unknown-linux-musl / aarch64-unknown-linux-musl
curl -fsSL "https://github.com/Z761293629/crablink/releases/latest/download/crablink-$target.tar.gz" | tar xz
sudo mv crablink /usr/local/bin/
crablink --version
```

macOS 上用 `curl` 下载不会带隔离属性，因此不会触发 Gatekeeper 拦截；用浏览器下载的话需先 `xattr -d com.apple.quarantine crablink`。也可以从源码安装：`cargo install --path crates/cli`。

CLI 用 Workspace 的 API Key 登录。每个 User 首次登录时会得到一个个人 Workspace（自己是 Owner，slug 取自 email `@` 前的部分）；在 Web UI 上线前，用 curl 登录并建一个 API Key：

```bash
curl -c jar -H 'Origin: https://crabl.ink' -H 'Content-Type: application/json' \
  -d '{"email":"you@example.com","password":"…"}' https://crabl.ink/api/session
curl -b jar https://crabl.ink/api/workspaces                      # → [{"slug":"you",…}]
curl -b jar -H 'Origin: https://crabl.ink' -H 'Content-Type: application/json' \
  -d '{"name":"laptop"}' https://crabl.ink/api/workspaces/you/api-keys   # secret 只显示这一次
```

```bash
crablink login --server https://crabl.ink   # 提示输入 API Key，向服务器问出它的 Workspace，存到 ~/.config/crablink/config.toml
crablink create https://example.com/a/very/long/path
crablink create https://rustconf.com --code rust-meetup --expires 7d
crablink ls                                  # 全部 Link：Code、Click Count、Expiry、Target
crablink get rust-meetup
crablink edit rust-meetup --target https://rustconf.com/2027
crablink edit rust-meetup --no-expires       # 或 --expires 30d / RFC 3339，二者互斥
crablink rm rust-meetup
crablink ls --json | jq -r '.[].short_url'   # create / ls / get / edit 加 --json 打印 API 原样的 JSON
crablink qr rust-meetup                      # 在终端里显示 Short URL 的 QR 码，可直接扫
crablink qr rust-meetup -o poster.png        # 存为文件，格式看扩展名：.png 或 .svg
```

`--json` 输出与 HTTP API 的响应格式一致（`ls` 为跨所有分页的一个数组，新的在前），API 变它就跟着变；出错时仍只在 stderr 打印错误并以非零码退出，stdout 不输出 JSON。

服务器地址、API Key 和 Workspace 按字段逐个取，优先级从高到低：`--server` / `--api-key` / `--workspace` 参数 > 环境变量 `CRABLINK_URL` / `CRABLINK_API_KEY` / `CRABLINK_WORKSPACE` > `~/.config/crablink/config.toml`。没给 Workspace 时向 `/api/me` 问 API Key 所属的 Workspace。`login` 先用 API Key 请求 `/api/me`，通过后才把服务器、API Key 和 Workspace 写进配置文件，权限为 `0600`（仅本人可读写）。非终端环境下 `login` 从 stdin 读 API Key：`echo "$CRABLINK_API_KEY" | crablink login --server https://crabl.ink`。0.2 / 0.3 的配置文件里的 `token` 不再生效，需重新 `login`。

不带参数运行 `crablink` 会在终端里打开全屏 TUI 浏览 Link（stdout 不是终端时只打印帮助）。左侧边栏顶部通过终端图像协议显示 crablink logo，并按终端背景自动选浅色/深色版本：Ghostty、kitty、WezTerm、iTerm2 及其他支持 kitty / iTerm2 / sixel 图像协议的终端会显示；macOS Terminal.app 等不支持图像协议的终端不显示 logo，其余照常可用。TUI 运行中切换终端主题，logo 约 1 秒内跟着换版本，按 `r` 可立即重新检测（tmux 内不跟随）。在选中的 Link 上按 `v` 弹出它的 QR 码（白底黑码，与终端主题无关），`Esc` 关闭；终端放不下时显示所需的最小尺寸。

### HTTP API

Link 属于 Workspace，都在 Default Domain 上；Code 在其 Domain 内唯一，删除后也不再复用。`{ws}` 是 Workspace 的 slug，`/api/workspaces/{ws}/links…` 接受该 Workspace 成员的 session cookie 或它的 API Key，其他凭据一律 403（`forbidden`）。

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/{code}` | 302 跳转；不存在 404；已删除或已过期 410 |
| `POST` | `/api/workspaces/{ws}/links` | 创建 Link：`{ target, code?, expires_at? }`；返回的 Link 带 `domain` 和 `short_url` |
| `GET` | `/api/workspaces/{ws}/links` | 列表，新的在前，游标分页 |
| `GET` | `/api/workspaces/{ws}/links/{code}` | 查看单个 Link |
| `PATCH` | `/api/workspaces/{ws}/links/{code}` | 修改 `target` / `expires_at` |
| `DELETE` | `/api/workspaces/{ws}/links/{code}` | 删除（不可恢复） |
| `GET` | `/api/workspaces/{ws}/links/{code}/qr?format=svg\|png` | Short URL 的 QR 码，默认 SVG；已过期 410（`link_expired`） |
| `GET` | `/api/workspaces/{ws}/links/{code}/clicks?from=&to=&interval=hour\|day` | `[from, to)` 内的 Click：`series` 按 UTC 小时 / 天分桶（默认天，空桶也列出，最多 1000 个），`referrers` / `countries` / `devices` / `browsers` 为 `[{ value, count }]`，多的在前，未知为 `null` |
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
| `PATCH` | `/api/admin/users/{id}` | Instance Admin 改 `disabled` / `instance_admin`，不能对自己停用或撤销；停用即结束其 session |
| `POST` | `/api/admin/users/{id}/password` | Instance Admin 重置为随机密码：`{ password }` 只返回这一次，并结束其 session |

Member 只能管理 Link；成员、API Key 和 Workspace 设置只归 Owner，Member 或 API Key 去做得到 403（`forbidden`）。`/api/admin` 只接受 Instance Admin 的 session，其他凭据 403。最后一个 Owner 不能退出、被移除或降级，得到 409（`last_owner`）。

程序用 `Authorization: Bearer <API Key>`；API Key 属于 Workspace，创建它的人离开后照样可用。带 session cookie 的写请求必须带与 Host 一致的 `Origin`。未知或已吊销的 Bearer 值（包括 0.2 / 0.3 的 token）得到 401，提示用 API Key 重新登录。

## Roadmap

- [x] **M1 跳转跑通**：workspace 骨架、数据库迁移、token 鉴权、创建 Link（随机 / Custom Code）、`GET /{code}`、CI（PR 自动跑 fmt / clippy / test）
- [x] **M2 Link 生命周期**：查看、修改、删除、分页列表、Expiry 与 410、Click Count
- [x] **M3 CLI**：`login` `create` `ls` `get` `edit` `rm`
- [x] **M4 交付**：Dockerfile、部署文档、Release workflow（打 `v*` tag 自动创建 GitHub Release、发布 GHCR 镜像、附上 CLI 预编译二进制）
- [x] **Homebrew tap**：发布稳定版时自动更新 tap 里的 formula，`brew upgrade crablink` 即可升级
- [x] **0.3**：API 错误带 `kind`、CLI `--json`、QR 码
- [x] **0.4 多用户地基**：Postgres、User 登录、Workspace 与 API Key、成员与角色、邀请与 User 管理（[#47](https://github.com/Z761293629/crablink/issues/47)）

接下来做什么见 [Issues](https://github.com/Z761293629/crablink/issues)。

## 文档

- [GLOSSARY.md](GLOSSARY.md)：领域术语（Link、Code、Target……）
- [docs/adr/](docs/adr/)：架构决策记录
- [logo/GUIDELINES.md](logo/GUIDELINES.md)：Logo 使用规范

<p align="center">
  <img src="logo/masters/crablink-symbol.svg" alt="" width="48">
</p>
