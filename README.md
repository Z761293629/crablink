<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="logo/masters/crablink-horizontal-reversed.svg">
    <img src="logo/masters/crablink-horizontal.svg" alt="crablink" width="360">
  </picture>
</p>

<p align="center">用 Rust 写的自托管短链服务：一个 server，一个 CLI，一个 SQLite 文件。</p>

---

> **状态：server 与 CLI 已实现。** 完整规格见 [#1](https://github.com/Z761293629/crablink/issues/1)，打版本 tag 后 server 镜像自动发布到 GHCR，CLI 预编译二进制附在 GitHub Release 上。

## 特性

- **Link 可改 Target**：海报印出去了也能改跳转地址，Short URL 不变。
- **Code 永不复用**：删除的 Link 保留墓碑，旧二维码不会被指向别处（[ADR-0002](docs/adr/0002-codes-are-never-reused.md)）。
- **随机 Code 或 Custom Code**：默认 7 位随机 base62，不可枚举；也可以自定义 `/rust-meetup`。
- **可选 Expiry**：到期返回 410；把 Expiry 改回未来即可恢复。
- **Click Count**：每个 Link 一个总点击数。
- **零运维**：单个二进制 + 单个 SQLite 文件，备份就是复制文件（[ADR-0001](docs/adr/0001-sqlite-for-storage.md)）。

## 快速开始

### Server（Docker）

```bash
docker run -d --name crablink --restart unless-stopped \
  -p 8080:8080 -v crablink:/data \
  -e CRABLINK_BASE_URL=https://crab.link \
  -e CRABLINK_TOKEN=change-me \
  ghcr.io/z761293629/crablink-server
curl http://localhost:8080/health   # → ok
```

镜像公开发布在 `ghcr.io/z761293629/crablink-server`（仅 `linux/amd64`），无需登录即可拉取。`latest` 跟随最新稳定版；生产环境建议钉住版本号，如 `ghcr.io/z761293629/crablink-server:0.1.0`，回滚只需换回旧版本号。也可以从源码构建：`docker build -t crablink-server .`。

镜像以非 root 用户运行 `crablink-server`，数据库在卷 `/data/crablink.db` 里，删掉或重建容器数据都还在。镜像自带 `HEALTHCHECK`（每 30 秒请求 `/health`），`docker ps` 能看到 `healthy`。缺少 `CRABLINK_BASE_URL` 或 `CRABLINK_TOKEN` 时容器会打印 `crablink-server: CRABLINK_TOKEN must be set and non-empty` 并以退出码 2 退出（`docker logs crablink` 查看）。

| 环境变量 | 必填 | 默认值 | 说明 |
|---|---|---|---|
| `CRABLINK_BASE_URL` | 是 | — | 对外域名，用于拼 Short URL |
| `CRABLINK_TOKEN` | 是 | — | 管理 API 的 Bearer token |
| `CRABLINK_DATABASE_URL` | 否 | 镜像内 `sqlite:///data/crablink.db`；直接运行二进制时为当前目录 `sqlite:crablink.db` | SQLite 路径，不存在会自动创建 |
| `CRABLINK_LISTEN` | 否 | `0.0.0.0:8080` | 监听地址（改端口的话 `-p` 和健康检查端口要一起改） |

**备份**：数据库开启了 WAL，最近的写入可能还在 `crablink.db-wal` 里。先停容器，再把 `crablink.db*` 一起复制出来：

```bash
docker stop crablink
docker run --rm -v crablink:/data -v "$PWD/backup":/backup alpine sh -c 'cp /data/crablink.db* /backup/'
docker start crablink
```

恢复时停掉容器，把这些文件复制回卷里再启动。

### CLI

从 [Releases](https://github.com/Z761293629/crablink/releases) 下载预编译二进制，无需安装 Rust。提供 `aarch64-apple-darwin`、`x86_64-unknown-linux-musl`、`aarch64-unknown-linux-musl`（Linux 版为静态链接），均未签名、未公证：

```bash
target=aarch64-apple-darwin   # 或 x86_64-unknown-linux-musl / aarch64-unknown-linux-musl
curl -fsSL "https://github.com/Z761293629/crablink/releases/latest/download/crablink-$target.tar.gz" | tar xz
sudo mv crablink /usr/local/bin/
crablink --version
```

macOS 上用 `curl` 下载不会带隔离属性，因此不会触发 Gatekeeper 拦截；用浏览器下载的话需先 `xattr -d com.apple.quarantine crablink`。也可以从源码安装：`cargo install --path crates/cli`。

```bash
crablink login --server https://crab.link   # 提示输入 token，存到 ~/.config/crablink/config.toml
crablink create https://example.com/a/very/long/path
crablink create https://rustconf.com --code rust-meetup --expires 7d
crablink ls                                  # 全部 Link：Code、Click Count、Expiry、Target
crablink get rust-meetup
crablink edit rust-meetup --target https://rustconf.com/2027
crablink edit rust-meetup --no-expires       # 或 --expires 30d / RFC 3339，二者互斥
crablink rm rust-meetup
```

服务器地址和 token 按字段逐个取，优先级从高到低：`--server` / `--token` 参数 > 环境变量 `CRABLINK_URL` / `CRABLINK_TOKEN` > `~/.config/crablink/config.toml`。`login` 先用 token 请求一次服务器，通过后才写配置文件，权限为 `0600`（仅本人可读写）。非终端环境下 `login` 从 stdin 读 token：`echo "$TOKEN" | crablink login --server https://crab.link`。

不带参数运行 `crablink` 会在终端里打开全屏 TUI 浏览 Link（stdout 不是终端时只打印帮助）。左侧边栏顶部通过终端图像协议显示 crablink logo，并按终端背景自动选浅色/深色版本：Ghostty、kitty、WezTerm、iTerm2 及其他支持 kitty / iTerm2 / sixel 图像协议的终端会显示；macOS Terminal.app 等不支持图像协议的终端不显示 logo，其余照常可用。

### HTTP API

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/{code}` | 302 跳转；不存在 404；已删除或已过期 410 |
| `POST` | `/api/links` | 创建 Link：`{ target, code?, expires_at? }` |
| `GET` | `/api/links` | 列表，新的在前，游标分页 |
| `GET` | `/api/links/{code}` | 查看单个 Link |
| `PATCH` | `/api/links/{code}` | 修改 `target` / `expires_at` |
| `DELETE` | `/api/links/{code}` | 删除（不可恢复） |

`/api/*` 需要 `Authorization: Bearer <token>`。

## Roadmap

- [x] **M1 跳转跑通**：workspace 骨架、数据库迁移、token 鉴权、创建 Link（随机 / Custom Code）、`GET /{code}`、CI（PR 自动跑 fmt / clippy / test）
- [x] **M2 Link 生命周期**：查看、修改、删除、分页列表、Expiry 与 410、Click Count
- [x] **M3 CLI**：`login` `create` `ls` `get` `edit` `rm`
- [x] **M4 交付**：Dockerfile、部署文档、Release workflow（打 `v*` tag 自动创建 GitHub Release、发布 GHCR 镜像、附上 CLI 预编译二进制）

**以后再说**（有真实需求再做）：

- 多用户 / 公网开放注册
- Postgres 与多实例部署
- 点击明细统计（时间、来源、地区）
- Web 管理界面

## 文档

- [GLOSSARY.md](GLOSSARY.md)：领域术语（Link、Code、Target……）
- [docs/adr/](docs/adr/)：架构决策记录
- [logo/GUIDELINES.md](logo/GUIDELINES.md)：Logo 使用规范

<p align="center">
  <img src="logo/masters/crablink-symbol.svg" alt="" width="48">
</p>
