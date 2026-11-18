# Changelog

## 0.3.0

没有破坏性改动：0.2 的 CLI 可以继续连 0.3 的 server，数据库和 `CRABLINK_TOKEN` 都不用动。

- **API 错误带上 `kind`**：管理 API 的错误 body 从 `{ "error": "..." }` 变为 `{ "kind": "...", "error": "..." }`，`kind` 是稳定的 snake_case 值（如 `code_taken`、`link_not_found`、`unauthorized`），脚本可以据此分支。HTTP status 和英文 message 不变。(#49)
- **CLI `--json`**：`crablink create`、`ls`、`get`、`edit` 加 `--json`，原样打印 API 的 Link JSON；`ls --json` 输出跨所有分页的一个数组。出错时仍只写 stderr。(#50)
- **QR 码**：`GET /api/links/{code}/qr?format=svg|png` 返回 Short URL 的 QR 码，已过期的 Link 返回 410（`link_expired`），不给 QR 码；`crablink qr <code>` 在终端显示，`-o file.png|file.svg` 存成文件；TUI 里选中 Link 按 `v` 弹出 QR 码。(#51)
