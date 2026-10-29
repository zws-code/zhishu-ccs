# 隔离开发

`pnpm dev` 使用正式 Tauri 配置启动开发版，只隔离 CC Switch 应用数据和 Codex；Claude、Gemini 及其他 CLI 继续读写真实用户目录。macOS 自动沿用稳定签名 runner，见 [macOS 开发签名](macos-dev-signing.md)。

## 目录与连接

默认开发数据根目录为 `~/.zhishu-ccs-dev-home`：

| 内容                           | 路径 / 值                            |
| ------------------------------ | ------------------------------------ |
| 开发版数据库、日志、模式与备份 | `~/.zhishu-ccs-dev-home/.zhishu-ccs` |
| 开发版 Codex 配置、会话与凭据  | `~/.zhishu-ccs-dev-home/.codex`      |
| Claude、Gemini 及其他 CLI 数据 | 真实用户目录，例如 `~/.claude`       |
| Tauri 标识与链接协议           | 与正式版一致                         |

启动前关闭正在运行的正式版，避免 single-instance、正式链接协议和应用数据冲突。单实例行为和正式版一致。

启动器在启动应用前自动准备开发目录，不会复制或删除真实数据库、登录凭据、配置或未完成写入。已有开发目录会继续使用。后端在开发模式下固定应用数据目录和 Codex 目录，持久化目录覆盖不能将它们改回真实目录；其他 CLI 不随开发数据目录迁移。

## 命令

```sh
pnpm dev --dry-run           # 查看目录和启动参数，不创建文件
pnpm dev                     # 启动开发版；macOS 使用稳定签名
pnpm test:dev-isolation      # 验证启动器的目录隔离
```

新建的 Codex 开发配置使用 `cli_auth_credentials_store = "file"`，已有开发配置不会被覆盖。隔离环境变量只传给应用子进程，不修改 shell 或当前 Codex 会话。

也可以指定其他独立目录：

```sh
ZHISHU_CCS_DEV_HOME=/absolute/path/to/dev-home pnpm dev
```

通过 `pnpm dev` 启动应用；直接执行 `pnpm tauri dev` 或底层 `scripts/dev-macos.sh` 会绕过开发目录隔离。

## 范围

这是开发启动配置，不是操作系统沙箱。Claude、Gemini 及其他 CLI 使用真实目录，开发版中的切换、MCP 和 Skill 操作可能直接修改正在使用的配置。系统钥匙串和系统操作也不隔离。不要在开发版启用开机自启或自动更新。
