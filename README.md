<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="logo/masters/crablink-horizontal-reversed.svg">
    <img src="logo/masters/crablink-horizontal.svg" alt="crablink" width="360">
  </picture>
</p>

<p align="center">用 Rust 写的自托管短链服务：一个 server（内置 Web UI），一个 CLI，一个 Postgres。</p>

---

> **状态：server、CLI 与 Web UI 已实现，0.5 起支持自定义 Domain、Click 图表、Tag 和 CSV。** 规格见 [#1](https://github.com/Z761293629/crablink/issues/1) 与 [#47](https://github.com/Z761293629/crablink/issues/47)，打版本 tag 后 server 镜像自动发布到 GHCR，CLI 预编译二进制附在 GitHub Release 上。

## 特性

- **Link 可改 Target**：海报印出去了也能改跳转地址，Short URL 不变。
- **Code 永不复用**：删除的 Link 保留墓碑，旧二维码不会被指向别处（[ADR-0002](docs/adr/0002-codes-are-never-reused.md)）。
- **随机 Code 或 Custom Code**：默认 7 位随机 base62，不可枚举；也可以自定义 `/rust-meetup`。
- **可选 Expiry**：到期返回 410；把 Expiry 改回未来即可恢复。
- **Click 与图表**：每次真人跳转记一个 Click（不存 IP），按小时 / 天看趋势，按 Referrer、国家、设备、浏览器看分布，也能看整个 Workspace 的 Click 和排名靠前的 Link。
- **Tag 与搜索**：给 Link 打 Tag，按 Tag 过滤，按 Code 或 Target 搜索。
- **自定义 Domain**：Workspace 可以用自己的 Domain，如 `go.example.com`，Instance Admin 批准后生效。
- **CSV 导入导出**：导出 Workspace 的全部 Link，或从 CSV 批量创建。
- **Web UI**：同一个 server 在 `/app` 提供中英文界面，适配手机和深色模式，覆盖 CLI 能做的一切，外加 Workspace、成员、Domain、API Key 管理和 Click 图表。
- **多用户与 Workspace**：Link 属于 Workspace，成员分 Owner / Member；Instance Admin 邀请 User；程序用 Workspace 的 API Key（[ADR-0005](docs/adr/0005-workspaces-own-links-on-domains.md)）。
- **自托管**：`docker compose up` 起 server + Postgres（[ADR-0007](docs/adr/0007-postgres-only.md)）。

## 快速开始

```bash
docker compose up -d                       # server + Postgres，见 docs/self-hosting.md
docker compose exec crablink crablink-server admin create --email you@example.com
open http://localhost:8080/app             # 用刚才的 email 和密码登录 Web UI
```

命令行用户：

```bash
brew install Z761293629/tap/crablink
crablink login --server http://localhost:8080   # 用 Web UI 里建的 API Key 登录
crablink create https://example.com/a/very/long/path
```

对外部署（HTTPS、自定义 Domain、备份）见 [docs/self-hosting.md](docs/self-hosting.md)。

## Roadmap

- [x] **M1 跳转跑通**：workspace 骨架、数据库迁移、token 鉴权、创建 Link（随机 / Custom Code）、`GET /{code}`、CI（PR 自动跑 fmt / clippy / test）
- [x] **M2 Link 生命周期**：查看、修改、删除、分页列表、Expiry 与 410、Click Count
- [x] **M3 CLI**：`login` `create` `ls` `get` `edit` `rm`
- [x] **M4 交付**：Dockerfile、部署文档、Release workflow（打 `v*` tag 自动创建 GitHub Release、发布 GHCR 镜像、附上 CLI 预编译二进制）
- [x] **Homebrew tap**：发布稳定版时自动更新 tap 里的 formula，`brew upgrade crablink` 即可升级
- [x] **0.3**：API 错误带 `kind`、CLI `--json`、QR 码
- [x] **0.4 多用户地基**：Postgres、User 登录、Workspace 与 API Key、成员与角色、邀请与 User 管理（[#47](https://github.com/Z761293629/crablink/issues/47)）
- [x] **0.5 团队日常使用**：自定义 Domain、Web UI、Click 图表、Tag 与搜索、CSV 导入导出（[#47](https://github.com/Z761293629/crablink/issues/47)）
- [x] **0.6 Web UI 改版**：登机牌和航班信息屏的视觉、每个 Link 最近 7 天的 Click 翻牌屏、右上角统一菜单（[#72](https://github.com/Z761293629/crablink/pull/72)）

接下来做什么见 [Issues](https://github.com/Z761293629/crablink/issues)。

## 文档

- [docs/self-hosting.md](docs/self-hosting.md)：部署、环境变量、HTTPS、自定义 Domain、邀请 User、备份与升级
- [docs/web-ui.md](docs/web-ui.md)：Web UI 能做什么，以及前端开发
- [docs/cli.md](docs/cli.md)：CLI 安装、登录、命令和 TUI
- [docs/api.md](docs/api.md)：HTTP API、权限和错误 `kind`
- [CHANGELOG.md](CHANGELOG.md)：版本改动与升级说明
- [GLOSSARY.md](GLOSSARY.md)：领域术语（Link、Code、Target……）
- [docs/adr/](docs/adr/)：架构决策记录
- [logo/GUIDELINES.md](logo/GUIDELINES.md)：Logo 使用规范

<p align="center">
  <img src="logo/masters/crablink-symbol.svg" alt="" width="48">
</p>
