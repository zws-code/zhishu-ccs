# AGENTS.md

> 最近核对：2026-10-25。以当前代码、IPC 注册、脚本和测试为准；文档中的命令入口或测试用例存在，不代表本次已执行或真实平台已验收。

## 1. 项目定位与边界

- `zhishu-ccs` 是 cc-switch 的二开桌面应用，用于管理和切换 Claude Code、Codex、Gemini CLI 等工具的供应商配置。
- 技术栈：Tauri 2、React 18、TypeScript、Vite 7、Tailwind CSS、TanStack Query 5；后端使用 Rust、Tokio、Serde、SQLite/rusqlite。
- 上游继承能力包括供应商管理、CLI 配置适配、MCP、Skills、提示词、会话及本地代理；这些主干在本分支也有修改，不能直接用上游文件覆盖。
- 本项目新增的平台登录、会话恢复、登录历史、平台基础数据、供应商同步和订阅额度，主要位于前后端的 `platform*` 模块；它们与授权中心的 Codex/Copilot/xAI OAuth 是不同链路。
- DeepSeek Harness、WorkBuddy 均已默认显示，已有原生配置适配、平台供应商同步和回归用例；代码支持不等于目标工具的真实运行验收。DeepSeek Harness 仅面向 `desktop` 磁盘配置，不承诺运行中热更新。
- 登录或会话恢复成功后自动尝试同步一次「平台基础数据」，成功后自动同步全部已支持工具；失败可手动重试。各应用也可手动同步供应商列表；本地同步不检测外部修改，也不做覆盖确认。
- 应用可见性、MCP、Skills、增量供应商、路由和 Stack 模式使用不同白名单，见 `src/config/appConfig.tsx` 及后端对应能力判断；不要以出现在应用列表中推断所有功能均支持。
- README、CONTRIBUTING 和部分功能文档可能滞后；登录后先同步平台基础数据，再自动同步全部已支持工具。当前切换式应用采用关键字段投影，不应照旧说明恢复整份配置回填。

## 2. 目录结构速查

- `src/main.tsx`：启动、全局 Provider、错误边界及平台登录门禁；`src/App.tsx`、`src/lib/navigation.ts`：基于 React 状态的页面切换及导航持久化。
- `src/components/`、`src/hooks/`：界面、交互及业务 Hook。
- `src/config/`：应用定义、供应商预设及模型元数据；新增应用需同时检查前后端类型。
- `src/lib/api/`：Tauri `invoke` 封装；`platformAuth.ts`、`platformModels.ts`、`platformUsage.ts`、`configHistory.ts` 定义平台登录、两阶段同步、订阅额度和配置修改记录契约；共享类型另见 `src/types.ts`、`src/lib/api/types.ts`。
- `src/lib/query/`：TanStack Query 查询、mutation、缓存键和失效处理；平台目录、订阅额度和配置修改记录分别见 `platformCatalog.ts`、`platformUsage.ts`、`configHistory.ts`；平台同步能力白名单见 `src/config/platformConfig.ts`。
- `src/contexts/PlatformAuthContext.tsx`：账户态、恢复重试、可见窗口活动上报和退出缓存清理；`src/components/platform/PlatformLogin.tsx` 同时包含登录门禁及恢复失败页。
- `src/components/platform/PlatformCatalogPage.tsx`、`PlatformCatalogDetails.tsx`、`PlatformCatalogKey.tsx`：平台基础数据、分组/模型详情及显式 Key 查看；`PlatformConnectButton.tsx`：当前应用的本地供应商同步。
- `src-tauri/src/lib.rs`：后端启动、插件、共享状态和 command 注册。
- `src-tauri/src/commands/`、`src-tauri/src/services/`：IPC 入口及业务实现；平台业务目前也直接位于 commands 中。
- `src-tauri/src/config.rs`、`codex_config.rs`、`gemini_config.rs`：路径解析、格式读写、兼容及原子写入。
- `src-tauri/src/live/`、`src-tauri/src/mode/`：关键字段投影、文件补丁、写锁、模式状态及崩溃恢复。
- `src-tauri/src/database/`、`settings.rs`、`app_store.rs`：SQLite、设备设置和应用数据目录覆盖。
- `src-tauri/src/commands/platform_auth.rs`、`platform_session.rs`、`platform_login_history.rs`：平台认证、刷新与七天闲置过期、系统凭据库存储及记住密码；`platform_storage.rs`：会话、登录记录与目录快照的设备路径入口；`platform_http_log.rs`：平台 HTTP 脱敏诊断。
- `src-tauri/src/commands/platform_models.rs`：平台/本地两阶段同步与账户清理；`platform_models/base_data.rs` 存取设备目录快照，`catalog.rs` 解析模型能力，`adapter.rs` 适配工具格式，`connection.rs` 处理模型选择与目录刷新，`sync_report.rs` 维护受监测文件清单并生成同步前后哈希与操作报告。
- `src-tauri/src/config_history.rs`、`commands/config_history.rs`：配置修改记录、内部写入指纹、同步操作报告、备份定位及文件还原；前端入口见 `src/components/settings/ConfigChangeHistoryPanel.tsx`。
- `src-tauri/src/deepseek_harness_config.rs`、`workbuddy_config.rs`：新应用配置适配；业务入口见 `services/provider/` 下同名模块。
- `tests/`、`src-tauri/tests/`：前端测试及 Rust 集成/金标测试；Rust 单元测试也与源码同文件。

## 3. 常用命令

命令定义已核对 `package.json`、`CONTRIBUTING.md`、[打包发布文档](docs/zhishu-release.md)和 `.github/workflows/ci.yml`；入口存在不代表每次执行均已通过。

- pnpm 版本以 `package.json` 的 `packageManager` 为准；Rust 工具链以 `rust-toolchain.toml` 为准，当前为 1.95。
- 仓库没有 `lint`/ESLint 脚本；前端检查使用 TypeScript 和 Prettier，后者仅覆盖 `src/`。

| 用途 | 从仓库根目录执行 |
| --- | --- |
| 安装依赖 | `pnpm install --frozen-lockfile` |
| 桌面开发 | `pnpm dev` |
| 检查开发启动参数，不启动程序 | `pnpm dev --dry-run` |
| 开发启动脚本回归 | `pnpm test:dev-isolation` |
| 本机正式构建与打包 | `pnpm build` |
| 仅构建前端 | `pnpm build:renderer` |
| 前端类型检查 | `pnpm typecheck` |
| 前端格式检查 | `pnpm format:check` |
| 前端测试 | `pnpm test:unit` |
| Rust 格式检查 | `cargo fmt --check --manifest-path src-tauri/Cargo.toml` |
| Rust Clippy 检查 | `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` |
| Rust 测试，macOS/Linux 隔离示例 | `CC_SWITCH_TEST_HOME="$(mktemp -d)" cargo test --manifest-path src-tauri/Cargo.toml` |

- 前端单元测试使用 `pnpm test:unit`。
- `pnpm dev` 使用 `scripts/dev-isolated.mjs`，默认目录为 `~/.zhishu-ccs-dev-home`，可用绝对路径 `ZHISHU_CCS_DEV_HOME` 指定。**只隔离应用数据和 Codex**；Claude、Gemini 及其他 CLI 默认仍走真实用户目录，不能把整个开发环境视为全量隔离。
- 启动器只为子进程设置隔离 `CODEX_HOME`，不复制真实凭证，并为新 Codex 目录初始化文件凭证模式。其他工具写入测试需使用各自目录覆盖/夹具；Rust 测试用 `CC_SWITCH_TEST_HOME`，同时检查已有目录覆盖和 `DSH_HOME`、`WORKBUDDY_CONFIG_DIR` 等变量，不能修改智能体自身的 `HOME`/`CODEX_HOME` 来隔离。
- Rust 编译前需存在 `dist/`；打包始终启用更新包签名，需要配置发布私钥，具体配置见[底层打包命令](docs/zhishu-release.md#打包命令)。CI 后端使用 nextest；本地未安装该扩展时使用上表的 `cargo test`，不要把扩展缺失报告成代码失败。

### 打包、发布与更新

- **唯一操作文档：[智枢客户端下载、打包、发布与更新](docs/zhishu-release.md)。** 涉及发布、签名、安装包、下载清单或更新时先读该文档。
- 正式发布只走 `zws-code/zhishu-ccs` 的 GitHub Actions → 腾讯云 COS/CDN。手动运行或推送专用分支 `codex/cos-validation` 只构建并校验；推 `vX.Y.Z` tag 才上传并切换稳定清单，COS 成功后由独立作业复用同一份 payload 创建 GitHub Release 下载入口。首次正式发布不兼容旧测试客户端更新入口。当前无自有域名，先用广州桶 `zhishu-ccs-release-1252121424` 的 COS 默认 HTTPS 地址直连，CDN 尚未接入。
- 二开旧 SSH 发布工具、服务器脚本和配置示例已删除；原 `release:init/server/build/publish/import/help` 命令已移除。上游 GitHub Release/R2 实现保留，但不在智枢仓库执行；智枢使用独立 Release 作业，仅该作业授予 `contents: write`。
- `pnpm release:check` 校验 COS 配置（不连接云端）；`pnpm test:release` 运行 Node 发布回归；`pnpm test:release-cos` 运行 Python 模拟上传回归。底层 `build` 仅用于本机构建验证。
- GitHub Repository Variables 配置平台与 CDN 地址、公钥及 COS 桶/地域/前缀；更新私钥放 Repository Secrets，COS 凭据可放 `cos-production` Environment Secrets。不提交或输出密钥。
- 正式构建已保存原 payload、上传脚本需要修复时，可用 `resume-cos-release.yml` 恢复：核对原正式 tag/提交/产物来源并再次验签与校验哈希，不重编译、不移动 tag；该工作流的手动运行会实际发布。后台专用分支 `codex/cos-resume` 需提交说明带 `Resume-Release-Run: <原正式run_id>`。
- 0.0.1 已于 2026-10-29 正式发布至 COS 与 GitHub Release（Latest），原构建 run `40825474319`，成功恢复 run `40834709561`；实际 macOS 包已核对更新源/公钥并下载验签，跨版本安装升级仍待后续正式版本验收。上传器四路并发，`COS_UPLOAD_ACCELERATE` 默认关闭；仅显式启用时使用已开启的桶全球加速，累计上传读取上限 3 GB/次（含重试），不改变客户端更新地址。
- COS 版本目录不可覆盖，全部文件与签名校验并通过公网检查后才发布稳定清单；禁止降版本或遗漏已发布平台。失败重试复用原产物，不重新编译覆盖同版本。两个清单逐个发布，不宣称对象存储支持跨对象事务。
- 默认 macOS 使用 Developer ID 签名/公证；显式 `adhoc` 模式跳过公证，不能称已公证。Tauri 更新签名始终独立保留。
- 当前 Git remote 仍须核对：`origin` 是上游，`zhishu-origin` 是二开 GitLab，`zhishu-git` 是自己的 GitHub；发布 tag 推到 `zhishu-git`，不推上游。
- 云端资源与真实平台验收状态以文档和本次实测为准，不以工作流存在推断桶、CDN、证书、Secrets 已配置或安装/升级已验收。

## 4. 主干禁区

### CLI 配置路径、schema 与格式转换

- 为什么：错误的字段、认证形态或整份覆盖会导致 CLI 无法启动，并可能删除用户自己的设置。
- Claude Code：默认 `~/.claude/settings.json`，兼容已有 `~/.claude/claude.json`；供应商字段主要在 `env`，MCP 默认另存 `~/.claude.json` 的 `mcpServers`。
- Codex：默认 `~/.codex/config.toml` 和 `auth.json`；供应商行使用 `{ auth, config }`，其中 `config` 是 TOML 字符串。
- Codex 当前第三方路由通常投影到 `[model_providers.custom]`，Key 写入 `experimental_bearer_token`；不要退回仅向 `auth.json.OPENAI_API_KEY` 写 Key 的旧方案。
- Gemini：默认 `~/.gemini/.env` 和 `settings.json`；行结构为 `{ env, config }`，关注 `GEMINI_API_KEY`、`GOOGLE_GEMINI_BASE_URL`、`security.auth.selectedType`、`model.name`。
- DeepSeek Harness：通过 `get_dsh_home()` 解析设置覆盖、`DSH_HOME` 和默认 `~/.dsh`；凭证在 `.credentials.yaml`，配置补丁在 `profiles/desktop/cordis.patch.yml`。不要把 Key 写入 profile patch，也不要恢复已不合法的旧 `profile` 字段。
- WorkBuddy：通过 `get_workbuddy_home()` 解析设置覆盖、`WORKBUDDY_CONFIG_DIR` 和默认 `~/.workbuddy`，写入 `models.json`；兼容模型数组和含 `models` 数组的对象，保留无关字段及 `availableModels` 的既有处理。
- 所有路径必须调用已有路径函数，保留设置覆盖和隔离环境的优先级；不要把上述默认路径硬编码进新逻辑。
- 要改先做什么：读对应 `*_config.rs`、`live/floor.rs`、`live/project/` 和 `live/patch/`，补充格式往返、未知字段保留及金标回归。

### Tauri command 接口契约

- 为什么：命令名、参数大小写、可选值或返回形状改变，会直接破坏现有前端调用。
- 例如 `switch_provider({ app, id })` 返回 `{ warnings, route? }`；不能改成裸布尔值或吞掉非致命警告。
- `get_providers({ app })` 返回供应商映射；`add_provider`、`update_provider`、`delete_provider` 返回布尔值，参数以 `src/lib/api/providers.ts` 为准。
- 要改先做什么：对照前端 wrapper、Rust 签名、Serde 属性及 `generate_handler!`，同步类型、调用方、测试与 mock；保留现有错误形状。

### 备份、恢复、并发与原子写入

- 为什么：配置文件、数据库指针和代理模式必须一致；写到一半失败时，简单恢复整份旧文件可能覆盖 CLI 的新修改。
- 通用引擎的首写备份位于设备目录 `~/.zhishu-ccs/backups/live-first-write/`；`live-state.json` 保存本机模式与 pending，二者不跟随数据库目录覆盖。
- `mode/operation.rs` 在发布前失败时放弃；开始发布后按 pending 前滚恢复，并保留检测到的外部修改。不要统一改成“失败就全量回滚”。
- 平台供应商同步通过 `ProviderService` 复用各工具的配置写入与恢复路径；旧单模型接入命令及其专用 `snapshot.rs` 已移除。不要恢复整库或整个配置目录。
- 平台供应商同步不做同步前检测、确认或备份：外部修改不阻塞同步，同步直接按本机目录数据调和托管供应商；未变化的托管行不改写原生文件，外部编辑因此保留。`platform_models/sync_report.rs` 在同步前后快照托管供应商与受监测文件哈希，`ProviderSyncReport` 按差值记录增删改与变更文件，失败或部分失败同样留痕。
- 配置修改记录区分供应商数据库变更与实际文件变更；`ProviderSyncReport` 记录增删改、变更文件及备份路径，新同步不再创建 `backups/platform-sync/` 备份，仅历史记录可能带有备份路径，既有入口仍可打开。同步报告不能作为单文件历史还原。
- 聚合模式增删成员写独立的操作摘要记录（`config_history::record_operation_summary`，`ConfigOperationSummary`）：即使只改派生的模型目录（仅记指纹）或文件完全没变也留痕；摘要不是文件级记录，同样不能作为单文件还原。
- 单文件历史还原必须保留路径归属、当前内容、解析和路由模式检查，先保存独立 `config-change-history/restore-backups/` 备份，再走既有原子发布/pending 恢复。清空历史不能删除恢复备份。
- DeepSeek Harness、WorkBuddy 还有各自的事务或局部回滚路径，不能假定它们全部受通用写入引擎保护。
- 要改先做什么：读实际调用链，保留应用级锁、hash 比较、首写备份和原子发布；覆盖解析失败、写入失败、并发修改及重启恢复。

### API Key、密码与 token 存储

- 为什么：本项目同时使用内存、系统凭据库和明文配置；混淆它们会造成泄漏、丢失登录或退出后凭据残留。
- 普通供应商 Key 随 `settings_config` 以 JSON 文本存入 SQLite，默认数据库为 `~/.zhishu-ccs/cc-switch.db`；生效时还可能写入 CLI 配置，不能宣称统一由 keyring 加密。
- Codex 原生登录需尊重 `cli_auth_credentials_store` 的 `file/keyring/auto/ephemeral` 语义；沿用 `codex_config.rs` 和 `services/provider/codex_direct.rs` 的判断。
- 平台 access token 和 2FA 临时 token 仅驻 Rust 内存；refresh token 默认持久化到系统 keyring，service 为 `com.zhishu.ccswitch.desktop.platform-session`。它们均不返回前端，不进入 localStorage、SQLite、配置导出或云同步；不要恢复为每次重启都需登录的旧实现。
- `platform-session.json` 只保存平台地址、用户 ID、凭据条目 ID 和最后使用时间；正常位于 Tauri `app_local_data_dir()`，开发隔离时使用 `get_app_config_dir()`。凭据 ID 按存储目录区分，文件使用私密原子写入。
- “记住密码”独立使用 keyring service `com.zhishu.ccswitch.desktop.platform-login`；`platform-login-history.json` 与会话元数据遵循相同目录规则，不含密码。保持登录不依赖勾选记住密码，也不使用已保存密码自动登录或跳过 2FA。
- 平台分组模型 Key 则保存在设备本地 `platform-catalog.json`，应用后还可能在供应商 SQLite 行和 CLI 配置中出现；目录文件使用私密原子写入（Unix 0600），不是 keyring 加密存储，不进入数据库导出或云同步。普通目录摘要不含 Key；只有 `platform_get_catalog_key` 显式查看/复制入口返回单个分组 Key，不得放入前端查询缓存。
- 授权中心另有 `codex_oauth_auth.json`、`copilot_auth.json`、`xai_oauth_auth.json` 文件存储；不要与平台会话或记住密码的存储/清理规则混用。
- 要改先做什么：列清读取、落盘、备份、退出和删除路径；保留私密文件权限及日志脱敏，验证 keyring 不可用时不会改用明文密码兜底。

### 平台登录与会话恢复

- 为什么：登录、会话恢复与账户切换涉及系统凭据库、本机快照和托管配置；误改可能串号、丢失登录或留下退出后可恢复的凭据。
- 登录 IPC：`platform_login({ baseUrl, email, password, rememberPassword })`、`platform_login_2fa({ code })` 返回 `authenticated` 或 `requires_2fa` 分支；成功可带 `persistenceWarning`。会话持久化失败可保留本次登录并提示，不能用明文 token/密码兜底。
- `platform_get_account({ active })` 返回 `{ baseUrl, user: { id, email, username, role, balance } }` 或 `null`；平台错误保持 `{ code, message }`。前端按页面可见性传 `active`，后端还检查主窗口可见且未最小化；后台检查不延长闲置期限。
- 重启从 keyring 恢复 refresh token，向平台刷新并验证账户；连续超过七天未使用、确认凭据失效或用户身份变化时清除会话。401 按现有路径刷新，轮换结果先保存再继续请求；暂时断网或凭据库故障保留可重试状态。
- `PlatformAuthContext` 每分钟及窗口重新可见、网络恢复时查询账户。已有账户遇到暂时网络错误仍可使用本地功能；启动恢复失败进入 `recoveryPending` 重试页，可主动退出后重新登录，但没有未验证的离线入口。
- 认证 HTTP 路径均带 `/api/v1` 前缀：`/settings/public`、`/auth/login`、`/auth/login/2fa`、`/auth/me`、`/auth/refresh`、`/auth/logout`。保留人机验证不支持时明确拒绝的行为，不绕过平台验证。
- 主动退出先失效本机会话指针并清理会话 keyring，再尽力撤销服务端 refresh token，同时清理前端查询缓存；保留本机目录、供应商数据库和工具配置中的 API Key，不改写原生配置。退出后平台目录与 Key 查看/同步仍需登录并校验账户归属；同账号重新登录可继续使用本机目录。
- 要改先做什么：核对 `platform_auth.rs`、`platform_session.rs`、`platform_login_history.rs`、门禁和 Context；覆盖重启恢复、轮换后断网、凭据库故障、七天边界、2FA、切换账号、主动退出与迟到响应。

### 平台基础数据与供应商同步

- 为什么：平台拉取会管理服务端 Key，本地同步会写供应商和工具配置；两阶段副作用不同，不能把按钮重新合并为隐式联网与自动应用。
- 平台层：`platform_sync_catalog()` 获取可用分组、完整订阅、分页 Key 和模型目录，按需创建分组 Key，保存 `platform-catalog.json`，返回目录摘要；不修改 Agent 供应商。整体拉取失败保留上一快照，分组失败保留失败标记。
- 本地层：`platform_get_catalog()` 返回当前账户本机摘要或 `null`；`platform_sync_providers({ agent })` 只从本机目录生成供应商，不发平台请求、不创建/撤销 Key，也不触发平台额度刷新，且不做同步前检测或覆盖确认。`agent: null` 表示全部已支持应用。缺快照返回 `CATALOG_REQUIRED`；旧版本退出或显式清理过 Key 的快照返回 `CATALOG_CREDENTIALS_REQUIRED`，需先手动同步平台数据。
- 应用页右上角「更多」中的「一键删除平台供应商」调用 `platform_delete_providers({ agent, baseUrl, userId })`，删除当前应用、当前账户的平台托管行；「平台基础数据」页面的同名按钮传 `agent: null`，遍历全部已支持应用（含未显示应用），返回总计及逐应用 `apps` 结果，单个应用失败不阻塞其他应用。复用操作锁和 `ProviderService`，无安全备用项的当前供应商保留并提示。该操作不依赖平台目录、不联网、不改平台目录、不撤销服务端 Key，部分失败也记录 `platform_delete` 报告；后续同步可重新添加供应商。
- 同步返回 `{ added, removed, skipped, warnings }`，另有可选 `updated`、`nativeFilesChanged`。部分成功仍需展示 warnings。供应商行有变化不代表原生配置文件已变化。
- 目录摘要为 `{ baseUrl, userId, syncedAt, credentialsCleared, groups, warnings }`，`syncedAt` 为 Unix 毫秒，模型展示字段使用白名单。`platform_get_catalog_key({ baseUrl, userId, groupId })` 只读本机单个 Key，并校验当前身份、快照归属及分组；查看详情不触发同步。
- 平台层读取 `/api/v1/groups/available`、`/api/v1/subscriptions`、分页 `/api/v1/keys`，按需 `POST /api/v1/keys`，用模型 Key 访问 `/v1/models` 及其能力目录；网页登录 JWT 不能作为模型 Key。订阅分组不受名称前缀限制，普通分组要求 `智枢router-ccs-` 前缀，专用 Key 名为 `zhishu-ccs-<platform>-<group-id>`。
- 专用 Key 按当前账户的名称与分组复用，额度耗尽不能靠新建 Key 绕过。旧 Key 仅由平台层在检查所有应用托管项及手工供应商引用后撤销；过期/暂停订阅的归属和失败分组须保留，不能按当前可用分组简单误删。
- 协调和清理保留 `meta.platformManaged` 的来源、用户、分组与订阅类型判断；不修改本地自建项和其他账号项。一般不主动切换当前供应商；删除当前项需要先找到安全备用项，否则保留并提示。刷新模型目录时保留未知字段、未报告参数和用户选择，目录失败不能当作空列表删除。
- 当前前后端平台白名单为 `claude`、`claude-desktop`、`codex`、`gemini`、`grokbuild`、`opencode`、`openclaw`、`hermes`、`pi`、`deepseek-harness`、`workbuddy`、`mcode`、`kimi`；`agy` 尚不在其中。新增应用必须同时核对 `platform_models/adapter.rs::AGENTS` 和 `src/config/platformConfig.ts::PLATFORM_SYNC_APPS`，一致性回归见 `tests/config/platformConfig.test.ts`。
- 订阅额度通过 `platform_get_subscription_usage` 在 Rust 中读取 `/api/v1/subscriptions/progress` 与完整订阅记录，按分组匹配；前端按账户缓存、五分钟刷新。过期、暂停、进度缺失及查询失败不能显示为不限量或退回钱包余额。
- 要改先做什么：核对 `APPLY_LOCK`、本地身份校验、目录解析与存储、`sync_report`、局部恢复及前端缓存失效；用模拟响应和临时目录验证两阶段互不越界。

## 5. 可以放心改的叶子区域

- React 组件布局、样式、图标摆放、文案和纯展示格式，可在不改变数据语义、交互入口及可访问性的前提下局部修改。
- 纯函数提取、重复展示代码合并和局部命名调整，可在输入输出保持一致且类型检查通过时进行。
- 表单序列化、供应商 ID、query key、登录门禁、配置编辑器和同步按钮行为涉及契约，不因位于组件目录就视为叶子区域。

## 6. 代码约定

- 页面导航沿用 `App.tsx` 与 `lib/navigation.ts` 的状态模型；局部 UI 使用 React state，平台账户态使用 `PlatformAuthContext`。
- 后端数据沿用 TanStack Query；供应商缓存键包含应用 ID，例如 `["providers", appId]`。平台目录键为 `["platformCatalog", baseUrl, userId]`，订阅额度键也包含平台地址和用户 ID；跨账户不得复用数据，退出后的迟到响应不得重建已清空的缓存。
- mutation 后按实际副作用失效缓存：本地供应商同步刷新对应应用列表、相关原生配置/代理状态和 `configChangeHistory`，不刷新 `platformUsage`；平台目录更新只更新对应账户的目录查询。沿用调用方现有范围，不用全局失效把本地同步变成隐式平台请求。
- IPC 优先封装在 `src/lib/api/`，再由 query/hook 调用；平台 HTTP 认证和供应商协调继续由 Rust 处理。
- 普通错误按现有场景使用 `extractErrorMessage` 和国际化兜底；平台错误使用 `platformAuthError`，不展示任意 IPC 异常或服务端原始响应。
- 通用通知使用 `src/lib/toast.ts` 的去重封装；用户文案使用 i18n，修改共享翻译键时检查 `zh`、`zh-TW`、`en`、`ja`、`tr`。
- Rust 业务层沿用 `Result<T, AppError>`，通过 `?` 传播并附带路径/上下文；多数 command 映射为字符串错误，平台 command 保留 `PlatformAuthError`。
- Rust 使用 `log`；前端已有 `reportFrontendError`、`logFrontendInfo` 和 Tauri 日志插件。平台 HTTP 诊断沿用 `platform_http_log::RequestLog`，先脱敏再截断，只记录可解析的安全摘要；不直接记录密码、token、完整认证头、原始响应或含 Key 的配置。
- 新 command 流程：实现 `#[tauri::command]` → 在 `commands/mod.rs` 声明/导出 → 加入 `lib.rs` 的 `generate_handler!` → 添加前端 wrapper、类型及相关测试。
- 只有 `#[tauri::command]` 标记不代表已暴露；新增共享状态还需在启动阶段注册，参数命名按现有 camelCase IPC 约定核对。

## 7. 改动后的验证清单

- [ ] 前端代码改动通过 `pnpm typecheck`；按改动范围完成格式检查、相关测试和构建，并记录未执行项。仅文档改动核对引用路径、命令和代码事实，不把未运行的检查写成通过。
- [ ] 写入后的 JSON/TOML/YAML/.env 可被对应解析器读取；用户无关字段、注释及文件权限符合该适配器的既有约定。
- [ ] 在隔离配置下切换供应商后，实际启动受影响 CLI，验证地址、模型及认证；缺少运行环境时明确记录未验证。
- [ ] 配置写入改动验证备份存在，并覆盖写入失败、并发编辑、恢复或回滚失败；不能仅验证成功切换。
- [ ] 平台会话覆盖重启恢复、七天闲置边界、后台不续期、刷新令牌轮换、keyring 故障和退出后不再恢复；401 尝试刷新，确认失效后返回登录页，临时错误不误清账户态。
- [ ] 启动恢复失败有重试入口；仍无未登录离线进入主界面的入口，不能把 `recoveryPending` 描述为离线模式。
- [ ] 平台目录刷新不写 Agent 配置；Agent 同步不发平台请求、不新建/撤销 Key、不刷新额度；缺快照或凭证已清理时提示先同步平台数据。
- [ ] 覆盖首次已有配置、外部编辑、正常内部切换和部分失败；同步不做检测与确认，外部修改不阻塞同步，报告准确区分数据库与原生文件变化，部分失败同样留痕。
- [ ] 同步超时或部分失败能提示并重试，不阻塞已登录状态下的本地供应商使用；失败分组、过期/暂停订阅、本地自建项及其他账号项不误删。
- [ ] 退出后保留目录、供应商和工具配置中的 API Key，登录会话不可恢复；未登录不能经平台接口查看或同步 Key，同账号重新登录可复用目录。验证切换账号、前端缓存清理、迟到响应隔离及旧版本已清理凭证快照的兼容。
- [ ] command 改动同时验证前端参数/返回类型、Rust 序列化和 mock；新应用另测原生配置样例及平台白名单边界。

相关回归入口：`tests/lib/platformAuth.test.ts`、`platformModels.test.ts`，`tests/components/PlatformLogin.test.tsx`、`PlatformCatalogPage.test.tsx`、`PlatformConnectButton.test.tsx`、`PlatformUsageFooter.test.tsx`、`ConfigChangeHistory.test.tsx`；后端见 `platform_auth.rs`、`platform_session.rs`、`platform_models.rs` 及其子模块、`config_history.rs` 的同文件测试。

## 8. 智能体工作纪律

- 先读相关实现、调用方和测试，再修改；开始前检查工作区，保留用户已有未提交改动。
- **禁止删除上游代码**：品牌、数据目录和功能扩展可以做必要适配，但必须保留上游文件、组件、函数、类型、API、兼容入口及其依赖；不得以未使用、冗余、重构或清理为由删除。整理提交时发现既有的上游清理删除，应恢复后验证，不能直接压入新历史。品牌相关文件改名须保留对应实现，不视为删除功能。
- 每次变更只解决一个明确问题，小步提交；不要把无关格式化、重命名或用户改动混入提交。
- 不手改 `dist/`、`src-tauri/target/` 等构建产物；锁文件随依赖工具更新，金标快照通过测试提供的更新入口生成并逐项审查。
- 改 Tauri command 必须同步前后端契约、注册、类型和测试，不以“前端暂时没报错”替代验证。
- 涉及配置文件写入必须保留原始备份及既有恢复路径；新路径缺少备份时先补齐，禁止直接覆盖真实用户配置试验。
- 测试使用隔离目录；不要通过修改 `HOME` 或 `CODEX_HOME` 来挪用当前智能体/用户的工作环境，也不要删除 pending、备份来掩盖失败。
- 根目录 `AGENTS.md` 已被 Git 跟踪，虽然 `.gitignore` 仍有同名规则；修改会正常进入 diff，不要再当成仅本机未跟踪文件。当前 `origin` 指向上游，`zhishu-origin` 指向二开仓库；推送/发布前核对目标 remote，不按默认 `origin` 推送二开改动。

## 待人工确认项

- TODO：确定二开对应的上游基线 commit/tag；当前 `origin` 仍指向上游，逐文件的原版/二开归属不能仅凭目录判定。
- TODO：确认登录过期后返回登录页是否符合产品要求，还是需要新增离线进入主界面的降级路径。
- TODO：补充平台服务端正式接口版本、响应样例、错误码、Key 创建/撤销与额度策略，以及 `/v1/models` 普通列表和能力清单的兼容承诺。
- TODO：补充 WorkBuddy 目标版本与真实 `models.json` 的运行验收，以及 DeepSeek Harness `desktop` 配置的端到端验证；平台同步已实现，不能再列为尚未接入。
- TODO：补充 macOS、Windows、Linux 上系统凭据库与七天会话恢复的实际验收记录；已有模拟测试不替代真实系统、断网和重启验证。
