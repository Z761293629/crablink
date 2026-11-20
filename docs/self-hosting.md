# 自托管

部署、配置、自定义 Domain、邀请 User、备份与升级。

## 本地试用（Docker Compose）

server 把数据存在 Postgres 里（[ADR-0007](adr/0007-postgres-only.md)）。仓库根目录的 `compose.yaml` 用于本地试用：从源码构建 server，同时起官方 `postgres:18` 镜像，数据库密码固定。对外部署请用下一节的 `deploy/`。

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

## 镜像

镜像公开发布在 `ghcr.io/z761293629/crablink-server`（`linux/amd64` 与 `linux/arm64`，Apple 芯片 Mac 原生运行），无需登录即可拉取。`latest` 跟随最新稳定版；生产环境建议钉住版本号，如 `ghcr.io/z761293629/crablink-server:0.6.0`，回滚只需换回旧版本号。`compose.yaml` 默认从源码构建（`build: .`），想用发布的镜像就把它换成 `image:`。已有 Postgres 的话也可以只跑这个镜像，用 `CRABLINK_DATABASE_URL` 指过去。

镜像以非 root 用户运行 `crablink-server`，自带 `HEALTHCHECK`（每 30 秒请求 `/health`），`docker ps` 能看到 `healthy`。缺少 `CRABLINK_BASE_URL` 或 `CRABLINK_DATABASE_URL` 时会打印 `crablink-server: CRABLINK_BASE_URL must be set and non-empty` 并以退出码 2 退出。

Web UI 在构建时打进 server 二进制（Docker 镜像和 Release 已包含）；从源码构建前先在 `web/` 里 `pnpm install && pnpm build`，没构建的话 server 照样能编译，`/app` 只显示一句说明。

## 环境变量

| 环境变量 | 必填 | 默认值 | 说明 |
|---|---|---|---|
| `CRABLINK_BASE_URL` | 是 | — | 对外地址，如 `https://crabl.ink`；启动时用它的 host（和端口）建 Default Domain，Short URL 是 `scheme://host/{code}`，路径部分不计入；跳转按请求的 `Host` 找 Domain，用别的地址访问得到 404 |
| `CRABLINK_DATABASE_URL` | 是 | compose 里已设好 | Postgres URL，如 `postgres://user:pass@host:5432/crablink`；启动时自动建表 |
| `CRABLINK_LISTEN` | 否 | `0.0.0.0:8080` | 监听地址（改端口的话 `ports` 和健康检查端口要一起改） |
| `CRABLINK_TRUST_PROXY` | 否 | `false` | `true` 时访客地址取 `X-Forwarded-For` 的第一项（只在反向代理之后这样设，`deploy/compose.yaml` 已设好），否则取 TCP 连接的对端地址 |
| `CRABLINK_COUNTRY_DB` | 否 | 镜像里已设好 | Click 查国家用的 DB-IP Lite country 数据库（`.mmdb`）；镜像构建时自带当月版本。不设则 Click 不记国家，设了却读不了就退出 |

Click 只记时间、Referrer 的 host、国家、设备类型和浏览器，从不存 IP 和完整 User-Agent（[ADR-0006](adr/0006-clicks-store-no-ip.md)）；爬虫和链接预览（Slack 等）照常跳转，但不算 Click。国家数据来自 [IP Geolocation by DB-IP](https://db-ip.com)（CC BY 4.0）。

## 生产部署（HTTPS）

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

`POSTGRES_PASSWORD` 只在 Postgres 卷第一次初始化时写入，之后改 `.env` 不会改掉库里的密码。升级时改 `compose.yaml` 里的镜像版本号，再 `docker compose pull && docker compose up -d`，schema 在启动时自动升级。备份见下方[备份与恢复](#备份与恢复)，命令在同一目录里执行；把备份文件复制到这台主机以外。

## 自定义 Domain

Workspace 可以用自己的 Domain，比如 `go.example.com`。Owner 在 Web UI 的 Domain 页或用 API 添加后它处于 pending，Instance Admin 批准后才会跳转、才能在上面建 Link、Caddy 才会为它申请证书；被拒绝或未批准的 Domain 一律 404。

1. Owner 把 `go.example.com` 的 CNAME 指向 Default Domain（或 A / AAAA 指向这台主机），然后添加：`POST /api/workspaces/{ws}/domains`，`{ "domain": "go.example.com" }`。
2. Instance Admin 在 Web UI 的实例管理页批准，或在 `GET /api/admin/domains` 里看到它，`PATCH /api/admin/domains/go.example.com` 设 `{ "status": "approved" }`（或 `rejected`）。
3. 在上面建 Link：`crablink create https://example.com/sale --code sale --domain go.example.com`。同一个 Code 可以同时存在于不同 Domain。

`deploy/Caddyfile` 用 Caddy 的 on-demand TLS：某个 host 第一次有 HTTPS 请求时，Caddy 先问 crablink 的 `GET /internal/domain-allowed?domain=<host>`，只有 Default Domain 和已批准的 Domain 得到 200，其余 404，所以别人把任意域名指过来也无法让这台主机去申请证书。crablink 自己从不处理 TLS（[ADR-0009](adr/0009-tls-terminated-by-reverse-proxy.md)）。

Owner 移除 Domain 后，它上面的 Link 变为已删除；之后无论哪个 Workspace 再添加这个 Domain，这些 Code 都仍被占用。删除 Workspace 也会释放它的 Domain。

## 邀请与管理 User

没有公开注册：Instance Admin 在 Web UI 的实例管理页生成邀请链接，发给对方；对方打开链接、设好密码就注册并登录了。也可以用 API 生成，`workspace`（Workspace slug）可省略，给了就让对方以 Member 加入这个 Workspace：

```bash
curl -c jar -H 'Origin: https://crabl.ink' -H 'Content-Type: application/json' \
  -d '{"email":"you@example.com","password":"…"}' https://crabl.ink/api/session
curl -b jar -H 'Origin: https://crabl.ink' -H 'Content-Type: application/json' \
  -d '{"email":"friend@example.com","workspace":"you"}' https://crabl.ink/api/admin/invites
# → {"url":"https://crabl.ink/app/invite#<token>","expires_at":"…"}
```

邀请 7 天有效、只能用一次。不用浏览器的话，取 `#` 后面的 token，自己设密码完成注册（同时登录）：

```bash
curl -c jar -H 'Content-Type: application/json' \
  -d '{"token":"<token>","password":"…"}' https://crabl.ink/api/invites/accept
```

Instance Admin 用 `/api/admin/users` 列出 User、停用 / 启用、设为 Instance Admin 或重置密码（见 [HTTP API](api.md)）；停用或重置密码会立即结束该 User 的所有 session。

## 备份与恢复

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

## 升级

每个版本的改动和升级说明见 [CHANGELOG](../CHANGELOG.md)。

- **从 0.4 升级**：换上新镜像重启即可，新表在启动时自动迁移，已有数据保留，CLI 不用重新 `login`。注意跳转现在要求 `Host` 是 Default Domain 或已批准的 Domain。
- **从 0.2 / 0.3 升级**：SQLite 数据不会迁移过来，新实例从空的 Postgres 开始，Link 需要重新创建；`CRABLINK_TOKEN` 已移除，程序和 CLI 改用 Workspace 的 API Key，旧 token 得到 401；管理 API 移到 `/api/workspaces/{ws}/…`，CLI 要升级并重新 `login`；`CRABLINK_BASE_URL` 里的路径不再出现在 Short URL 里。
