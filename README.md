<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="logo/masters/crablink-horizontal-reversed.svg">
    <img src="logo/masters/crablink-horizontal.svg" alt="crablink" width="360">
  </picture>
</p>

<p align="center">用 Rust 写的自托管短链服务：一个 server，一个 CLI，一个 SQLite 文件。</p>

---

> **状态：设计完成，尚未开始编码。** 完整规格见 [#1](https://github.com/Z761293629/crablink/issues/1)，下面的用法是目标形态，随 [Roadmap](#roadmap) 逐步兑现。

## 特性

- **Link 可改 Target**：海报印出去了也能改跳转地址，Short URL 不变。
- **Code 永不复用**：删除的 Link 保留墓碑，旧二维码不会被指向别处（[ADR-0002](docs/adr/0002-codes-are-never-reused.md)）。
- **随机 Code 或 Custom Code**：默认 7 位随机 base62，不可枚举；也可以自定义 `/rust-meetup`。
- **可选 Expiry**：到期返回 410；把 Expiry 改回未来即可恢复。
- **Click Count**：每个 Link 一个总点击数。
- **零运维**：单个二进制 + 单个 SQLite 文件，备份就是复制文件（[ADR-0001](docs/adr/0001-sqlite-for-storage.md)）。

## 快速开始

### Server

```bash
docker run -d -p 8080:8080 -v crablink:/data \
  -e CRABLINK_BASE_URL=https://crab.link \
  -e CRABLINK_TOKEN=change-me \
  -e CRABLINK_DATABASE_URL=sqlite:///data/crablink.db \
  crablink-server
```

| 环境变量 | 必填 | 默认值 | 说明 |
|---|---|---|---|
| `CRABLINK_BASE_URL` | 是 | — | 对外域名，用于拼 Short URL |
| `CRABLINK_TOKEN` | 是 | — | 管理 API 的 Bearer token |
| `CRABLINK_DATABASE_URL` | 否 | 当前目录 `crablink.db` | SQLite 路径 |
| `CRABLINK_LISTEN` | 否 | `0.0.0.0:8080` | 监听地址 |

### CLI

```bash
crablink login --server https://crab.link   # 提示输入 token，存到 ~/.config/crablink/config.toml
crablink create https://example.com/a/very/long/path
crablink create https://rustconf.com --code rust-meetup --expires 7d
crablink ls
crablink edit rust-meetup --target https://rustconf.com/2027
crablink rm rust-meetup
```

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

- [ ] **M1 跳转跑通**：workspace 骨架、数据库迁移、token 鉴权、创建 Link（随机 / Custom Code）、`GET /{code}`
- [ ] **M2 Link 生命周期**：查看、修改、删除、分页列表、Expiry 与 410、Click Count
- [ ] **M3 CLI**：`login` `create` `ls` `get` `edit` `rm`
- [ ] **M4 交付**：Dockerfile、部署文档

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
