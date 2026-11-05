# crablink — Logo 使用规范

## 1. Logo

**想法**：蟹壳就是一节短链环。一眼看是螃蟹，再看是链接。

| 文件 | 用途 |
|---|---|
| `masters/crablink-horizontal.svg` | 首选，README、网站页头、文档 |
| `masters/crablink-stacked.svg` | 方形或竖向空间，比如贴纸、海报、社交封面 |
| `masters/crablink-symbol.svg` | 头像、favicon、app 图标，以及空间很小的地方 |
| `masters/crablink-wordmark.svg` | 图形已经出现过时，单独用字标 |
| `*-reversed.svg` | 深色背景（图形保持蟹红，字标改为白色） |
| `export/` | 黑色、白色、单色版本，PNG 文件，以及网页图标套件 |
| `export/stacked/crablink-stacked-800.png`、`crablink-stacked-reversed-800.png` | 竖版全彩透明 PNG（宽 800），终端里显示 logo 用，见第 7 节 |

所有母版都是纯路径，没有 `<text>`、位图、滤镜或渐变。

## 2. 安全空间

记 **X = 蟹壳的环宽**，即 256 画布中的 28 单位，约为图形高度的 1/5。
Logo 四周至少留 **2X** 的空白，其他文字、图形和画面边缘都不能进入这个范围。

## 3. 最小尺寸

| 版本 | 屏幕 | 印刷 |
|---|---|---|
| 图形 | 16 px | 6 mm |
| 横版组合 | 宽 120 px | 宽 25 mm |
| 竖版组合 | 宽 80 px | 宽 18 mm |

16px 下靠的是两侧的 "=" 腿撑住螃蟹的轮廓。我们试过去掉腿的简化版，那一版在小尺寸下会读成一张脸，所以小尺寸也直接用完整图形。

## 4. 颜色

| 名称 | HEX | RGB | CMYK（近似） | Pantone（近似，需打样） |
|---|---|---|---|---|
| 蟹红 Crab Red | `#E0412A` | 224 65 42 | 0 71 81 12 | 7417 C |
| 壳墨 Shell Ink | `#17181C` | 23 24 28 | 75 65 55 85 | Black 6 C |

- 默认用法：白底或浅底上，图形用蟹红、字标用壳墨。深底上，图形用蟹红、字标用白色。
- 单色场景（印章、刺绣、单色印刷）用全黑、全白或全红的单色版。
- 对比度：蟹红对白色为 4.23:1，蟹红对壳墨为 4.19:1，都满足图形的 3:1 要求。不要把蟹红用作小号正文文字的颜色。

## 5. 字体

字标是几何构造的，不依赖任何字体，不用担心授权：
- x-height 100、笔画 22，圆形字母上下各超出 1.5 做视觉补偿。
- c 的开口和蟹钳的缺口都是 60° 径向切口，k 的斜笔是 45°。

配套正文字体建议用 Inter 或系统无衬线字体，代码用 JetBrains Mono。

## 6. 不要这样做

- 不要拉伸、倾斜或旋转 logo。
- 不要改颜色（品牌色和单色版之外的颜色都不要用），不要加渐变、描边或投影。
- 不要去掉腿或蟹钳，也不要换掉中间的链环。
- 不要用其他字体重新排 "crablink"，要用 `crablink-wordmark.svg`。
- 不要改变图形和字标的大小比例或间距，要用现成的组合文件。
- 不要放在杂乱的照片上，也不要放在和蟹红对比度不足的颜色上（比如橙色、粉色）。

## 7. 终端

- 只通过终端图像协议（kitty / iTerm2 / sixel）显示 logo，用 `export/stacked/` 下的 800 px 全彩 PNG。终端不支持图像协议时就不显示 logo。
- 不要用字符画代替 logo：半块字符（▀▄）、盲文点阵、ASCII 都不行。缺 logo 时宁可不显示，也不要"补"一个字符画。
- 按终端背景选组合：浅色背景用标准版 `crablink-stacked-800.png`（壳墨字标）；深色背景或无法判断背景时用反白版 `crablink-stacked-reversed-800.png`（白色字标）。
- 终端里用等宽字体打出的 "crablink" 只是名称标签，不是字标，不受第 5 节和第 6 节字标规则约束，也不要把它当 logo 摆放。

## 8. 待办与说明

- 商标：发布前请做一次正式的商标检索（商标库加以图搜图）。本设计不能保证没有冲突。
- 网站部署：把 `export/symbol/` 下的 `favicon.ico`、`favicon.svg`、`apple-touch-icon.png`、`icon-*.png`、`maskable-512.png` 和 `site.webmanifest` 放到站点根目录，再把 `head-snippet.html` 的内容加进 `<head>`。
- 需要改几何时，修改 `build_kit.py` 后重新运行 `python3 build_kit.py`，母版会重新生成。改完后需要重新执行导出命令。
- 终端 PNG 用 logo-design 技能的 `scripts/render_png.py` 从 `masters/crablink-stacked*.svg` 导出（`--width 800 --height 661`），导出后运行 `python3 test_stacked_exports.py` 检查尺寸、透明背景和颜色。
