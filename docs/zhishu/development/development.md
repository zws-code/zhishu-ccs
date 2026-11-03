# 开发启动

`pnpm dev` 直接调用 `tauri dev`，使用正常 Tauri 配置及应用数据与工具配置路径。启动、参数和进程生命周期交由 Tauri CLI 管理，不增加自有通用启动器。

```sh
pnpm dev          # 启动开发版
pnpm dev --help   # 查看 Tauri 开发参数，不启动应用
```

各平台开发启动均使用上述 Tauri CLI 入口。2026-11-03 按用户要求移除自有 macOS 开发签名脚本及对应测试命令。OAuth2 refresh token 的保存与恢复由 Rust 会话模块通过系统凭据库完成，不依赖开发签名脚本；macOS 开发版重编译后仍可能出现系统钥匙串授权提示。

## 数据路径

- 应用数据库默认位于 `~/.zhishu-ccs/cc-switch.db`，仍遵循设置中的目录覆盖。
- 设备设置、模式状态和首写备份位于 `get_device_data_dir()`，不随数据库目录覆盖。
- Codex 默认目录仍由 `get_codex_config_dir()` 解析，设置中的目录覆盖优先；其他工具沿用各自路径函数。
- 平台会话元数据与模型目录使用 Tauri `app_local_data_dir()`；OAuth2 refresh token 使用系统 keyring，不保存账号密码或登录历史。

开发版与正式版使用相同应用身份和默认路径。启动前关闭正式版；开发版中的供应商切换、MCP、Skills 等操作会使用当前配置。

2026-11-03 按用户要求移除开发隔离，旧 `ZHISHU_CCS_DEV_HOME` 不再生效；不注入 `CODEX_HOME`，不创建开发目录或写入 `cli_auth_credentials_store`。历史 `~/.zhishu-ccs-dev-home` 及其中的配置、数据库和凭据引用保留原样，不自动迁移、合并或清理。决策和回退见 [ADR-0010](../adr/ADR-0010-remove-development-isolation.md)。

## 测试

测试专用 `CC_SWITCH_TEST_HOME` 仍保留。Rust 测试使用临时目录，工具写入测试同时检查各自目录覆盖及 `WORKBUDDY_CONFIG_DIR` 等变量；不要将开发启动视为测试隔离环境。
