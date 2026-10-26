# 智枢router Logo

- `zhishu-router-original.png`：用户提供的原图，保留用于后续品牌素材制作。
- `../../src/assets/icons/logo.png`：内置 imagegen 工具提取的透明背景版本。界面暗色模式用 CSS invert 显示白色图形。
- `../../src-tauri/icons/`：通用安装与应用图标保留白色底板；macOS 专用 `icon.icns` 使用下面的圆角底板版本。macOS 托盘使用透明模板图，随系统菜单栏着色。

提取提示词：Use case: background-extraction. Extract only the exact existing black artwork onto a genuinely transparent background. Preserve the central angular interlocking Z/S monogram, both segmented horizontal lines, end dots, proportions, gaps and stroke widths. Remove the paper background and excess margins. No redesign, added text, shadow or texture.

开发时已监听 `src-tauri/icons`，图标更新会触发桌面端重新编译。应用图标底板为不透明浅色；macOS 圆角轮廓外保留透明边距。界面与 macOS 托盘继续使用透明素材。

## macOS 应用图标

- `macos/app-icon.png`：最终 1024×1024 图标，实色浅色圆角底板、轻微高光与阴影，外侧透明边距匹配 Dock 中的图标轮廓。
- `macos/preview.png`：256×256 预览。
- `macos/app-icon-artwork.png`：内置 imagegen 生成的底板与 Logo 素材。
- `macos/app-icon.svg`：图标封装源，将位图放入精确圆角蒙版内，并以实色底板保证内部不透明。
- `macos/prompts.json`：内置 imagegen 的完整生成与清理提示词。

使用 Tauri icon 将 `macos/app-icon.svg` 转换为各尺寸 ICNS，更新 `src-tauri/icons/icon.icns`。仅 macOS 应用图标使用这套底板，界面 Logo 和托盘图标不使用底板。
