# CLI

## 安装

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

## 登录

CLI 用 Workspace 的 API Key 登录。每个 User 首次登录时会得到一个个人 Workspace（自己是 Owner，Workspace slug 取自 email `@` 前的部分）。在 Web UI 的 Manage Workspace 页建一个 API Key，或者用 curl：

```bash
curl -c jar -H 'Origin: https://crabl.ink' -H 'Content-Type: application/json' \
  -d '{"email":"you@example.com","password":"…"}' https://crabl.ink/api/session
curl -b jar https://crabl.ink/api/workspaces                      # → [{"slug":"you",…}]
curl -b jar -H 'Origin: https://crabl.ink' -H 'Content-Type: application/json' \
  -d '{"name":"laptop"}' https://crabl.ink/api/workspaces/you/api-keys   # secret 只显示这一次
```

```bash
crablink login --server https://crabl.ink   # 提示输入 API Key，向服务器问出它的 Workspace，存到 ~/.config/crablink/config.toml
```

服务器地址、API Key 和 Workspace 按字段逐个取，优先级从高到低：`--server` / `--api-key` / `--workspace` 参数 > 环境变量 `CRABLINK_URL` / `CRABLINK_API_KEY` / `CRABLINK_WORKSPACE` > `~/.config/crablink/config.toml`。没给 Workspace 时向 `/api/me` 问 API Key 所属的 Workspace。`login` 先用 API Key 请求 `/api/me`，通过后才把服务器、API Key 和 Workspace 写进配置文件，权限为 `0600`（仅本人可读写）。非终端环境下 `login` 从 stdin 读 API Key：`echo "$CRABLINK_API_KEY" | crablink login --server https://crabl.ink`。0.2 / 0.3 的配置文件里的 `token` 不再生效，需重新 `login`。

## 命令

```bash
crablink create https://example.com/a/very/long/path
crablink create https://rustconf.com --code rust-meetup --expires 7d
crablink create https://example.com/sale --domain go.example.com   # 建在 Workspace 已批准的自定义 Domain 上
crablink create https://example.com/spring --tag promo --tag spring  # --tag 可重复
crablink rm sale --domain go.example.com     # get / edit / rm / qr 同样用 --domain 指定 Domain，默认 Default Domain
crablink ls                                  # 全部 Link：Short URL（Blocked Link 标 (blocked)，只能 rm）、Click Count、Expiry、Target
crablink ls --tag promo --search spring      # 只列带这个 Tag、Code 或 Target 含这段文字（不区分大小写）的 Link
crablink get rust-meetup
crablink edit rust-meetup --target https://rustconf.com/2027
crablink edit rust-meetup --no-expires       # 或 --expires 30d / RFC 3339，二者互斥
crablink rm rust-meetup
crablink ls --json | jq -r '.[].short_url'   # create / ls / get / edit 加 --json 打印 API 原样的 JSON
crablink qr rust-meetup                      # 在终端里显示 Short URL 的 QR 码，可直接扫
crablink qr rust-meetup -o poster.png        # 存为文件，格式看扩展名：.png 或 .svg
```

`--json` 输出与 [HTTP API](api.md) 的响应格式一致（`ls` 为跨所有分页的一个数组，新的在前），API 变它就跟着变；出错时仍只在 stderr 打印错误并以非零码退出，stdout 不输出 JSON。

## TUI

不带参数运行 `crablink` 会在终端里打开全屏 TUI 浏览 Link（stdout 不是终端时只打印帮助）。左侧边栏顶部通过终端图像协议显示 crablink logo，并按终端背景自动选浅色/深色版本：Ghostty、kitty、WezTerm、iTerm2 及其他支持 kitty / iTerm2 / sixel 图像协议的终端会显示；macOS Terminal.app 等不支持图像协议的终端不显示 logo，其余照常可用。TUI 运行中切换终端主题，logo 约 1 秒内跟着换版本，按 `r` 可立即重新检测（tmux 内不跟随）。在选中的 Link 上按 `v` 弹出它的 QR 码（白底黑码，与终端主题无关），`Esc` 关闭；终端放不下时显示所需的最小尺寸。
