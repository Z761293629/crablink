# Web UI

Web UI 由 server 自己在 `/app` 提供（如 `https://crabl.ink/app`），与 API 同源，不用另外部署（[ADR-0008](adr/0008-spa-embedded-in-server.md)）；用 email 和密码登录，session cookie 30 天有效。界面有英文和简体中文，默认跟随浏览器语言，可以在右上角的菜单里切换并记在本浏览器里；跟随系统的浅色 / 深色主题，手机上也能用。

- **Link**：Link 页是一块航班信息屏，选中的 Link 显示为登机牌，下方是它最近 7 天的 Click 翻牌屏。创建（Generated 或 Custom Code、Domain、Expiry、Tag）、改 Target / Expiry / Tag、确认后删除、一键复制 Short URL、下载 SVG 或 PNG 二维码；按 Tag 过滤，按 Code 或 Target 搜索，分页加载。
- **CSV**：在 Link 页导出 `links.csv`，或选一个同格式的文件导入，逐行显示建好的 Short URL 或跳过的原因。
- **Click**：每个 Link 的 Click 图表（最近 24 小时按小时，7 / 30 / 90 天按天）和 Referrer、国家、设备、浏览器分布；Workspace Clicks 页看整个 Workspace 的趋势和排名前 10 的 Link。已删除 Link 的 Click 仍计入 Workspace 的总数。
- **Workspace**：切换、新建、改名、删除 Workspace；Owner 管理成员和 API Key（`secret` 只显示一次），添加自定义 Domain 并看到它的审批状态和 DNS 指向。
- **实例管理**：Instance Admin 生成邀请链接、批准或拒绝自定义 Domain、停用 / 启用 User、设为或撤销 Instance Admin、重置密码；最后一个启用的 Instance Admin 不能被停用或撤销。

## 开发

源码在 `web/`（React + TypeScript + Vite）。包管理用 pnpm：

```bash
cd web
pnpm install
pnpm server  # localhost:8080 上的 server，用 embedded Postgres，不用 Docker；登录 admin@example.com / e2e admin password（pnpm dev 的登录表单已预填），Ctrl-C 后数据清空
pnpm dev     # 另开一个终端：Vite 开发服务器（localhost:5173/app/），改前端热更新，/api 代理到 pnpm server
pnpm build   # 输出 web/dist，下次编译 server 时打进二进制
pnpm e2e     # Playwright 端到端测试：先 pnpm build，它会起 embedded Postgres 和真实的 crablink-server
```
