# 隔离开发与真实配置验收

`pnpm dev`、`pnpm dev:macos` 和 `pnpm dev:isolated` 使用正式 Tauri 配置启动开发版，只隔离 CC Switch 应用数据和 Codex；Claude、Gemini 及其他 CLI 继续读写真实用户目录。macOS 自动沿用稳定签名 runner，见 [macOS 开发签名](macos-dev-signing.md)。

需要把开发版全部接到真实用户配置做本地验收时，使用 `pnpm test`。该命令清除 `ZHISHU_CCS_DEV_HOME`、`CC_SWITCH_TEST_HOME`、`CODEX_HOME`，读写真实的 `.zhishu-ccs`、`.codex`、`.claude`、`.gemini` 目录。

## 目录与连接

默认开发数据根目录为 `~/.zhishu-ccs-dev-home`：

| 内容                           | 路径 / 值                            |
| ------------------------------ | ------------------------------------ |
| 开发版数据库、日志、模式与备份 | `~/.zhishu-ccs-dev-home/.zhishu-ccs` |
| 开发版 Codex 配置、会话与凭据  | `~/.zhishu-ccs-dev-home/.codex`      |
| Claude、Gemini 及其他 CLI 数据 | 真实用户目录，例如 `~/.claude`       |
| Tauri 标识与链接协议           | 与正式版一致                         |

启动前关闭正在运行的正式版，避免 single-instance、正式链接协议和应用数据冲突。单实例行为和正式版一致。

启动器不会复制或删除真实数据库、登录凭据、配置或未完成写入。已有开发目录会继续使用。后端在开发模式下固定应用数据目录和 Codex 目录，持久化目录覆盖不能将它们改回真实目录；其他 CLI 不随开发数据目录迁移。

## 命令

```sh
pnpm dev:prepare             # 只创建开发数据目录，不启动应用或联网
pnpm dev --dry-run           # 查看目录和启动参数，不创建文件
pnpm dev                     # 启动开发版；macOS 使用稳定签名
pnpm codex:dev               # 另开开发 Codex CLI，读取开发配置
pnpm codex:dev --version     # 检查开发 Codex CLI 是否可用
pnpm test:dev-isolation      # 验证启动器的目录隔离
pnpm test:dry-run            # 查看真实配置测试目录，不启动应用
pnpm test                    # 使用真实用户配置启动开发版
```

开发 Codex 需要单独配置登录或第三方连接。新建开发配置使用 `cli_auth_credentials_store = "file"`，已有开发配置不会被覆盖。`pnpm codex:dev` 启动在当前工作目录，环境变量只传给子进程，不修改 shell 或当前 Codex 会话。

也可以指定其他独立目录：

```sh
ZHISHU_CCS_DEV_HOME=/absolute/path/to/dev-home pnpm dev
```

同样的自定义目录也要传给 `pnpm codex:dev`。不要直接执行 `pnpm tauri dev` 或底层 `scripts/dev-macos.sh`；需要真实配置测试时使用受控的 `pnpm test` 入口。

## 真实配置测试

`pnpm test` 是手动验收命令，不是自动化测试。它使用正式 Tauri 配置和默认用户目录，可能修改正在使用的供应商配置、CLI 设置、代理状态和系统凭据。启动前关闭已安装的正式版，避免 single-instance、代理端口和配置文件冲突。

```sh
pnpm test:dry-run
pnpm test
```

macOS 通过 `scripts/dev-macos.sh` 使用稳定签名 runner。其他平台直接执行 `pnpm tauri dev`。应用设置里的 Claude、Gemini 等目录覆盖仍会生效，因此实际路径可能不同于默认真实目录。

## 范围

这是开发启动配置，不是操作系统沙箱。Claude、Gemini 及其他 CLI 使用真实目录，开发版中的切换、MCP 和 Skill 操作可能直接修改正在使用的配置。系统钥匙串和系统操作也不隔离。不要在开发版启用开机自启或自动更新；需要完全使用真实路径验收时，明确使用 `pnpm test`。
