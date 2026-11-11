# Zhishu CCS 上游补丁登记

> 这是按基线/事件累积的补丁台账。222 项清单针对 2026-10-29 的 4.0.21 快照，后续变更按事件追加；旧路径、版本和分支描述仅适用于该段，不是当前 4.0.22 的完整重新统计。当前现状见 [代码基线](baseline.md)。

## 2026-11-09：新增简易模式外壳

用户要求的简易模式：右下角「高级/简易」切换（localStorage 设备偏好），简易模式侧栏由「应用」开关页和既有全局项组成，各应用的供应商页从「应用」页进入。应用页页头隐藏、供应商列表只呈现当前登录账号的平台托管行。「应用」开关页复用「接入到工具」向导的接入逻辑与「一键删除平台供应商」，零新 IPC、无后端改动；新业务文件全部位于自有或新增路径（`src/lib/shellMode.ts`、`src/zhishu/models/platformManaged.ts`、`platformProvision.ts`、`PlatformAppsPage.tsx`、`src/components/shell/ShellModeToggle.tsx` 及测试，L0）。

| ID | 文件 | 处理与不能仅包装的原因 | 等级 | 验证 |
| --- | --- | --- | --- | --- |
| SIMPLE-20261109-01 | `src/App.tsx` | 外壳按模式分支：侧栏变体、应用页页头替换为拖动条、供应商列表按账号过滤、`platformApps` 全局页分支、右下角切换按钮与视图归一；布局分支依赖固定渲染入口内部的状态与回调，外层包装无法替换 | L1 | typecheck、format、前端单测、`pnpm dev` 手动场景 |
| SIMPLE-20261109-02 | `src/lib/navigation.ts` | `GlobalPage` 增加 `platformApps` 并新增跨模式视图归一 `normalizeViewForMode`；导航模型与持久化是固定入口 | L1 | navigation 单测 |
| SIMPLE-20261109-03 | `src/components/shell/Sidebar.tsx` | 在侧栏内部按模式选择目录，外层包装无法替换既有目录；`simple` 变体复用 `NavItem`、设置目录和账户菜单，「平台基础数据」紧跟「应用」，不提供独立的「供应商」按钮；高级目录保留上游既有条目 | L1 | 前端单测、`pnpm dev` 手动场景 |
| SIMPLE-20261109-04 | `src/components/shell/AppPageHeader.tsx` | 新增 `SimpleAppHeader` 导出（拖动区 + 自绘窗口按钮的极简页头）；既有组件与其余导出不变 | L1 | Windows/Linux 自绘窗口实机待验收 |
| SIMPLE-20261109-05 | 五语 JSON | 共享语言包新增 `simpleMode.*` 文案，含独立的「已接入」标签和「未启用平台模型」说明；复用现有语言加载入口 | L1 | 五语解析、组件测试 |

向导接入逻辑抽取为共用模块（`PlatformOnboarding.tsx` 改为调用 `src/zhishu/models/platformProvision.ts`，行为不变，回归测试未改断言）属自有模块重组，L0。功能行为与已执行/未执行验证见[简易模式](../features/simple-mode.md)。

## 2026-11-09：合并上游 v4.0.28

依据 [ADR-0013](../adr/ADR-0013-merge-4.0.28.md)，从当前产品 0.0.9 三方合入官方 v4.0.28，保留双方祖先。复用上游 Codex 路由写入、会话迁移、代理、会话索引、settings 与 IPC；没有新增平台业务或平行核心。15 个冲突文件中 5 个为身份/清单类（L1），语义类集中在 Codex 路由形态与历史迁移（L3）。

| ID / 既有补丁 | 文件 | 处理与不能仅包装的原因 | 等级 | 验证 |
| --- | --- | --- | --- | --- |
| SYNC-20261109-01 / ZS-044、ZS-046、ZS-047、ZS-148 | `package.json`、`src-tauri/Cargo.toml`、`Cargo.lock`、`tauri.conf.json` | 固定工具入口保留智枢 0.0.9、身份、1420 端口与深链 scheme；上游仅版本号且同样移除 staticlib/cdylib；锁文件删除上游根包条目后 `cargo check --locked` 核验 | L1 | frozen install、Cargo check/Clippy |
| SYNC-20261109-02 / 既有路由投影补丁（ADR-0009） | `src-tauri/src/mode/controller.rs`、`services/provider/codex_direct.rs`（自动合并）、相关测试 | 上游把「有 ChatGPT 登录的第三方路由」改写为内置 openai 形态；写入路径经 `ROUTE_ID` 常量天然落智枢 `zhishu` 表，无登录/统一历史/客户端设置分支仍投影智枢表。5 个上游测试块的 `custom` 表断言适配为 `zhishu`（本分支不写 `custom` 表）；路由/契约写入在既有核心内部，外层包装无法替换 | L3 | Codex 路由契约/会话/金标全量回归 |
| SYNC-20261109-03 / 既有统一历史迁移补丁 | `src-tauri/src/codex_history_migration.rs` | 采纳上游启动期每次迁入与 state DB 计数去重；保留多桶支持，兜底标记须命中当前目标桶（`custom` 或 `zhishu`）；切分提示受管桶扩为 openai/custom/zhishu。计数、标记与迁移目标都在迁移实现内部 | L3 | 迁移单测与切换提示回归 |
| SYNC-20261109-04 / 既有配置历史补丁 | `src-tauri/src/config.rs` | `commit_staged` 先按逻辑路径 `capture_before` 留痕（应用归属/还原入口依赖逻辑路径），再采纳上游软链接写穿；顺序在原子写入核心内部 | L3 | 配置历史与写入引擎回归 |
| SYNC-20261109-05 | 五语 JSON、金标 `codex_red_lines.rs`、`deeplink-codex.txt` | 统一历史文案采用上游新行为 + 智枢备份路径；金标指针期望改绝对路径但文件名保持 `zhishu-ccs-model-catalog.json`；快照随上游移除 `disable_response_storage`。固定字典/金标不能靠旁路替代 | L1/L0 | 五语解析、金标测试 |
| SYNC-20261109-06 | `.github/workflows/ci.yml` | 采纳上游四分片 `frontend-tests` 作业；智枢发布脚本与 COS 模拟步骤保留在 frontend 作业。固定工作流入口 | L1 | YAML 解析 |

自动合并复核的 `lib.rs` 注册、`settings.rs`（WorkBuddy/DSH 保留）、`App.tsx`（平台/配置历史入口保留）、`deeplink/provider.rs`、`live/project/codex.rs`（`CATALOG_FILENAME` 指向智枢常量）及 `switch_provider` 契约（增量可选 `codexHistorySplit`）见 [同步报告](../upstream-sync/2026-11-09.md)。平台自有源码、schema 24、OAuth2 会话和两阶段同步契约不变。

## 2026-11-06：合并上游 v4.0.26

依据 [ADR-0012](../adr/ADR-0012-merge-4.0.26.md)，从当前产品 0.0.7 三方合入官方 v4.0.26，保留双方祖先。直接复用上游 DAO、Provider、IPC、路径函数、模式/代理与原子写入；没有新增平台业务或平行核心。上游内部性能/协议修复必须更新实际调用处，外层新增 Adapter 无法替代内部锁、查询或转发，故最高 L3。

| ID / 既有补丁 | 文件 | 处理与不能仅包装的原因 | 等级 | 验证 |
| --- | --- | --- | --- | --- |
| SYNC-20261106-01 / ZS-044、ZS-046、ZS-047、ZS-148 | package、pnpm 锁、Cargo/Tauri 清单 | 固定工具入口保留智枢 0.0.7、身份和命令；采用上游 CLI 2.12.4；Cargo.lock 最终无差异 | L1 | frozen install、TypeScript、Cargo check/Clippy、发布模拟 |
| SYNC-20261106-02 / ZS-158 | `src/App.tsx`、`useUsageEventBridge.ts`、`windowActivity.ts` | lazy/Suspense 与查询可见性作用于原组件/事件生命周期，保留智枢平台/历史入口和认证门禁，外层包装不能替代内部声明与查询调度 | L2 | App、窗口活动、Hook 与平台前端全量回归 |
| SYNC-20261106-03 / 既有 WorkBuddy 补丁 | `settings.rs`、`useDirectorySettings.ts`、`useSettings.ts`、`types.ts`、目录组件及测试 | 固定设备设置及目录枚举同时保留 WorkBuddy/OMP 并接入 DSH；DSH 配置读写继续复用既有路径解析器 | L2/L3 | 目录 Hook、设置、DSH/WorkBuddy 隔离回归 |
| SYNC-20261106-04 / 既有深链补丁 | `deeplink/provider.rs` | 深链 match 是固定分发入口，合并上游新增应用协议与模型构造，保留智枢身份及 WorkBuddy 专属添加路径 | L3 | 深链解析/导入、Provider 全量回归 |
| SYNC-20261106-05 / 既有发布补丁 | `.github/workflows/release.yml` | stapler 重试在固定 Actions 步骤内接入；保留 Developer ID/adhoc 条件、COS、NSIS 与 Linux 暂停，新增模块无法替换已有签名调用 | L1 | YAML 解析、47 段 shell 语法与 Node/Python 发布模拟 |
| SYNC-20261106-06 | 五语 JSON、`AGENTS.md` | 共享字典补齐 DSH 提示且避免 OMP 重复键；维护入口更新基线；固定字典/指令不能靠旁路文档替代 | L1 | 五语解析、格式、路径/实现核对 |

自动合并的 commands、database、proxy、services、mode、lib 吸收上游批量查询、后台执行、备份维护和协议修复（L3），保留智枢启动注册、ProviderMeta、配置归属及首写备份/pending。平台自有源码、schema 24、OAuth2 会话和两阶段同步契约不变。15 个冲突的逐项处理、实际测试、未验收与回退见 [同步报告](../upstream-sync/2026-11-06.md)。

## 2026-11-03：合并上游 v4.0.25

依据 [ADR-0011](../adr/ADR-0011-merge-4.0.25.md)，从 4.0.23 三方合入官方 v4.0.25，保留双方祖先关系。复用上游 Provider、CLI 投影、MCP、代理、用量、DAO、IPC 和原子写入，平台业务继续位于自有命名空间；本次没有新增平台业务核心补丁。

| ID / 既有补丁 | 上游文件 | 复核与不能仅包装的原因 | 等级 | 验证 |
| --- | --- | --- | --- | --- |
| SYNC-20261103-01 / ZS-044、ZS-046、ZS-047、ZS-148 | package / Cargo 清单及锁文件、Tauri 配置 | 工具固定入口必须保留智枢 0.0.6 与身份，同时接入上游依赖；锁文件由工具更新 | L1 | frozen install、Cargo check/Clippy、发布脚本 |
| SYNC-20261103-02 / COEXIST-20261103-01、02 | `live/project/claude.rs`、`mode/{controller,stack}.rs`、`claude_desktop_config.rs`、`services/provider/mod.rs` | 投影和内部调用须吸收上游模型说明及 AUTH_TOKEN 修复，保留智枢占位符、Stack/Profile 和写入归属，纯外层包装不能替换核心调用；冲突测试采用双方语义 | L3 | Claude/模式/Provider/共存及金标回归 |
| SYNC-20261103-03 | `.github/dependabot.yml`、`assets/partners/logos/dit-banner.png` | 仓库规则要求保留上游文件；保留未引用资源与空 updates 配置，复用上游撤下赞助/预设及停用更新行为 | L1 | 文件/引用审查、前端全量与 renderer |
| SYNC-20261103-04 / ZS-158 | `src/App.tsx` | 完整格式检查发现两处既存 Prettier 问题，只修正换行，不改挂载或运行行为；包装无法修正固定入口的格式 | L1（纯格式） | typecheck、format:check、前端全量 |
| SYNC-20261103-05 | `tests/msw/handlers.ts` | 共享 mock 固定数组同时保留智枢配置历史与上游 models.dev IPC，新下载路径不再使用浏览器 HTTP mock | L0 测试 | 292 文件、3742 项前端测试 |

既有 lib 注册、设置、MCP/DSH、代理转发及五语补丁均保留，新增通用功能按上游实现吸收，不另建平行服务。数据库 schema 24 和 OMP 协调、OAuth2 凭据/平台 API、产品更新渠道不变。实际验证、未验收和回退见 [同步报告](../upstream-sync/2026-11-03.md)。

## 2026-11-03：移除自有 macOS 开发签名辅助

用户明确要求删除三份自有开发签名脚本及测试，同时保留 OAuth2 自动恢复。删除 `scripts/zhishu/dev-macos.sh`、`macos-dev-sign.mjs`、`macos-dev-sign.test.mjs` 和失去入口的开发签名文档；清理命令和当前文档引用，更新既有 ADR-0010。复用上游 Tauri CLI 开发启动与现有 Rust `platform_session` 会话存储，不新增包装或替代脚本。

自有脚本/文档删除 L0；固定 `package.json` 删除 `test:dev-signing` 按 DEV-20261103-01 追加登记（L1，外层 Adapter 无法撤销 package 命令），AGENTS 维护入口 L1 文档。没有更改 Rust、会话 schema、keyring 后端、产品身份或正式发布签名。macOS 开发版重编译后可能重新要求钥匙串授权，属于移除辅助签名的边界；实际检查与未执行项见[验证记录](../history/validation.md)。回退仅恢复本轮文件与命令，无凭据或配置迁移。以下事件保留当时实施事实。

## 2026-11-03：清理本地账号密码脚本

用户要求删除各平台本地账号密码相关脚本与代码，并明确确认保留 OAuth2 自动恢复。删除自有 `scripts/zhishu/fixtures/macos-keychain-probe.c` 及签名测试中的真实钥匙串读写/清理流程和 opt-in 环境变量入口，保留模拟签名回归与代码签名脚本。Windows/Linux 平台密码存储已随前轮历史模块删除，不新增兼容或清理脚本。复用现有 OAuth2 会话与系统 keyring，仅保存 refresh token；不改上游授权中心、CLI 凭据或真实用户数据。

自有脚本/测试/文档 L0；修正五语 `platformCleanup.retainedDetails` 中遗漏的登录历史/记住密码说明，沿用 AUTH-CLEANUP-20261103-02（L1，共享字典入口不能由自有包装替代）；AGENTS 更新沿用 AUTH-CLEANUP-20261103-03（L1 文档）。不涉及核心服务或数据 schema，无需 L3 ADR。回退只恢复本轮自有探针/测试及文案，不迁移凭据。验证与剩余平台验收范围见[验证记录](../history/validation.md)。

## 2026-11-03：彻底删除密码登录遗留代码

用户进一步明确要求删除所有账号/邮箱密码登录相关代码，替代上一轮保留旧密码兼容层的边界。删除自有 `platform_login_history.rs` 及其全部密码存取、遗忘/删除兼容、测试辅助和账号历史；OAuth2 不再更新历史，登录页只保留授权与服务器地址设置。前后端 `/config` DTO 删除密码/注册/找回/TOTP/验证码组件字段，保留 OAuth2 与订阅地址。通用日志脱敏、平台账户邮箱展示及授权中心独立认证继续按各自职责使用。

| ID | 文件 | 改动与不能仅新增/包装的原因 | 等级 | 验证入口 |
| --- | --- | --- | --- | --- |
| AUTH-CLEANUP-20261103-01 | `src-tauri/src/lib.rs` | 固定注册表删除 `platform_login_history`、`platform_forget_password`、`platform_remove_login`；外层包装无法撤销 IPC | L1 | Rust 编译与平台回归 |
| AUTH-CLEANUP-20261103-02 | `src/i18n/locales/{zh,zh-TW,en,ja,tr}.json` | 删除平台命名空间中废弃密码表单/历史文案，更新清理范围说明；共享语言包是运行时固定入口 | L1 | TypeScript、Prettier 与登录/额度组件回归 |
| AUTH-CLEANUP-20261103-03 | `AGENTS.md` | 按本次明确要求更新固定维护入口的旧凭据与历史边界 | L1 文档 | 实现与引用核对 |

其余为自有业务、测试和文档 L0 清理，不修改上游 Provider、CLI 配置或系统凭据库实现。复用 OAuth2 state/PKCE/回环和现有会话恢复、原子会话元数据写入；不改变会话 schema、keyring service 或登录令牌持久化语义。旧密码 keyring 条目和历史文件保留原处，本程序不再访问，不做自动迁移或真实数据清理；回退需恢复被删模块及其注册/前端调用。验证结果及未验收项见[验证记录](../history/validation.md)。

## 2026-11-03：平台仅保留 OAuth2 登录

依据用户明确要求移除桌面账号密码登录。自有登录 UI/API、Rust 认证服务及历史存储（L0）移除密码提交、客户端 TOTP、密码读取和密码接口白名单；复用现有 OAuth2 PKCE/state/回环授权、账户验证、刷新令牌存储及恢复。OAuth2 成功只更新账号/服务器历史，保留旧密码元数据与凭据，删除记录仍调用既有清理路径；不迁移真实数据，不改平台服务端或授权中心。

| ID | 文件 | 改动与不能仅新增/包装的原因 | 等级 | 验证入口 |
| --- | --- | --- | --- | --- |
| AUTH-20261103-01 | `src-tauri/src/lib.rs` | 从固定 handler 表撤销 `platform_login`、`platform_login_2fa`、`platform_saved_password`；包装 UI 不能撤销 IPC 入口 | L1 | Rust 编译、平台认证/会话回归 |
| AUTH-20261103-02 | `src/i18n/locales/{zh,zh-TW,en,ja,tr}.json` | 固定共享语言包添加 OAuth2 不可用、重试与历史选择说明，更新会话提示和返回登录文案 | L1 | TypeScript、格式和登录组件测试 |
| AUTH-20261103-03 | `AGENTS.md` | 更新固定维护入口的登录契约与旧密码清理边界，旁路文档无法撤回过期指令 | L1 文档 | 路径与现有实现核对 |

验收：平台缺少 OAuth2 配置或配置失败时阻止登录且不回退，允许重试和修改服务器；覆盖授权成功、失败、取消、迟到结果、历史记录、重启恢复及退出清理。数据 schema、keyring service 和旧历史文件格式不变，可通过恢复本次代码回退；不需要 L3 核心 ADR。实际执行与真实浏览器/跨平台未验收项见[验证记录](../history/validation.md)。

## 2026-11-03：自有发布与签名脚本去冗余

按用户确认清理 `scripts/zhishu/`：从 `release-artifacts.mjs` 删除没有生产调用的 `configFingerprint`、`walkFiles`、`oneFile`、`saveReceipt`、`validateReceipt`、`assembleRelease` 及无用依赖；删除两项仅验证旧合包/`build.json` 链路的测试，现用夹具不再生成旧收据，缺失签名回归直接调用 `collectUpdaterPlatforms`。当前版本/目标/签名/哈希与历史发布恢复校验继续复用现有实现。

将 `macos-dev-sign-runner.sh` 合并到既有 `dev-macos.sh --run` 分支并删除独立文件；签名 helper、身份选择、签名失败停止、应用参数与退出码保持原约定，已有测试增加启动入口与共享 runner 的接线验证。目录由 20 个文件减为 19 个，不新增脚本或测试文件。

本轮均为自有脚本/测试与文档的 L0 清理，没有修改上游文件、package 命令、工作流或签名策略，无新增上游补丁或 ADR。回退只恢复这些自有脚本及测试，不移动版本、产物、tag 或真实凭据；实际检查见[验证记录](../history/validation.md)。

## 2026-11-03：移除开发启动隔离

依据用户明确要求及 [ADR-0010](../adr/ADR-0010-remove-development-isolation.md) 撤回开发隔离。复用既有 Tauri CLI、macOS 签名 runner、正常目录解析、设置覆盖、测试隔离和受控写入；不删除上游能力，不迁移或清理真实数据。

| ID | 文件 | 改动与不能仅新增/包装的原因 | 等级 | 验证入口 |
| --- | --- | --- | --- | --- |
| DEV-20261103-01 | `package.json` | 固定 `dev` 命令直接复用 `tauri dev`，删除开发隔离测试入口；按用户反馈不保留通用包装脚本 | L1 | 命令指向、Tauri CLI 帮助、已有签名回归 |
| DEV-20261103-02 | `src-tauri/src/config.rs`、`codex_config.rs` | 撤回函数内部优先执行的开发目录分支，保留测试 home、用户设置和平台兼容路径；外层 Adapter 无法撤回强制覆盖 | L3 | config 路径回归、Provider、golden |
| DEV-20261103-03 | `src-tauri/src/lib.rs` | 删除固定启动流程中已失效的隔离日志 | L1 | Rust 编译与相关回归 |
| DEV-20261103-04 | `src-tauri/src/live/engine.rs`、`AGENTS.md` | 删除已过期的开发隔离说明，保留设备状态目录语义与测试隔离要求 | L0 文档 | 源码引用及文档链接核对 |

删除自有 `scripts/zhishu/dev-isolated{,.test}.mjs`，不保留通用启动器和对应测试；macOS 签名显式复用既有 `dev-macos.sh`。`zhishu/storage/platform_storage.rs` 移除开发分支，统一 Tauri 本地目录；`platform_session.rs` 仅更新目录摘要注释，凭据 ID 与 keyring 协议不变。当前操作文档改为[开发启动](development.md)，历史测试和迁移记录保留原事实。原 ZS-106、ZS-111 等累积台账中的“开发隔离”仅记录当时补丁，本事件撤回其专用运行时分支。

验收和未执行项见[验证记录](../history/validation.md)。回退仅恢复代码与旧入口，保留旧开发目录和正常目录各自数据，不做全目录覆盖或凭据迁移。

## 2026-11-03：按二开标准统一 Git 远程命名

用户授权整理远程名称和地址。旧 `origin` → `upstream`（官方）、旧 `zhishu-git` → `origin`（智枢 GitHub）、旧 `zhishu-origin` → `gitlab`（内网 GitLab），URL 保持原值。复用 Git 原生 `remote rename`、既有发布脚本和 URL 白名单；发布脚本/回归位于自有 `scripts/zhishu/`，L0，仍校验唯一 push URL、原子推送独立分支与 tag。根 `AGENTS.md` 仅更新固定维护入口中的远程说明（L1），新增旁路文档无法消除该入口中的过期指令；其他修订为自有标准、基线、发布指南及验证记录（L0）。没有上游运行时代码补丁，无需新增 L3 ADR。

远程改名保留分支与 tag、HEAD、历史祖先关系和原分支跟踪目标。最终复核发现并行操作将 `ccs-dev` 改为跟踪 `origin/ccs-dev` 并更新该缓存引用，已保留并记录。本任务未联网、推送、发布、处理 GitLab 分歧或变更凭据/应用数据；历史发布和同步报告保留当时旧名。回退命令及当前地址见 [基线](baseline.md)，验证与未执行项见 [验证记录](../history/validation.md)。

## 2026-11-02：接入向导识别本机 WorkBuddy

修复「接入到工具」向导把所有非 CLI 应用误判为未安装的问题。WorkBuddy 是桌面应用，没有可执行的 CLI，原检测只调用 `listToolInstallations`（CLI 工具）和 `getClaudeDesktopStatus`，因此即使本机已安装也始终显示「未安装」。

| ID | 上游文件 | 原因 | 为什么不能仅新增/包装 | 级别 | 对应测试 | 最后复核 |
|---|---|---|---|---|---|---|
| WKB-20261102-01 | `src-tauri/src/commands/provider.rs`、`src-tauri/src/lib.rs` | 新增只读 `get_workbuddy_installed` 命令并注册 | Tauri 命令必须加入固定 `generate_handler!`，不能由自有模块替代 | L1 | Rust `workbuddy_config` 同文件测试 | 2026-11-02 |
| WKB-20261102-02 | `src/lib/api/providers.ts` | 增加 `getWorkBuddyInstalled` IPC 封装 | 复用上游统一 `invoke` 入口 | L1 | PlatformOnboarding 测试、typecheck | 2026-11-02 |

检测实现位于自有 `src-tauri/src/zhishu/integrations/ccswitch/workbuddy_config.rs`（按 `.workbuddy` 数据目录或 macOS `WorkBuddy.app` 推断，复用 `get_workbuddy_home()` 的覆盖与环境变量优先级），接线位于自有 `src/zhishu/onboarding/PlatformOnboarding.tsx`。只读探测，不写配置、不联网、不改 Provider 或平台目录；只在向导同时勾选 WorkBuddy 时调用。回退需删除新命令、注册、前端封装和向导分支。

本轮验证（2026-11-02）：`pnpm typecheck`、`tests/zhishu/components/PlatformOnboarding.test.tsx` 8 项、`cargo fmt --check`、`cargo clippy -D warnings` 及 `workbuddy_config` 6 项 Rust 测试通过；未在 macOS 真机自动化中复验向导 UI，未覆盖 Windows/Linux 仅凭数据目录的识别。

## 2026-11-02：处理平台 OAuth 唤醒深链接

平台授权成功页会先尝试打开 `zhishu-ccswitch://v1/oauth` 拉起客户端，再向本地回环地址投递授权码。原深链接处理只接受 `/import`，会把唤醒链接当成导入请求解析并弹出「Invalid path: expected '/import', got '/oauth'」。本次在深链接入口识别该唤醒链接：只聚焦主窗口，不生成待处理导入；授权码仍只从回环接收，不改变 OAuth 的 state/PKCE 校验。

| ID | 上游文件 | 原因 | 为什么不能仅新增/包装 | 级别 | 对应测试 | 最后复核 |
|---|---|---|---|---|---|---|
| OAUTH-20261102-01 | `src-tauri/src/deeplink/parser.rs`、`src-tauri/src/deeplink/mod.rs` | 新增 `is_oauth_wakeup` 识别 `v1/oauth` 唤醒链接 | 深链接解析是上游固定入口，自有模块无法在解析前拦截 | L1 | `deeplink` 同模块测试 | 2026-11-02 |
| OAUTH-20261102-02 | `src-tauri/src/lib.rs` | `handle_deeplink_url` 对唤醒链接跳过导入暂存与事件 | 深链接分发位于上游固定处理函数，Tauri 注册表无法替换 | L1 | Rust `deeplink` 测试、clippy | 2026-11-02 |

复用现有深链接注册、窗口聚焦和 OAuth 回环监听；不新增 IPC、不写配置、不记录 code/state。回退需删除识别函数和分支，恢复按导入解析。

本轮验证（2026-11-02）：`cargo fmt --check`、`cargo clippy -D warnings`、`deeplink` 解析测试通过；未在真实浏览器中重跑平台授权链路，未覆盖 Safari 等对 loopback/深链更严格的浏览器。

## 2026-11-01：平台接入向导与语言包新增键

复核 ZS-190（L1）：`src/i18n/locales/{zh,zh-TW,en,ja,tr}.json` 新增 `platformOnboarding.*` 键，并更新 `platformCatalog.description` / `platformCatalog.success` 文案，说明登录后不再自动写入供应商列表、改用「接入到工具」向导。共享语言包是运行时固定入口，新增文案无法只放在自有模块里；实现和接入逻辑在 `src/zhishu/onboarding/PlatformOnboarding.tsx`、`src/zhishu/auth/PlatformAuthContext.tsx` 与 `src/zhishu/models/components/PlatformCatalogPage.tsx`（均为自有文件）；`src/main.tsx` 仅新增 `PlatformOnboardingProvider` 包裹（L1 薄接线）。

新增只读命令 `platform_preview_group_options({ agent })`（L2，自有模块 `zhishu/services/platform_models/mod.rs`），从本机快照列出某工具可用的分组，供接入向导选择分组；不写供应商、不联网。`src-tauri/src/lib.rs` 的 `generate_handler!` 仅新增这一行注册，属于上游薄接线；复用既有 `platform_sync_providers`、`switch_provider`、`set_proxy_takeover_for_app`、`set_proxy_stack_member` 和 `update_tray_menu`，未改 schema、凭据或存储。同时删除「全部同步平台供应商」入口和 `platform_sync_providers` 的 `agent: null` 分支，命令改为必须指定工具；各应用页的单工具同步按钮保留。语言包回归入口为 `tests/config/localeCoverage.test.ts`，接入向导回归为 `tests/zhishu/components/PlatformOnboarding.test.tsx`，Rust 分组预览用例在 `platform_models/mod.rs` 同文件测试。回退需删除新增键、命令和注册行，并恢复全部应用同步入口；不影响已保存的数据。

## 2026-11-01：macOS 发布 runner 迁移

复核 ZS-003（L1）：`.github/workflows/release.yml` 的 `macos-binary` 和 `macos-release` 从 `macos-14` 固定迁移到 `macos-15`。GitHub 在退役前安排 macOS 14 brownout，2026-11-02 起停用该镜像，已导致 `v0.0.4` 的 macOS 作业无法启动；官方说明见 [runner-images #13518](https://github.com/actions/runner-images/issues/13518)。`runs-on` 是 Actions 调度入口，新增脚本或 Adapter 无法在作业启动前替换镜像，故必须修改既有工作流。

复用原有 ARM64 runner、Rust 双目标编译、universal 合包、签名/公证与 COS → GitHub Release 链路；客户端最低 macOS 版本仍为 `12.0`。本次仅替换两处 runner 标签，不变更应用身份、签名凭据、业务数据、版本或已发布 tag。镜像更新涉及 Xcode/SDK 环境差异，实际跨架构构建、签名与安装升级须由后续演练和实机验收确认；回退需选择仍受支持的 runner，不能恢复即将退役的标签。

本轮验证（2026-11-01）：YAML 解析及改动前后结构比对通过，语义差异仅为两个 runner；双目标矩阵、发布依赖/门禁、签名步骤和最低系统版本保持一致。`pnpm test:release` 58 项全部通过，`git diff --check` 通过。未执行真实 Actions/macOS 构建、Apple 签名/公证及实机安装升级；未运行与本次工作流标签无关的前端/Rust 全量检查。本次不触发正式发布。

## 2026-11-01：版本递增与 tag 发布脚本

复核 ZS-044（L1）：`package.json` 仅增加 `release:tag`、`test:release-tag` 命令，并把新回归接入 `test:release`。pnpm 的命令发现依赖该固定入口，不能仅新增自有脚本替代；实现与测试均新增在 `scripts/zhishu/tag-release{,.test}.mjs`（L0），操作说明更新于[发布文档](../releases/README.md)。

复用 `cos-release.mjs::checkVersion`、`release-artifacts.mjs::versionNumber`、Cargo 锁文件同步和既有 Actions；不修改发布工作流、客户端更新协议/身份、凭据或业务存储。支持补丁/次/主版本递增、指定稳定 tag 和离线预览；检查通过后只提交版本文件及公告，向核实的 `zhishu-git` 原子推送独立发布分支与 tag。失败保留文件、提交和 tag，按日志重试原引用，不强推或移动已发布 tag。

验证入口为 `pnpm test:release-tag`（临时 Git/bare 仓库与 Cargo 离线用例）及既有发布回归；本轮仅实现/验证脚本，不触发真实 GitHub/COS 发布，不将模拟推送视为跨平台安装与更新验收。

本轮执行记录（2026-11-01）：提交前在独立基线上执行 TypeScript `--noEmit` 和完整 Node 发布回归，类型检查通过、58 项回归全部通过（含新增脚本 22 项，覆盖指定 tag、仅发送本次 tag、合并中止、带空格路径、真实 Cargo 离线同步）。脚本 Prettier、CLI 指定 tag 预览及 `git diff --check` 通过。COS Python 回归此前共执行 23 项，21 项通过、2 项因本机缺 COS SDK 跳过。原共享工作区的首次类型检查曾因并行上游合并引入的 `yaml` 未安装而失败，独立提交未混入该合并或该依赖。完整前端/Rust 测试、构建、真实 GitHub/COS 发布和 Windows/macOS 安装升级未执行。

## 2026-10-31：撤回 DeepSeek 遗留预设分组补丁

用户确认按审查建议移除无效的 `deepseek-official` 特判；“浏览目录”快捷入口按用户要求保留。

| 文件 | 当前处理及必要性 | 等级 | 验证 |
|---|---|---|---|
| `src/components/providers/forms/presetGroups.ts` | 撤回二开新增的一行 ID 特判，完整恢复上游 4.0.22（`b7642674`）。现有预设没有该 ID；DeepSeek API 预设沿用 `cn_official → vendor`，DSH 账号入口沿用 `official → login`。新增 Adapter 无需介入，直接复用原分类函数 | L2 补丁撤回 | 本轮执行与未验证项见[验证记录](../history/validation.md) |

只减少无调用对象的上游差异，不新增依赖、IPC、schema 或配置写入；无数据、安全凭据及跨平台存储变更。回退仅恢复该行自有判断，不涉及数据迁移。

## 2026-10-31：平台与上游边界收敛

决策见 [ADR-0008](../adr/ADR-0008-upstream-patch-convergence.md)。本段覆盖下方历史条目对应能力的当前状态；历史表不改写。

| 文件/既有补丁 | 当前处理及必要性 | 等级 | 回归入口 |
|---|---|---|---|
| `src-tauri/src/live/project/claude.rs`（ZS-123）、`services/provider/claude_direct.rs`（ZS-138）、`services/provider/claude_editor.rs`（ZS-139） | 完整恢复 4.0.22 上游；平台已在自有 Adapter 组装 Haiku/Sonnet/Opus，无需通用派生 | L3 补丁撤回 | 手动 Provider 切换/编辑预览、平台 Claude 原生切换、Claude 金标 |
| `src-tauri/src/mode/controller.rs`（ZS-125）、`services/provider/mod.rs`（ZS-142） | 撤回直连写入的全局档位派生；保留历史审计、WorkBuddy 与身份隔离；服务新增通用元数据入口 | L3 | Provider、mode 与历史还原 |
| `src-tauri/src/{opencode_config,kimi_config,mcode_config}.rs`、`pi_config/mod.rs` | 完整恢复 4.0.22 上游；平台专用测试夹具移到 `zhishu/integrations/ccswitch/test_support.rs`；Kimi 选择解析通过 Adapter 复用原生 `read/config_path` | L3 补丁撤回 | 平台全应用同步、删除、退出、Kimi 别名 |
| `src/components/settings/sections/GeneralSection.tsx`（ZS-175）、`src/components/providers/presentation.ts`（ZS-170）、`src/lib/api/index.ts` | 完整恢复 4.0.22 上游；撤下无调用者的 simplified、无赋值的 deleteReason 及自有 API 重导出；配置历史继续使用自有模块入口 | L2 补丁撤回 | Settings、卡片呈现、配置历史、类型检查 |
| `src/lib/query/mutations.ts`（ZS-201） | 保留 WorkBuddy 分派；成功沿用上游刷新，失败补刷一次，避免成功时顺序双刷 | L2 | `tests/zhishu/hooks/useWorkBuddyMutationRefresh.test.tsx` 八项成功/失败路径 |
| `src-tauri/src/services/provider/mod.rs::update_metadata`、`database/dao/providers.rs::update_provider_metadata` | 唯一新增的通用核心能力：复用应用锁，连接锁内读取最新原始 JSON，仅更新 meta，不插入、不写 CLI；上游现有 update 会发布配置，故不能直接用于元数据迁移。平台归属和订阅判断留在 Adapter | L3 | 元数据新鲜读取、未知字段保留、缺失项、错误不落盘、应用锁等待、原生文件不变 |

完整配置历史和还原、完整 WorkBuddy 原生支持继续保留。Tauri 事件监听清理竞态、macOS Dock 图标恢复、OpenCode V2 用量凭据读取按独立修复补丁保留，本次不将其归入平台同步。此前 DSH 修复已单独提交为 `2fea90d7`，继续复用上游原生服务。

本次不改 schema、IPC、凭据和数据目录，不覆盖正在整理的文档或开发签名改动；配置写入仍走已有锁、备份、原子发布/pending 和 WorkBuddy 局部恢复。已执行结果和未验收范围见 [验证记录](../history/validation.md)。

## 2026-10-31：4.0.22 冲突复核

以 `4.0.22` (`b7642674`) 为新上游基线；逐项决议及验证见 [同步报告](../upstream-sync/2026-10-31.md) 与 [ADR-0007](../adr/ADR-0007-merge-4.0.22.md)。本轮没有新增平行 Provider 核心。

| 既有补丁 | 决议 | 侵入等级 | 复核 |
|---|---|---|---|
| ZS-103、ZS-107、ZS-130、ZS-133、ZS-141、ZS-142 | 保留 WorkBuddy 路径和配置历史包装；在现有枚举与分派补 DSH，复用上游 `dsh_config` | L3 | Rust 编译、Provider/DSH 集成、库测试 |
| ZS-116 | 保留 v23 OMP 幂等协调，吸收上游 v24 DSH/游标迁移；旧测试改为验证当前 schema 和标记保留 | L3 | 数据库迁移回归 |
| ZS-148 及清单身份补丁 | 保留智枢 `0.0.3`、包名、签名/更新身份；只吸收上游依赖 | L1 | 锁文件编译、发行配置静态核对 |
| ZS-158 | 保留平台目录/同步/删除、配置历史入口及分支已有事件 Hook；恢复上游 `refreshAdditiveLiveState`，撤下页面中对自有 Helper 的重复调用 | L2 | App 集成测试 |
| ZS-164 | 保留上游归属判断、读取失败提示及 DSH 卡片，补 WorkBuddy 分支；自有 Hook 仅供平台批量按钮 | L2 | ProviderList 与平台回归 |
| ZS-190 等五份语言包 | 按 JSON 键补齐两侧缺失的顶层翻译，保留现有译文 | L1 | JSON 解析、前端回归 |

## 2026-10-29 归位后的完整差异清单

基线：官方 4.0.21 `c4db63c60d2a1ddcb4bd9f81152b9c6126413659`；范围：本次整理后的最终索引相对基线的已有上游文件差异。
下表逐文件覆盖实际 M/R/D 差异（含资源、测试、文档），新增自有文件见 ADR-0002 迁移表及目录 README。
并非所有行都由本次新增；“修改目的”依据原功能提交归组，历史核心扩展继续登记。
共 222 条。重新核对命令：`git diff --name-status --find-renames c4db63c6 HEAD`，排除 A 项。

对应本地功能整理：`feat/zhishu-structure-cleanup`；尚未创建 PR/Issue，不虚构编号。
合并前应补充实际 PR 链接。所有测试状态以 [验证记录](../history/validation.md) 为准，表内是关联验证范围。
L3 的统一依据见 [ADR-0002](../adr/ADR-0002-structure-and-history.md)，数据库另见 ADR-0001。

| ID | 上游文件 | 修改目的 | 为什么不能仅新增/包装 | 级别 | 对应测试 | 最后复核 |
|---|---|---|---|---|---|---|
| ZS-001 | `.github/actions/prepare-tauri-signing-key/action.yml` | 开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-002 | `.github/workflows/ci.yml` | 开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-003 | `.github/workflows/release.yml` | 开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-004 | `.github/workflows/sync-r2.yml` | 开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-005 | `.gitignore` | 品牌/共存；开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-006 | `CONTRIBUTING.md` | 平台目录/供应商；开发/发行 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-007 | `README.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-008 | `README_JA.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-009 | `README_ZH.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-010 | `docs/guides/claude-codex-routing-guide-en.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-011 | `docs/guides/claude-codex-routing-guide-ja.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-012 | `docs/guides/claude-codex-routing-guide-zh.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-013 | `docs/guides/codex-claude-routing-guide-en.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-014 | `docs/guides/codex-claude-routing-guide-ja.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-015 | `docs/guides/codex-claude-routing-guide-zh.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-016 | `docs/guides/codex-deepseek-routing-guide-en.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-017 | `docs/guides/codex-deepseek-routing-guide-ja.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-018 | `docs/guides/codex-deepseek-routing-guide-zh.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-019 | `docs/guides/codex-kimi-routing-guide-en.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-020 | `docs/guides/codex-kimi-routing-guide-ja.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-021 | `docs/guides/codex-kimi-routing-guide-zh.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-022 | `docs/guides/codex-official-auth-preservation-guide-en.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-023 | `docs/guides/codex-official-auth-preservation-guide-ja.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-024 | `docs/guides/codex-official-auth-preservation-guide-zh.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-025 | `docs/user-manual/en/1-getting-started/1.4-quickstart.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-026 | `docs/user-manual/en/2-providers/2.6-claude-desktop.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-027 | `docs/user-manual/en/4-proxy/4.1-service.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-028 | `docs/user-manual/en/4-proxy/4.2-routing.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-029 | `docs/user-manual/en/5-faq/5.2-questions.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-030 | `docs/user-manual/ja/1-getting-started/1.4-quickstart.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-031 | `docs/user-manual/ja/2-providers/2.6-claude-desktop.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-032 | `docs/user-manual/ja/4-proxy/4.1-service.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-033 | `docs/user-manual/ja/4-proxy/4.2-routing.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-034 | `docs/user-manual/ja/5-faq/5.2-questions.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-035 | `docs/user-manual/zh/1-getting-started/1.4-quickstart.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-036 | `docs/user-manual/zh/2-providers/2.6-claude-desktop.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-037 | `docs/user-manual/zh/4-proxy/4.1-service.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-038 | `docs/user-manual/zh/4-proxy/4.2-routing.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-039 | `docs/user-manual/zh/5-faq/5.2-questions.md` | 品牌/共存 | 原文入口、路径或示例须与 Fork 实际身份/端口及脚本一致，新增文档不能修复旧链接 | L0 | 路径审查；文档人工复核 | 2026-10-29 / ADR-0002 |
| ZS-040 | `flatpak/README.md` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-041 | `flatpak/com.ccswitch.desktop.desktop` → `flatpak/com.zhishu.ccswitch.desktop.desktop` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-042 | `flatpak/com.ccswitch.desktop.metainfo.xml` → `flatpak/com.zhishu.ccswitch.desktop.metainfo.xml` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-043 | `flatpak/com.ccswitch.desktop.yml` → `flatpak/com.zhishu.ccswitch.desktop.yml` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-044 | `package.json` | 品牌/共存；开发隔离；开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-045 | `scripts/generate-download-manifest.mjs` | 开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-046 | `src-tauri/Cargo.lock` | 品牌/共存；认证会话；开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-047 | `src-tauri/Cargo.toml` | 品牌/共存；开发/发行；认证会话 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-048 | `src-tauri/Info.plist` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-049 | `src-tauri/build.rs` | 品牌/共存；开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-050 | `src-tauri/icons/128x128.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-051 | `src-tauri/icons/128x128@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-052 | `src-tauri/icons/32x32.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-053 | `src-tauri/icons/64x64.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-054 | `src-tauri/icons/Square107x107Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-055 | `src-tauri/icons/Square142x142Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-056 | `src-tauri/icons/Square150x150Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-057 | `src-tauri/icons/Square284x284Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-058 | `src-tauri/icons/Square30x30Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-059 | `src-tauri/icons/Square310x310Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-060 | `src-tauri/icons/Square44x44Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-061 | `src-tauri/icons/Square71x71Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-062 | `src-tauri/icons/Square89x89Logo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-063 | `src-tauri/icons/StoreLogo.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-064 | `src-tauri/icons/android/mipmap-hdpi/ic_launcher.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-065 | `src-tauri/icons/android/mipmap-hdpi/ic_launcher_foreground.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-066 | `src-tauri/icons/android/mipmap-hdpi/ic_launcher_round.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-067 | `src-tauri/icons/android/mipmap-mdpi/ic_launcher.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-068 | `src-tauri/icons/android/mipmap-mdpi/ic_launcher_foreground.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-069 | `src-tauri/icons/android/mipmap-mdpi/ic_launcher_round.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-070 | `src-tauri/icons/android/mipmap-xhdpi/ic_launcher.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-071 | `src-tauri/icons/android/mipmap-xhdpi/ic_launcher_foreground.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-072 | `src-tauri/icons/android/mipmap-xhdpi/ic_launcher_round.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-073 | `src-tauri/icons/android/mipmap-xxhdpi/ic_launcher.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-074 | `src-tauri/icons/android/mipmap-xxhdpi/ic_launcher_foreground.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-075 | `src-tauri/icons/android/mipmap-xxhdpi/ic_launcher_round.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-076 | `src-tauri/icons/android/mipmap-xxxhdpi/ic_launcher.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-077 | `src-tauri/icons/android/mipmap-xxxhdpi/ic_launcher_foreground.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-078 | `src-tauri/icons/android/mipmap-xxxhdpi/ic_launcher_round.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-079 | `src-tauri/icons/icon.icns` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-080 | `src-tauri/icons/icon.ico` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-081 | `src-tauri/icons/icon.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-082 | `src-tauri/icons/ios/AppIcon-20x20@1x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-083 | `src-tauri/icons/ios/AppIcon-20x20@2x-1.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-084 | `src-tauri/icons/ios/AppIcon-20x20@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-085 | `src-tauri/icons/ios/AppIcon-20x20@3x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-086 | `src-tauri/icons/ios/AppIcon-29x29@1x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-087 | `src-tauri/icons/ios/AppIcon-29x29@2x-1.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-088 | `src-tauri/icons/ios/AppIcon-29x29@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-089 | `src-tauri/icons/ios/AppIcon-29x29@3x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-090 | `src-tauri/icons/ios/AppIcon-40x40@1x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-091 | `src-tauri/icons/ios/AppIcon-40x40@2x-1.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-092 | `src-tauri/icons/ios/AppIcon-40x40@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-093 | `src-tauri/icons/ios/AppIcon-40x40@3x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-094 | `src-tauri/icons/ios/AppIcon-512@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-095 | `src-tauri/icons/ios/AppIcon-60x60@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-096 | `src-tauri/icons/ios/AppIcon-60x60@3x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-097 | `src-tauri/icons/ios/AppIcon-76x76@1x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-098 | `src-tauri/icons/ios/AppIcon-76x76@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-099 | `src-tauri/icons/ios/AppIcon-83.5x83.5@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-100 | `src-tauri/icons/tray/macos/statusTemplate.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-101 | `src-tauri/icons/tray/macos/statusTemplate@2x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-102 | `src-tauri/icons/tray/macos/statusbar_template_3x.png` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-103 | `src-tauri/src/app_config.rs` | 品牌/共存；工具扩展 | 存量 AppType、目录字段、默认值及序列化别名贯穿上游匹配，包装无法增补枚举 | L3 | 配置加载/目录设置/工具回归 | 2026-10-29 / ADR-0002 |
| ZS-104 | `src-tauri/src/auto_launch.rs` | 品牌/共存 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-105 | `src-tauri/src/claude_desktop_config.rs` | 品牌/共存；配置历史 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-106 | `src-tauri/src/codex_config.rs` | 开发隔离 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-107 | `src-tauri/src/commands/config.rs` | 工具扩展 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-108 | `src-tauri/src/commands/misc.rs` | 撤销未使用的平台启动扩展；保留便携更新 TODO | 新增 Adapter 无法消除原文件中无调用方函数的 dead-code 告警；本轮删除 76 行旧二开代码，相对 4.0.21 仅剩 TODO 注释，无运行逻辑差异 | L1 | 引用搜索；Rust 全量回归与严格 clippy，结果见历史验证记录 | 2026-10-29 / 历史验证记录 |
| ZS-109 | `src-tauri/src/commands/provider.rs` | 工具扩展 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-110 | `src-tauri/src/commands/settings.rs` | 品牌/共存 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-111 | `src-tauri/src/config.rs` | 品牌/共存；开发隔离；配置历史 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-112 | `src-tauri/src/database/backup.rs` | 品牌/共存 | 既有 schema/默认端口/DAO 列兼容必须在实际数据库打开和升级路径执行；沿用 ADR-0001，不新增迁移 | L3 | schema 备份、幂等、失败回滚测试 | 2026-10-29 / ADR-0002 |
| ZS-113 | `src-tauri/src/database/dao/mcp.rs` | 工具扩展 | 既有 schema/默认端口/DAO 列兼容必须在实际数据库打开和升级路径执行；沿用 ADR-0001，不新增迁移 | L3 | schema 备份、幂等、失败回滚测试 | 2026-10-29 / ADR-0002 |
| ZS-114 | `src-tauri/src/database/dao/proxy.rs` | 品牌/共存 | 既有 schema/默认端口/DAO 列兼容必须在实际数据库打开和升级路径执行；沿用 ADR-0001，不新增迁移 | L3 | schema 备份、幂等、失败回滚测试 | 2026-10-29 / ADR-0002 |
| ZS-115 | `src-tauri/src/database/mod.rs` | 品牌/共存 | 既有 schema/默认端口/DAO 列兼容必须在实际数据库打开和升级路径执行；沿用 ADR-0001，不新增迁移 | L3 | schema 备份、幂等、失败回滚测试 | 2026-10-29 / ADR-0002 |
| ZS-116 | `src-tauri/src/database/schema.rs` | 品牌/共存 | 既有 schema/默认端口/DAO 列兼容必须在实际数据库打开和升级路径执行；沿用 ADR-0001，不新增迁移 | L3 | schema 备份、幂等、失败回滚测试 | 2026-10-29 / ADR-0002 |
| ZS-117 | `src-tauri/src/deeplink/parser.rs` | 品牌/共存 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-118 | `src-tauri/src/deeplink/provider.rs` | 工具扩展 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-119 | `src-tauri/src/deeplink/tests.rs` | 品牌/共存 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-120 | `src-tauri/src/kimi_config.rs` | 平台目录/供应商 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-121 | `src-tauri/src/lib.rs` | 品牌/共存；开发隔离；工具扩展；认证会话；配置历史；平台目录/供应商 | Tauri 命令、状态、启动初始化需编译期注册；自有实现已通过 zhishu 挂载 | L1 | Rust 编译/隔离测试 | 2026-10-29 / ADR-0002 |
| ZS-122 | `src-tauri/src/live/engine.rs` | 品牌/共存；开发隔离 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-123 | `src-tauri/src/live/project/claude.rs` | Claude 模型档位 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-124 | `src-tauri/src/mcode_config.rs` | 平台目录/供应商 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-125 | `src-tauri/src/mode/controller.rs` | 品牌/共存；开发隔离；配置历史；Claude 模型档位 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-126 | `src-tauri/src/mode/operation.rs` | 配置历史 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-127 | `src-tauri/src/opencode_config.rs` | 平台目录/供应商 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-128 | `src-tauri/src/panic_hook.rs` | 品牌/共存 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-129 | `src-tauri/src/pi_config/mod.rs` | 平台目录/供应商 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-130 | `src-tauri/src/prompt_files.rs` | 工具扩展 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-131 | `src-tauri/src/provider.rs` | 工具扩展；平台目录/供应商 | 已有 ProviderMeta 序列化扩展和工具凭据解析需保留兼容；本轮不迁移持久化类型 | L3 | Provider/平台同步与凭据回归 | 2026-10-29 / ADR-0002 |
| ZS-132 | `src-tauri/src/proxy/http_client.rs` | 品牌/共存 | 代理默认端口和 AppType 穷举属于核心协议入口，额外工具不得误进不兼容的代理 | L3 | 代理配置及工具分派测试 | 2026-10-29 / ADR-0002 |
| ZS-133 | `src-tauri/src/proxy/providers/mod.rs` | 工具扩展 | 代理默认端口和 AppType 穷举属于核心协议入口，额外工具不得误进不兼容的代理 | L3 | 代理配置及工具分派测试 | 2026-10-29 / ADR-0002 |
| ZS-134 | `src-tauri/src/proxy/types.rs` | 品牌/共存 | 代理默认端口和 AppType 穷举属于核心协议入口，额外工具不得误进不兼容的代理 | L3 | 代理配置及工具分派测试 | 2026-10-29 / ADR-0002 |
| ZS-135 | `src-tauri/src/services/backup_storage.rs` | 品牌/共存；开发隔离 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-136 | `src-tauri/src/services/env_manager.rs` | 品牌/共存；开发隔离 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-137 | `src-tauri/src/services/mcp.rs` | 工具扩展 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-138 | `src-tauri/src/services/provider/claude_direct.rs` | Claude 模型档位 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-139 | `src-tauri/src/services/provider/claude_editor.rs` | Claude 模型档位 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-140 | `src-tauri/src/services/provider/codex_client_catalog.rs` | 品牌/共存 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-141 | `src-tauri/src/services/provider/live.rs` | 工具扩展；配置历史 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-142 | `src-tauri/src/services/provider/mod.rs` | 工具扩展；配置历史；Claude 模型档位 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-143 | `src-tauri/src/services/provider/pi.rs` | 平台目录/供应商 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-144 | `src-tauri/src/services/skill.rs` | 品牌/共存；工具扩展 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-145 | `src-tauri/src/services/stream_check.rs` | 工具扩展 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-146 | `src-tauri/src/settings.rs` | 品牌/共存；开发隔离；工具扩展 | 存量 AppType、目录字段、默认值及序列化别名贯穿上游匹配，包装无法增补枚举 | L3 | 配置加载/目录设置/工具回归 | 2026-10-29 / ADR-0002 |
| ZS-147 | `src-tauri/src/tray.rs` | 品牌/共存；工具扩展 | 现有实际写入/切换/路径分派中的扩展与审计挂载必须覆盖原调用链；自有实现迁出，保留核心调用点 | L3 | Provider/MCP/Skills/配置历史隔离回归 | 2026-10-29 / ADR-0002 |
| ZS-148 | `src-tauri/tauri.conf.json` | 品牌/共存；开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |
| ZS-149 | `src-tauri/tests/app_config_load.rs` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-150 | `src-tauri/tests/golden/claude_key_fields.rs` | 归位挂载/上游兼容 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-151 | `src-tauri/tests/golden/util.rs` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-152 | `src-tauri/tests/mcp_commands.rs` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-153 | `src-tauri/tests/provider_commands.rs` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-154 | `src-tauri/tests/provider_service.rs` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-155 | `src-tauri/tests/skill_sync.rs` | 归位挂载/上游兼容 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-156 | `src-tauri/tests/support.rs` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-157 | `src-tauri/wix/per-user-main.wxs` | 品牌/共存 | 平台打包器、桌面入口或品牌资源要求固定文件位置，新增业务 Adapter 不能替代 | L1 | 发行配置测试；实机安装待验证 | 2026-10-29 / ADR-0002 |
| ZS-158 | `src/App.tsx` | 工具扩展；配置历史；平台目录/供应商；事件生命周期 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-159 | `src/components/SubscriptionQuotaFooter.tsx` | 平台目录/供应商 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-160 | `src/components/apps/AppsPage.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-161 | `src/components/providers/AddProviderDialog.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-162 | `src/components/providers/EditProviderDialog.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-163 | `src/components/providers/ProviderCard.tsx` | 工具扩展；平台目录/供应商 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-164 | `src/components/providers/ProviderList.tsx` | 工具扩展；平台目录/供应商 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-165 | `src/components/providers/forms/EndpointSpeedTest.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-166 | `src/components/providers/forms/ProviderForm.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-167 | `src/components/providers/forms/ProviderPresetSelector.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-168 | `src/components/providers/forms/hooks/index.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-169 | `src/components/providers/forms/presetGroups.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-170 | `src/components/providers/presentation.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-171 | `src/components/quota/quotaRules.ts` | 平台目录/供应商 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-172 | `src/components/settings/AboutSection.tsx` | 品牌/共存；开发/发行 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-173 | `src/components/settings/SettingsPage.tsx` | 配置历史 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-174 | `src/components/settings/sections/AppConfigSection.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-175 | `src/components/settings/sections/GeneralSection.tsx` | 平台目录/供应商 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-176 | `src/components/settings/sections/RoutingSection.tsx` | 品牌/共存 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-177 | `src/components/shell/AppGlyph.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-178 | `src/components/shell/Sidebar.tsx` | 品牌/共存；认证会话；配置历史；平台目录/供应商 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-179 | `src/components/skills/SkillsStorageSheet.tsx` | 品牌/共存 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-180 | `src/components/skills/UnifiedSkillsPanel.tsx` | 品牌/共存 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-181 | `src/components/usage/RequestLogTable.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-182 | `src/config/appConfig.tsx` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-183 | `src/contexts/UpdateContext.tsx` | 开发/发行 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-184 | `src/hooks/useDirectorySettings.ts` | 品牌/共存；工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-185 | `src/hooks/useProviderActions.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-186 | `src/hooks/useSettings.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-187 | `src/hooks/useSettingsForm.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-188 | `src/hooks/useTauriEvent.ts` | 事件生命周期 | 事件订阅生命周期修复必须应用到原监听器，外层包装无法取消原泄漏的监听 | L2 | useTauriEvent 与 App 集成测试 | 2026-10-29 / ADR-0002 |
| ZS-189 | `src/hooks/useUsageEventBridge.ts` | 事件生命周期 | 事件订阅生命周期修复必须应用到原监听器，外层包装无法取消原泄漏的监听 | L2 | useTauriEvent 与 App 集成测试 | 2026-10-29 / ADR-0002 |
| ZS-190 | `src/i18n/locales/en.json` | 品牌/共存；开发/发行；工具扩展；认证会话；配置历史；平台目录/供应商 | 现有 i18n 注册表集中载入语言包，保留兼容键值避免重复注册与覆盖顺序改变 | L1 | 前端全量与业务测试 | 2026-10-29 / ADR-0002 |
| ZS-191 | `src/i18n/locales/ja.json` | 品牌/共存；开发/发行；工具扩展；认证会话；配置历史；平台目录/供应商 | 现有 i18n 注册表集中载入语言包，保留兼容键值避免重复注册与覆盖顺序改变 | L1 | 前端全量与业务测试 | 2026-10-29 / ADR-0002 |
| ZS-192 | `src/i18n/locales/tr.json` | 品牌/共存；工具扩展；认证会话；配置历史；平台目录/供应商 | 现有 i18n 注册表集中载入语言包，保留兼容键值避免重复注册与覆盖顺序改变 | L1 | 前端全量与业务测试 | 2026-10-29 / ADR-0002 |
| ZS-193 | `src/i18n/locales/zh-TW.json` | 品牌/共存；开发/发行；工具扩展；认证会话；配置历史；平台目录/供应商 | 现有 i18n 注册表集中载入语言包，保留兼容键值避免重复注册与覆盖顺序改变 | L1 | 前端全量与业务测试 | 2026-10-29 / ADR-0002 |
| ZS-194 | `src/i18n/locales/zh.json` | 品牌/共存；开发/发行；工具扩展；认证会话；配置历史；平台目录/供应商 | 现有 i18n 注册表集中载入语言包，保留兼容键值避免重复注册与覆盖顺序改变 | L1 | 前端全量与业务测试 | 2026-10-29 / ADR-0002 |
| ZS-195 | `src/lib/api/index.ts` | 工具扩展；配置历史 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-196 | `src/lib/api/providers.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-197 | `src/lib/api/skills.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-198 | `src/lib/api/types.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-199 | `src/lib/navigation.ts` | 配置历史；平台目录/供应商 | 应用根挂载与视图联合类型没有运行时插件入口，需薄注册自有视图/认证 | L1 | App 集成、导航回归 | 2026-10-29 / ADR-0002 |
| ZS-200 | `src/lib/query/index.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-201 | `src/lib/query/mutations.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-202 | `src/lib/updater.ts` | 开发/发行 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-203 | `src/main.tsx` | 品牌/共存；认证会话 | 应用根挂载与视图联合类型没有运行时插件入口，需薄注册自有视图/认证 | L1 | App 集成、导航回归 | 2026-10-29 / ADR-0002 |
| ZS-204 | `src/types.ts` | 工具扩展；平台目录/供应商 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-205 | `src/utils/providerConfigUtils.ts` | 工具扩展 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-206 | `src/vite-env.d.ts` | 开发/发行 | 现有表单/卡片/查询/类型内部需扩展工具分派或挂载自有组件；包装会遗漏内部状态，保留原核心页面 | L2 | 类型、前端全量和自有模块回归 | 2026-10-29 / ADR-0002 |
| ZS-207 | `tests/components/AppsPage.test.tsx` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-208 | `tests/components/SessionManagerPage.test.tsx` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-209 | `tests/components/SettingsDialog.test.tsx` | 配置历史 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-210 | `tests/components/SkillsStorageSheet.test.tsx` | 品牌/共存 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-211 | `tests/components/SwitchModePanel.test.tsx` | 品牌/共存 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-212 | `tests/components/UsageDashboard.test.tsx` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-213 | `tests/components/providerPresentation.test.ts` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-214 | `tests/components/shell/SidebarToday.test.tsx` | 配置历史 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-215 | `tests/hooks/useDirectorySettings.test.tsx` | 品牌/共存；工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-216 | `tests/hooks/useProxyStatus.test.tsx` | 品牌/共存 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-217 | `tests/integration/App.test.tsx` | 平台目录/供应商 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-218 | `tests/integration/SettingsDialog.test.tsx` | 品牌/共存 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-219 | `tests/msw/handlers.ts` | 配置历史 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-220 | `tests/msw/state.ts` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-221 | `tests/utils/providerConfigUtils.baseUrl.test.ts` | 工具扩展 | 现有上游测试或公共夹具须匹配已保留的路径、枚举和事件行为，平行测试不能修正原断言 | L0 | 对应前端/Rust 回归 | 2026-10-29 / ADR-0002 |
| ZS-222 | `vite.config.ts` | 开发/发行 | 构建/CI 固定入口需要引用自有脚本、依赖或身份配置，纯新增模块不会被工具链发现 | L1 | 类型/构建；开发与发行脚本测试 | 2026-10-29 / ADR-0002 |

## 历史合并登记（原记录保留）

下面是原 4.0.21 冲突修复阶段的记录；其中“未提交合并”等描述属于当时状态。
原差异覆盖见本页对应历史清单；当前文件位置以源码和文档总索引为准。

## 变更审查规范

- 补丁必须引用 PR / Issue；敏感信息不得进入登记表。
- 上游已提供同类能力时，优先删除重复补丁。
- 每次 `sync/upstream-*` PR 均需逐项复核本表。

## 2026-10-29：4.0.21 合并复核

范围仅为本次冲突与兼容修复，不代表历史 Fork 差异已全部审计。对应本地未提交合并；尚无 PR/Issue。详见 ADR-0001 和同步报告。

| ID | 文件 | 修改目的 | 替代方案评估 | 侵入级别 | 验证方式 | 上游同步复核 |
|---|---|---|---|---|---|---|
| SYNC-20261029-01 | `package.json` | 保留智枢 0.0.1 产品身份、发行配置与 keyring，合入 prost | 工具链固定读取这些清单，Adapter 无法替代发布元数据 | L1 | release-config / release / 构建 | c4db63c6 / ADR-0001 |
| SYNC-20261029-02 | `src-tauri/Cargo.lock` | 保留智枢 0.0.1 产品身份、发行配置与 keyring，合入 prost | 工具链固定读取这些清单，Adapter 无法替代发布元数据 | L1 | release-config / release / 构建 | c4db63c6 / ADR-0001 |
| SYNC-20261029-03 | `src-tauri/Cargo.toml` | 保留智枢 0.0.1 产品身份、发行配置与 keyring，合入 prost | 工具链固定读取这些清单，Adapter 无法替代发布元数据 | L1 | release-config / release / 构建 | c4db63c6 / ADR-0001 |
| SYNC-20261029-04 | `src-tauri/src/app_config.rs` | 合并 DSH/WorkBuddy/OMP 的注册、设置及枚举覆盖 | 现有入口和枚举需要直接补全；prompt/proxy 两文件仅规范合并格式 | L3 | Rust fmt/clippy/test | c4db63c6 / ADR-0001 |
| SYNC-20261029-05 | `src-tauri/src/lib.rs` | 合并 DSH/WorkBuddy/OMP 的注册、设置及枚举覆盖 | 现有入口和枚举需要直接补全；prompt/proxy 两文件仅规范合并格式 | L1 | Rust fmt/clippy/test | c4db63c6 / ADR-0001 |
| SYNC-20261029-06 | `src-tauri/src/services/mcp.rs` | 保留 DSH/WorkBuddy 分支及配置历史包装，同时合入 OMP | 既有核心穷举、服务分派与配置写入交叉，新增 Adapter 无法补全调用点 | L3 | provider / MCP / Skills / stream_check 测试与 clippy | c4db63c6 / ADR-0001 |
| SYNC-20261029-07 | `src-tauri/src/services/provider/live.rs` | 保留 DSH/WorkBuddy 分支及配置历史包装，同时合入 OMP | 既有核心穷举、服务分派与配置写入交叉，新增 Adapter 无法补全调用点 | L3 | provider / MCP / Skills / stream_check 测试与 clippy | c4db63c6 / ADR-0001 |
| SYNC-20261029-08 | `src-tauri/src/services/provider/mod.rs` | 保留 DSH/WorkBuddy 分支及配置历史包装，同时合入 OMP | 既有核心穷举、服务分派与配置写入交叉，新增 Adapter 无法补全调用点 | L3 | provider / MCP / Skills / stream_check 测试与 clippy | c4db63c6 / ADR-0001 |
| SYNC-20261029-09 | `src-tauri/src/services/skill.rs` | 保留 DSH/WorkBuddy 分支及配置历史包装，同时合入 OMP | 既有核心穷举、服务分派与配置写入交叉，新增 Adapter 无法补全调用点 | L3 | provider / MCP / Skills / stream_check 测试与 clippy | c4db63c6 / ADR-0001 |
| SYNC-20261029-10 | `src-tauri/src/services/stream_check.rs` | 保留 DSH/WorkBuddy 分支及配置历史包装，同时合入 OMP | 既有核心穷举、服务分派与配置写入交叉，新增 Adapter 无法补全调用点 | L3 | provider / MCP / Skills / stream_check 测试与 clippy | c4db63c6 / ADR-0001 |
| SYNC-20261029-11 | `src-tauri/src/settings.rs` | 合并 DSH/WorkBuddy/OMP 的注册、设置及枚举覆盖 | 现有入口和枚举需要直接补全；prompt/proxy 两文件仅规范合并格式 | L3 | Rust fmt/clippy/test | c4db63c6 / ADR-0001 |
| SYNC-20261029-12 | `src-tauri/tauri.conf.json` | 保留智枢 0.0.1 产品身份、发行配置与 keyring，合入 prost | 工具链固定读取这些清单，Adapter 无法替代发布元数据 | L1 | release-config / release / 构建 | c4db63c6 / ADR-0001 |
| SYNC-20261029-13 | `src/App.tsx` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-14 | `src/components/apps/AppsPage.tsx` | 保留上游安装分组/拖拽，并保留手动管理应用及开关 | 现有列表定义与渲染路径冲突，包装无法恢复内部行类型和拖拽上下文 | L2 | AppsPage 57 tests | c4db63c6 / ADR-0001 |
| SYNC-20261029-15 | `src/components/providers/ProviderCard.tsx` | 平台模型数/额度与 Claude 官方账号额度共存 | 两个条件共享卡片内部查询和渲染状态，需合并条件 | L2 | ProviderCard / PlatformUsageFooter / ClaudeAccount tests | c4db63c6 / ADR-0001 |
| SYNC-20261029-16 | `src/components/providers/ProviderList.tsx` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-17 | `src/components/providers/forms/ProviderPresetSelector.tsx` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-18 | `src/components/settings/sections/AppConfigSection.tsx` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-19 | `src/config/appConfig.tsx` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-20 | `src/hooks/useDirectorySettings.ts` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-21 | `src/hooks/useSettings.ts` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-22 | `src/i18n/locales/en.json` | 同时保留平台登录/额度、DSH/WorkBuddy 和 OMP 翻译 | i18n 现有键由共享字典读取，不能通过页面包装恢复缺失键 | L1 | 类型检查 / 平台登录与额度测试 | c4db63c6 / ADR-0001 |
| SYNC-20261029-23 | `src/i18n/locales/ja.json` | 同时保留平台登录/额度、DSH/WorkBuddy 和 OMP 翻译 | i18n 现有键由共享字典读取，不能通过页面包装恢复缺失键 | L1 | 类型检查 / 平台登录与额度测试 | c4db63c6 / ADR-0001 |
| SYNC-20261029-24 | `src/i18n/locales/tr.json` | 同时保留平台登录/额度、DSH/WorkBuddy 和 OMP 翻译 | i18n 现有键由共享字典读取，不能通过页面包装恢复缺失键 | L1 | 类型检查 / 平台登录与额度测试 | c4db63c6 / ADR-0001 |
| SYNC-20261029-25 | `src/i18n/locales/zh-TW.json` | 同时保留平台登录/额度、DSH/WorkBuddy 和 OMP 翻译 | i18n 现有键由共享字典读取，不能通过页面包装恢复缺失键 | L1 | 类型检查 / 平台登录与额度测试 | c4db63c6 / ADR-0001 |
| SYNC-20261029-26 | `src/i18n/locales/zh.json` | 同时保留平台登录/额度、DSH/WorkBuddy 和 OMP 翻译 | i18n 现有键由共享字典读取，不能通过页面包装恢复缺失键 | L1 | 类型检查 / 平台登录与额度测试 | c4db63c6 / ADR-0001 |
| SYNC-20261029-27 | `src/lib/query/mutations.ts` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-28 | `src/types.ts` | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-29 | `tests/hooks/useDirectorySettings.test.tsx` | 合并双方目录预期，覆盖手动管理分组与 OMP | 修改对应既有回归断言，避免吞掉自有扩展或上游新增应用 | L0 | vitest | c4db63c6 / ADR-0001 |
| SYNC-20261029-30 | `src/hooks/useConfigMembership.ts`（分支自有） | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-31 | `src/lib/query/additiveLiveState.ts`（分支自有） | 合并应用类型、目录设置和 Provider 查询，复用共享缓存逻辑 | 缺失声明/既有 Hook 调用点必须修复，新增平行实现会造成状态分裂 | L2 | typecheck / vitest / renderer build | c4db63c6 / ADR-0001 |
| SYNC-20261029-32 | `src-tauri/src/database/mod.rs` | 修复重复 22 分支及已发布 v23 缺少 OMP 字段；缺列升级先备份 | 旧数据库同号异构，必须在既有迁移事务及初始化备份入口收敛 | L3 | merged_v22 / merged_v23 / migration_failure 与数据库全量测试 | c4db63c6 / ADR-0001 |
| SYNC-20261029-33 | `src-tauri/src/database/schema.rs` | 修复重复 22 分支及已发布 v23 缺少 OMP 字段；缺列升级先备份 | 旧数据库同号异构，必须在既有迁移事务及初始化备份入口收敛 | L3 | merged_v22 / merged_v23 / migration_failure 与数据库全量测试 | c4db63c6 / ADR-0001 |
| SYNC-20261029-34 | `src-tauri/src/mode/controller.rs` | 保留上游 publish_stack 抽取，将配置历史摘要留在成员变更调用方 | roster 属于现有调用作用域，外部包装无法修复未定义引用 | L3 | mode/config_history 全量测试 | c4db63c6 / ADR-0001 |
| SYNC-20261029-35 | `src-tauri/src/prompt_files.rs` | 合并 DSH/WorkBuddy/OMP 的注册、设置及枚举覆盖 | 现有入口和枚举需要直接补全；prompt/proxy 两文件仅规范合并格式 | L3 | Rust fmt/clippy/test | c4db63c6 / ADR-0001 |
| SYNC-20261029-36 | `src-tauri/src/proxy/providers/mod.rs` | 合并 DSH/WorkBuddy/OMP 的注册、设置及枚举覆盖 | 现有入口和枚举需要直接补全；prompt/proxy 两文件仅规范合并格式 | L3 | Rust fmt/clippy/test | c4db63c6 / ADR-0001 |
| SYNC-20261029-37 | `tests/components/AppsPage.test.tsx` | 合并双方目录预期，覆盖手动管理分组与 OMP | 修改对应既有回归断言，避免吞掉自有扩展或上游新增应用 | L0 | vitest | c4db63c6 / ADR-0001 |
| SYNC-20261029-38 | `tests/zhishu/mergePiFamily.test.tsx`（分支自有） | 合并双方目录预期，覆盖手动管理分组与 OMP | 修改对应既有回归断言，避免吞掉自有扩展或上游新增应用 | L0 | vitest | c4db63c6 / ADR-0001 |
| SYNC-20261029-39 | `src-tauri/src/config_history.rs`（分支自有） | 给配置路径回归测试补共用串行锁，避免其他测试切换 HOME/配置目录导致误报 | 仅测试属性；该文件为分支原有扩展，不改生产逻辑 | L0 | 单独复核与全量 cargo test | 原测试单跑通过，首次并发失败 |
| SYNC-20261029-40 | `src-tauri/tests/golden/claude_key_fields.rs` | 将显式档位替换与缺失档位派生分开验证，覆盖原始数据库配置不变 | 原 golden 契约未覆盖分支既有派生行为；不修改生产实现 | L0 | golden 集成测试 | 既有 with_derived_model_slots 语义 |
| SYNC-20261029-41 | `src-tauri/tests/provider_service.rs` | 集成夹具使用智枢既有 .zhishu-ccs 数据目录，验证账号镜像和 Skills 备份恢复 | 测试硬编码旧上游目录导致夹具未进入实际服务目录；不修改生产路径 | L0 | provider_service / skill_sync | 旧路径在合并测试中遗漏适配 |
| SYNC-20261029-42 | `src-tauri/tests/skill_sync.rs` | 集成夹具使用智枢既有 .zhishu-ccs 数据目录，验证账号镜像和 Skills 备份恢复 | 测试硬编码旧上游目录导致夹具未进入实际服务目录；不修改生产路径 | L0 | provider_service / skill_sync | 旧路径在合并测试中遗漏适配 |


## 2026-10-29 Windows EXE / Linux 暂停打包补丁

依据：[ADR-0003](../adr/ADR-0003-windows-exe-release.md)。功能分支：`feat/windows-exe-release`。

| ID | 上游文件 | 原因 | 为什么不能仅新增/包装 | 级别 | 对应测试 | 最后复核 |
|---|---|---|---|---|---|---|
| BUILD-20261029-01 | `.github/workflows/release.yml` | Windows 仅打包 x64/ARM64 NSIS EXE，移除 Linux 打包及便携 ZIP，复用自有脚本生成更新清单 | GitHub 的固定工作流入口必须调整现有矩阵、命令与产物收集，新增 Adapter 无法取消已有构建 | L1 | 发布脚本、工作流语法；Windows 实机待验证 | 2026-10-29 / ADR-0003 |
| BUILD-20261029-02 | `src-tauri/tauri.windows.conf.json` | Windows 默认使用 NSIS/currentUser，与 CI 和原 MSI perUser 范围一致 | Tauri 按固定平台配置合并默认打包目标，业务包装无法改变该默认值 | L1 | 配置 Schema；Windows 安装/升级待验证 | 2026-10-29 / ADR-0003 |
| BUILD-20261029-03 | `scripts/generate-download-manifest.mjs` | 官网清单识别 x64/ARM64 EXE，保留历史包格式读取 | 所有发布入口复用该生成器，需要扩展同一格式规则而非新增平行清单 | L1 | 产物及官网解析/下载展示回归 | 2026-10-29 / ADR-0003 |

自有文件同步调整：`scripts/zhishu/release-artifacts.mjs`、`cos-release.mjs`、`publish-cos.py` 及对应测试，`.github/workflows/resume-cos-release.yml`、发布说明和 ADR。COS 仅对六个明确的旧 Linux updater 键允许暂停；仍拒绝其他平台丢失、降级、同版本改包。历史恢复依原收据识别旧 MSI/Linux 产物。

## 0.0.2 发布准备复核（2026-10-30）

复核 ZS-044、ZS-046、ZS-047、ZS-148：应用版本统一为 0.0.2，侵入等级 L1。版本字段由 npm/Cargo/Tauri 固定入口读取，不能由新增 Adapter 替代；复用既有打包、签名、COS 发布与更新实现，应用身份和更新协议不变。新增 `src/whats-new/0.0.2.json`；验证及发布状态见 [0.0.2 发布记录](../releases/0.0.2.md)。

发布准备补充复核 ZS-003（L1）：`.github/workflows/release.yml` 的 COS SDK 契约测试发现目录由 `scripts` 改为实际的 `scripts/zhishu`；自有恢复工作流同步修正。原命令实测执行 0 项测试，固定工作流入口必须直接修正，新增模块不能改变 unittest 的发现目录。未改变构建、签名、上传策略及权限。

2026-10-30 加速恢复补充：仅修改自有 `scripts/zhishu/resume-release.mjs` 与回归，L1；支持在完整 payload 保存成功后取消的正式运行，仍核对原 tag/SHA、唯一未过期产物及后续签名/哈希。没有新增上游文件补丁；本次用户授权见 [0.0.2 发布记录](../releases/0.0.2.md)。

## 2026-10-31 客户端专用 API

依据：[ADR-0004](../adr/ADR-0004-client-api-only.md)。自有旧平台请求、Key 生命周期及网络模型发现已删除，复用上游 Provider 与配置引擎。

| ID | 上游文件 | 原因 | 为什么不能仅新增/包装 | 级别 | 对应测试 | 最后复核 |
|---|---|---|---|---|---|---|
| CLIENT-20261031-01 | `src-tauri/src/lib.rs` | 注册只读客户端 config IPC | Tauri 固定注册表必须挂载命令 | L1 | auth IPC / config / typecheck | 2026-10-31 |
| CLIENT-20261031-02 | `src-tauri/src/provider.rs`、`src/types.ts` | 平台元数据新增可选 connectionBaseUrl，保留生成网关地址用于增量刷新 | 共享序列化类型须保留新字段，外部包装不能保证数据库往返 | L2 | gateway / Provider 同步 / typecheck | 2026-10-31 |
| CLIENT-20261031-03 | `src/i18n/locales/{zh,zh-TW,en,ja,tr}.json` | 新额度状态和原因文案 | 现有 i18n 通过共享字典读取 | L1 | PlatformUsageFooter / PlatformCatalogPage | 2026-10-31 |

其余实现均在 `src/zhishu/`、`src-tauri/src/zhishu/` 和对应测试、文档内。不修改上游 DAO、代理、配置写入/恢复引擎，不引入依赖。

2026-10-31 后续清理复核 CLIENT-20261031-03：删除五份共享翻译中的二开 `platformCatalog.credentialsCleared` 键，旧凭证清理状态及页面分支已移除；对应 `PlatformCatalogPage` 回归改用新协议的缺凭证状态。其余清理位于自有模块和测试，没有新增上游核心补丁。


## 2026-10-31 账号基础数据隔离与清理退出

依据：[ADR-0005](../adr/ADR-0005-account-local-cleanup.md)。用户明确保留单活动会话，暂缓多账号同时登录。

| ID | 上游文件 | 原因 | 为什么不能仅新增/包装 | 级别 | 对应测试 | 最后复核 |
|---|---|---|---|---|---|---|
| ACCOUNT-20261031-01 | `src-tauri/src/lib.rs` | 注册 `platform_logout_and_clear_account` | Tauri 固定注册表必须挂载新命令，自有实现通过 facade 导出 | L1 | platformAuth IPC / Rust 清理退出 | 2026-10-31 |
| ACCOUNT-20261031-02 | `src/i18n/locales/{zh,zh-TW,en,ja,tr}.json` | 增加清理确认、结果与账号隔离说明 | 共享 i18n 加载入口读取现有字典 | L1 | PlatformLogin / PlatformCatalogPage | 2026-10-31 |

业务、存储迁移和测试均位于自有命名空间。复用既有全应用供应商删除、ProviderService Adapter、操作锁、私密原子写入及配置审计/恢复；保留上游代码，无 DAO 或通用配置引擎改动。验证状态见 [VALIDATION](../history/validation.md)。

清理弹窗视觉迭代仅修改自有 `PlatformAccountCleanup`、`PlatformProviderDeletionResult` 及对应测试，复用上游 Dialog 原有插槽，未修改通用 Dialog。五语文案仍归 ACCOUNT-20261031-02；等级仍为 L0/L1。

全应用供应商删除反馈迁至自有 `PlatformProviderDeletionDialog`，目录页及自有 Hook 增加可关闭结果与通知选择；五语 `platformDeletion` 文案沿用 ACCOUNT-20261031-02 的翻译入口。L0/L1，复用通用 Dialog 与既有删除/缓存刷新能力，未改上游核心。


## 2026-10-31 平台基础数据详情布局

| ID | 上游文件 | 原因 | 为什么不能仅新增/包装 | 级别 | 对应测试 | 最后复核 |
|---|---|---|---|---|---|---|
| DETAILS-20261031-01 | `src/i18n/locales/{zh,zh-TW,en,ja,tr}.json` | 增加详情页签、模型搜索、数量与用量表头文案 | 复用共享 i18n 字典加载入口 | L1 | PlatformCatalogPage / typecheck | 2026-10-31 |

布局与本地搜索位于自有 `PlatformCatalogDetails`、`PlatformCatalogModels`，L0；复用上游 Sheet、Tabs、Collapsible、Input 和既有显式 Key 组件。概览改用单一滚动区域，移除内部 SheetBody 包装，避免其 `scroll-stable` 与 `overscroll-contain` 截断滚轮；未改通用 Sheet 或全局 CSS，未删除任何上游实现。无 IPC、持久化或配置写入变化；回退只需恢复自有详情布局及相应翻译/测试，不涉及数据迁移。


## 0.0.3 发布准备复核（2026-10-31）

复核 ZS-044、ZS-046、ZS-047、ZS-148：package/Tauri/Cargo/lock 版本统一为 0.0.3，L1，工具固定读取的版本字段不能通过 Adapter 替代。Cargo 生成锁文件，不升级依赖。

复核 ZS-003（L1）：GitHub 固定工作流增加本次发布分支演练入口，并仅为 v0.0.3 正式 tag 强制启用 COS 全球上传加速；其他 ref 保持现有仓库变量语义。复用既有签名、3 GB 加速读取上限、不可变产物及双清单检查，不改客户端更新地址/公钥。验证与剩余风险见 [0.0.3 发布记录](../releases/0.0.3.md)。

## 2026-10-31 zws 历史整理分支

依据 [ADR-0006](../adr/ADR-0006-zws-history-without-harness.md)，当时的独立整理分支移除二开新增的 Harness 应用入口及其适配。该记录不表示当前分支缺少 DSH；后续已合入上游 4.0.22，当前平台同步也覆盖 DSH。受影响的上游文件集中在 `app_config.rs`、`settings.rs`、`provider.rs`、`services/provider/{mod,live}.rs`、`services/{mcp,skill,stream_check}.rs`、`commands/{config,provider}.rs`、`lib.rs`、前端应用列表/表单/设置及五语字典。没有替代新增模块能够取消这些既有枚举分支与固定注册项；移除属于 L1/L3 反向补丁。WorkBuddy、平台同步与配置历史继续复用上游 ProviderService、CLI 配置、DAO、IPC 和原子写入路径。回退为切回原 `ccs-dev`，不迁移或删除本机数据。验证范围见 [VALIDATION](../history/validation.md)。

## 2026-10-31 文档分类与引用修正

本次为 L0 文档整理，固定工具入口的文档提示路径修正按 L1 复核；复核既有 ZS-006（贡献指南）与 ZS-222（Vite 配置）：只更新指向新文档目录的链接或错误提示字符串。固定入口中的字符串不能由新增文档或 Adapter 自动替换；不改变构建校验、IPC、配置写入或运行行为。macOS 开发脚本中的文档提示属于自有文件，同步修正。

旧审查报告的有效修复与遗留风险已合入历史验证记录，两份提交映射合并保留；版本发布证据与操作指南分离。强制标准按已有 ADR-0004/0005 修正被替代的业务示例，核心红线不变。用户原有 DSH 适配、验证与 AGENTS 修改保留。实际本次检查见历史验证记录的文档整理小节。


## 2026-11-03 与原版 CC Switch 共存隔离

用户授权修复共存冲突，依据 [ADR-0009](../adr/ADR-0009-product-coexistence.md)。复用 ProviderService、路径解析、配置投影、原子发布/首写备份/pending、Skills 部署校验与既有 S3/WebDAV 传输；自有常量与归属记录位于 `src-tauri/src/zhishu/{identity,coexistence}.rs`。没有新增依赖或 IPC，也不迁移凭据、不删除上游文件/API。旧资源不自动认领，详见[共存说明](coexistence.md)。

| ID | 上游文件 | 原因与无法仅包装的说明 | 等级 | 关联验证 |
| --- | --- | --- | --- | --- |
| COEXIST-20261103-01 | `codex_config.rs`、`live/project/{codex,claude}.rs`、`mode/stack.rs`、`claude_desktop_config.rs`、`deeplink/provider.rs` | 原生派生文件、路由/聚合 ID、占位符及 Desktop profile 的常量在投影内部读取，须在已有边界接入独立身份；保留旧输入解析和原版表/profile，`comp_hash` 沿用既有协议分组值 | L3 | Codex 投影、Desktop 原版 profile 保留、聚合拒绝外来 ID、压缩协议分组、金标 |
| COEXIST-20261103-02 | `mode/{controller,operation}.rs`、`services/proxy.rs`、`lib.rs` | 启停/定时刷新和发布重算在核心内部，外层包装不能阻止误认领或并发重算覆盖；添加自有路径哈希记录，Codex profile 只比较路由/认证字段，限制共享历史自动迁移，保留所有恢复函数 | L3 | 外来代理启动/退出保留、主动交接、profile 模型编辑后分离/重连及路由变更撤销归属、并发 attach/detach、已有 pending/故障回归 |
| COEXIST-20261103-03 | `settings.rs`、`services/{sync_protocol,s3_sync,webdav_sync}.rs` | 默认根与协议校验由固定序列化/传输入口控制；保留旧设置，拒绝未知/原版目标，上传前验证与部分上传归属标记 | L3 | 缺失 root 保留旧语义、双传输拒绝写入原版目标、同步协议及命令回归 |
| COEXIST-20261103-04 | `services/skill.rs`、`dsh_config.rs`、`agy_config.rs` | 原生文件直接由现有适配器部署/清理，薄接自有部署证据并复用内容/链接校验，独立 DSH 回执与 MCP ID；AGY 程序发现排除两款产品的旧包装脚本，清理仅接受智枢标记 | L3 | 外来 Skills 保留、copy 更新/卸载、DSH、两类 AGY 包装脚本排除与原版脚本保留 |
| COEXIST-20261103-05 | `proxy/forwarder.rs`、`proxy/providers/codex_oauth_auth/browser.rs`、`codex_history_migration.rs` | 新旧占位符均禁止向上游泄漏；保留旧历史桶的独立含义，显式统一迁移随 live 目标并兼容新旧账本还原；OAuth 保留协议端口，明确占用错误并关闭回调连接 | L3 | 占位符拒绝、历史迁移回归、浏览器监听器替换/取消/端口占用 |
| COEXIST-20261103-06 | `commands/misc.rs`、`tauri.windows.conf.json`、前端 `settings/{AboutSection,WebdavSyncSection}.tsx` | 固定旧更新入口/链接和表单默认值须直接接线，接口保留，转到智枢安装包页面 | L1/L2 | 全量前端、旧 command URL 回归、renderer 构建 |

其余已有 Rust/集成测试文件中的变更为命名对应断言；三个金标通过 `CC_SWITCH_UPDATE_GOLDEN=1` 生成并审查。部分旧 `custom` MCP/历史夹具刻意保留，验证输入兼容及原版数据不会随名称替换消失。实际验证和未验收项见[验证记录](../history/validation.md)。
