# 历史验证记录

> 本页保留各次任务的执行结果和未验证项；测试数量、代码路径、分支状态及临时资料均属于当时快照。本次文档整理没有重跑这些历史测试。当前行为以 [代码基线](../development/baseline.md)、功能说明和源码为准。

## 目录与功能提交整理（2026-10-29）

日期：2026-10-29；环境：本机 macOS，使用仓库现有 node_modules 与 Rust 缓存。
范围：ADR-0002 的文件归位、调用边界、脚本路径及功能提交压缩；不代表批准发布。

| 检查 | 结果 |
|---|---|
| 整理前 `pnpm typecheck` / `pnpm format:check` | 均通过，建立基线 |
| 最终 `pnpm typecheck` | 通过 |
| 最终 `pnpm format:check` | 通过 |
| `pnpm test:unit` | 283 文件、3580 项通过 |
| 最后菜单提取回归 | App 与删除 Hook，2 文件、36 项通过 |
| 最后集成层路径调整回归 | `tests/zhishu` + App，22 文件、232 项通过 |
| 最终 `pnpm build:renderer` | 通过，仍有既有大 chunk 提示 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy -- -D warnings` | 未通过：原 `commands/misc.rs` 两个未使用平台启动函数；原合并报告已记录，未以 allow 忽略 |
| 全量 Rust 首轮串行测试 | 4359 通过、11 忽略，1 个原有 OAuth keep-alive 用例失败（IncompleteMessage） |
| 最终 Rust 串行回归，排除上述 1 项 | 4359 单测通过、11 忽略；212 个集成测试通过 |
| 上述 OAuth 单项复测 | 1 项通过；仍保留完整执行时的偶发失败记录 |
| Rust 文档测试 | 仓库 `Cargo.toml` 已设置 `doctest = false`，本次没有执行文档测试 |
| `pnpm test:release` | 32 项通过 |
| `pnpm test:dev-isolation` | 3 项通过 |
| `pnpm test:dev-signing` | 6 项通过、1 项忽略（真实 Keychain 重建访问） |
| `pnpm test:release-cos` | unittest 21 项，19 通过、2 忽略 |
| 路径审查 | 100 项目标均存在，旧位置均移除，无遗留 TS 旧导入；保留原执行权限 |
| 有限机密模式审查 | 检查 31 个自有 first-parent 提交新增内容：未命中私钥 PEM/GitHub token/live key/AWS ID 高置信模式；不是完整安全审计 |
| 提交压缩等价性 | 平台、发行两个分组与上游合并节点的 tree ID 均与原组末节点相同 |
| 补丁覆盖 | 222 个已有上游文件差异逐项登记；新增自有文件不作为已有上游补丁计数 |

## 命令与隔离

Rust 每轮使用独立临时 `CC_SWITCH_TEST_HOME`，不启动客户端或写真实 CLI 配置。
最终完整覆盖按两轮完成：

```bash
CC_SWITCH_TEST_HOME="$(mktemp -d)" cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 --skip replacement_closes_the_previous_keep_alive_connection
CC_SWITCH_TEST_HOME="$(mktemp -d)" cargo test --manifest-path src-tauri/Cargo.toml replacement_closes_the_previous_keep_alive_connection -- --test-threads=1
```

源文件移动中发现的 IPC 重复前缀、旧脚本导入、测试访问私有 live 模块均已修复。
修复测试调用时使用 `services::provider::sync_additive_app_to_live` 已有再导出，没有扩大 live 模块可见性。
前端全量测试在菜单提取前通过；最后局部变更由相关 232 项覆盖，不声称再次运行了全量 3580 项。

## 未执行与保留限制

- Windows/macOS 实机安装、生产签名/公证、自动更新和双应用共存：本轮是源码组织任务，没有部署/发布或操作真实账号。
- 真实系统凭据库交互及 COS 真实网络发布测试：现有套件要求额外环境/显式启用，保持跳过。
- 历史每个中间提交未逐一编译；压缩只断言相应完整树等价，最终树执行本页验证。
- 原便携更新 URL TODO、正式远程命名/主线治理另行处理，不借目录整理改变既有运行协议。
- 自动审批拒绝删除无调用平台启动实现及迁移持久化元数据类型；采用保留方案，完整记录 clippy 基线缺陷。

日志保存在本机 `/tmp/zhishu-structure-20261029/`，为临时辅助资料；长期结果以本文和提交映射为准。

## 二开审查修复（2026-10-29，fix/zhishu-review-20261029）

审查起点为 `bf9228e3`，固定基线 4.0.21；以下合并原二开审查报告的有效结论，不覆盖上面目录整理阶段的结果。

- 修复批量写入、同步回调在退出、换账号或离开页面后继续执行的问题；已提交的单次原子 IPC 不强制中断。
- 读取本机成员名单失败不再按成功空列表处理；修正 `claudeDesktopStatus` 缓存键，并复用统一刷新适配器。
- 当时修复旧 Key 替换失败仍使用已拒绝凭证的缺陷；该客户端建 Key 链路随后由 ADR-0004 专用 `/access` 协议整体取代，不能据此恢复旧逻辑。
- 当时清理的 `commands/misc.rs` 两个函数为无调用方的二开补丁，消除了 dead-code 基线错误；保留上游受控终端功能和便携更新 TODO。
- 原报告建议的 v1 凭据库迁移未实施；当前实际是账号独立 v2 明文目录，见 [代码基线](../development/baseline.md)。本次没有新增凭据迁移或生产安全验收。

| 检查 | 结果 |
|---|---|
| 修复前全量前端测试 | 283 文件、3580 项通过 |
| 修复前新增回归 | 前端 6 项失败；Rust Key 替换用例失败，确认原代码会保留被拒绝的 Key |
| 最终 `pnpm typecheck` / `pnpm format:check` | 均通过 |
| 最终 `pnpm test:unit` | 283 文件、3591 项通过；同步/批量操作两个文件共 27 项 |
| 最终 `pnpm build:renderer` | 通过，保留既有大 chunk 提示 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy -- -D warnings` | 通过，删除无调用的二开平台启动函数后消除此前两个 dead-code 错误 |
| 隔离目录全量 Rust 串行测试 | 4360 项通过、11 项忽略；1 个既有 OAuth keep-alive 用例失败（IncompleteMessage），不能表述为单轮全量通过 |
| 上述 OAuth 用例单独复测 | 1 项通过；未修改或忽略该用例，保留其整套运行不稳定性记录 |
| Rust 集成测试补跑 | `cargo test --test '*' -- --test-threads=1`：16 个目标、212 项通过；前轮单测失败导致集成目标未执行，已补齐 |
| Node 开发/发布脚本测试 | `node --test scripts/zhishu/*.test.mjs`：41 项通过、1 项真实 Keychain 测试忽略 |
| Python COS 单测 | 21 项：19 通过、2 项依赖真实环境的用例忽略；使用模拟存储，不是实际上传 |
| `git diff --check` | 通过 |

Rust 每次执行都设置独立临时 `CC_SWITCH_TEST_HOME`；没有启动生产客户端、访问真实账户或覆写真实 CLI 配置。
未运行 Windows/macOS 实机 CLI、安装、升级、生产签名、真实凭据库和双应用共存回归；本次为源码修复，没有真实账号和发布环境的验证。
Rust doctest 由仓库现有 `doctest = false` 禁用。
日志位于 `/tmp/zhishu-review-*.log`。本轮没有提交、推送或发布，原有未跟踪 `out/` 保留。


## 平台地址校验放宽（2026-10-31）

范围：`src-tauri/src/zhishu/services/platform_auth.rs` 及其同文件测试、平台登录文档。复用现有登录、会话恢复、HTTP 客户端和 SessionStore；L0 自有模块调整，无新增上游补丁，未修改 IPC 参数、返回类型、数据库或 CLI 配置。

| 检查 | 本次结果 |
|---|---|
| `cargo fmt --check --manifest-path src-tauri/Cargo.toml` | 通过 |
| `CC_SWITCH_TEST_HOME="$(mktemp -d /tmp/zhishu-platform-url-test.XXXXXX)" cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --lib zhishu::services::platform_auth::tests -- --test-threads=1` | 38 项通过；覆盖远程 HTTP、完整地址规范化、实际本机 HTTP 登录请求和会话记录，以及既有 2FA、刷新、恢复与退出回归 |
| `pnpm test:unit tests/zhishu/lib/platformAuth.test.ts tests/zhishu/components/PlatformLogin.test.tsx` | 2 文件、31 项通过 |
| `git diff --check` | 通过 |

测试只使用本机模拟平台、临时目录和模拟凭据库。未运行全量测试、Clippy、前端类型检查或构建，也未进行真实平台登录、真实凭据库、Windows/Linux 与安装包验证。HTTP 明文传输及地址附带内容的清除行为见 [平台登录文档](../features/platform-login.md)。


## 客户端专用 API 改造（2026-10-31）

范围：按 [客户端专用接口契约](../api/client-api.md) 将平台登录、恢复、账户、基础同步与额度查询统一到 `/api/zhishu/v1/client`，删除自有旧平台调用、Key 查找/创建/替换/撤销与网络模型发现逻辑，不保留旧接口回退。前后端契约、额度与订阅展示、版本 2 私密快照、网关地址适配及相关文档同步更新。

侵入等级为 L0 自有业务、L1 注册/翻译、L2 可选共享 Provider 元数据；复用 ProviderService、应用锁、路径解析、原子配置发布/恢复、keyring 与账户查询隔离。不修改上游 DAO/schema、代理或通用配置引擎。决策、兼容与回退见 [ADR-0004](../adr/ADR-0004-client-api-only.md)，上游薄接入见 [补丁登记](../development/upstream-patches.md) CLIENT-20261031-01～03。

| 检查 | 本次结果 |
|---|---|
| `pnpm typecheck` | 通过 |
| `pnpm format:check` | 通过 |
| `pnpm test:unit` | 283 文件、3604 项通过 |
| `pnpm build:renderer` | 最终前端构建通过；保留浏览器数据过旧、duration 类歧义、混合静态/动态导入和大 chunk 的既有警告 |
| 全量 Rust 库测试：`cargo test --offline --manifest-path src-tauri/Cargo.toml --lib` | 完整复跑 4360 项通过、0 失败、11 项既有忽略项；未排除 OAuth 用例 |
| 最终自有模块回归：`cargo test --offline --manifest-path src-tauri/Cargo.toml --lib zhishu:: --quiet` | 195 项通过；覆盖全量库测试之后的恒真字段移除、缺凭证警告去重和 Clippy 条件合并 |
| 最终 `cargo clippy --offline --manifest-path src-tauri/Cargo.toml -- -D warnings` | 通过 |
| 最终 `cargo fmt --check --manifest-path src-tauri/Cargo.toml` | 通过 |
| 旧平台路径与 Key 生命周期源码审查 | 扫描 `src/zhishu` 与 `src-tauri/src/zhishu` 的 Rust/TS/TSX：旧 `/api/v1`、账户/分组/订阅/模型发现路径及旧 Key 生命周期函数引用为 0；另有 HTTP 请求方法/路径封闭白名单回归 |
| 变更文档相对链接核对 | README、补丁表、登录、基础数据、接入契约与 ADR 共 26 条有效，无缺失目标 |
| `git diff --check` | 通过 |

初轮全量 Rust 库测试为 4358 项通过、2 项失败、11 项忽略：本次缺凭证警告重复导致的断言失败已修复；另一项为未修改的 `proxy::providers::codex_oauth_auth::browser::tests::replacement_closes_the_previous_keep_alive_connection`，错误 `IncompleteMessage`。完整复跑全部通过，但保留该用例的偶发失败记录（历史验证亦有同类记录）。全量库测试完成后仅清理已无意义的目录可用字段/分支；最终 195 项自有模块测试及 Clippy、格式检查覆盖最终代码，不宣称最后清理后再次执行全量库测试。收尾 Clippy 曾指出可合并的嵌套条件，已修复并重新通过。

Rust 每轮使用独立临时目录：

```bash
CC_SWITCH_TEST_HOME="$(mktemp -d /tmp/zhishu-client-api.XXXXXX)" cargo test --offline --manifest-path src-tauri/Cargo.toml --lib
CC_SWITCH_TEST_HOME="$(mktemp -d /tmp/zhishu-client-api.XXXXXX)" cargo test --offline --manifest-path src-tauri/Cargo.toml --lib zhishu:: --quiet
```

网络/存储回归使用本机模拟 HTTP、临时目录、内存数据库和模拟凭据库，不改智能体的 `HOME`/`CODEX_HOME`，未运行真实账户操作或真实 CLI 配置切换。

兼容与未验证项：

- 旧版 `platform-catalog.json` 不迁移、不接管旧 Key；读取返回无目录，升级后需重新同步平台基础数据。新版本 2 快照与原生配置写入保留既有私密权限、原子保存和恢复路径。
- 真实服务端专用接口、多个客户端共用同一 Key、续订/撤权、请求丢失后的服务端事务幂等尚未联调；模拟测试不能代替服务端验收。
- 真实系统 keyring、macOS/Windows/Linux 实机矩阵、各 CLI 的模型调用与配置运行未验收。
- 本次仅执行完整 Rust `--lib`；独立集成测试目标、开发/发布脚本、安装、签名、公证和跨版本升级未执行，未发布或推送。
- 保留用户原有 README/基础数据文档修改、接入契约文件和未跟踪 `out/`；未创建提交。


## 客户端旧逻辑与测试清理（2026-10-31）

用户要求先提交前一轮改造，已在 `ccs-dev` 创建 `6f0c38d2`（`feat(zhishu): migrate platform integration to client-only APIs`），没有推送；随后继续清理二开残留，本节对应后续独立工作区差异。

清理范围：

- 删除无生产入口的 `clear_account_credentials`、`clear_cached_catalog_credentials`、`CatalogStore::clear_credentials`、`SessionStore::identity` 与凭证替换为登录占位符的包装函数，移除对应 `allow(dead_code)`。
- 删除旧 `credentialsCleared`/`credentials_cleared` 状态、错误码、页面禁用/提示分支及五种语言中的自有旧文案；缺失凭证使用新协议的分组状态表示。正常版本 2 快照继续读取，旧版本仍不迁移；空白凭证或损坏快照拒绝读取并保留原文件。
- 删除旧模型目录的 `api_format`、`mode`、嵌套 limit 和 camelCase 等元数据回退；模型映射与摘要读取 `/access` 白名单字段。测试使用专用 Key 元数据及当前协议字段，移除一项只验证旧清理流程的 Rust 用例和其他用例中的旧清理断言。
- 保留退出不改 Provider/原生文件、同账号恢复、跨账号隔离、摘要不泄密、原文件损坏保护、授权撤回及本地配置适配等有效回归。下载页仍在使用的构建网站地址、通用上游 API 和实际生效的配置迁移不属于旧平台 API 清理。

等级为 L0 自有业务、测试及契约清理 + L1 自有翻译键删除；复用现有 ProviderService、配置锁、私密原子存储和会话 keyring，无新增依赖、数据库迁移或上游核心补丁。[ADR-0004](../adr/ADR-0004-client-api-only.md)、[补丁登记](../development/upstream-patches.md)、AGENTS 和基础数据文档已同步。

| 检查 | 本轮结果 |
|---|---|
| `pnpm typecheck` | 通过 |
| `pnpm format:check` | 通过 |
| `pnpm test:unit tests/zhishu` | 21 文件、228 项通过，其中目录页面 23 项 |
| `pnpm build:renderer` | 通过；保留浏览器数据过旧、duration 类歧义、混合导入与大 chunk 的既有警告 |
| `cargo fmt --check --manifest-path src-tauri/Cargo.toml` | 通过 |
| `CC_SWITCH_TEST_HOME="$(mktemp -d /tmp/zhishu-client-cleanup.XXXXXX)" cargo test --offline --manifest-path src-tauri/Cargo.toml --lib zhishu:: --quiet` | 最终 194 项通过，无失败或忽略；最后模型字段清理后重新执行 |
| `cargo clippy --offline --manifest-path src-tauri/Cargo.toml -- -D warnings` | 最终通过 |
| 源码及测试残留扫描 | `src/zhishu`、`src-tauri/src/zhishu`、`tests/zhishu` 内旧平台路径、旧清理函数/状态/占位 Key、旧平台 Key 命名与 dead-code 豁免均无匹配；两个目录映射模块的旧字段回退亦无匹配 |
| `git diff --check` / 变更文档链接 | 通过 / 相对链接目标均存在 |

本轮没有重复执行全仓前端、全量 Rust 库或独立集成测试，前一轮结果仍按上一节记录；未操作真实账户或 CLI 配置，未进行多设备共享 Key、真实 keyring、跨平台运行、安装升级或发布验收。保留原有未跟踪 `out/`；后续清理未自动提交或推送。


## 账号基础数据隔离与清理退出（2026-10-31）

本轮用户授权新增「退出并清除此账号本机数据」，并要求平台基础数据区分账号；多账号同时保持登录明确暂缓。保留上一节尚未提交的自有旧逻辑清理及用户 `out/`，不自动提交或推送。

改动范围：新增自有清理编排、IPC wrapper、确认框和结果展示；目录按平台地址 + 用户 ID 独立私密存储，迁移有效的旧单文件 v2；普通退出仍保留目录/供应商/工具配置。清理按账号复用全应用安全删除，清理快照后退出；保留登录账号记录、服务器地址和已记住密码；逐应用失败及 keyring/服务端退出分别报告。结果只存在 Auth Context 内存，在退出后通过独立可关闭弹窗展示，不占用登录表单；关闭或重新登录成功后清除。前端清理期间阻止新登录/轮询，隔离旧响应，账号变化关闭原详情页。

复用 ProviderService Adapter、操作锁、私密原子写入和既有配置审计/恢复，无新的核心配置写入路径。L0 自有模块 + L1 注册及五种语言入口；[ADR-0005](../adr/ADR-0005-account-local-cleanup.md)、[补丁登记](../development/upstream-patches.md)、基础数据文档及 AGENTS 已同步。

| 检查 | 本轮最终结果 |
|---|---|
| `pnpm typecheck` | 通过 |
| `pnpm format:check` | 通过 |
| `pnpm test:unit tests/zhishu` | 21 文件、236 项通过；登录/门禁 37 项，目录页 24 项，auth IPC 4 项 |
| `pnpm build:renderer` | 通过；既有浏览器数据过旧、duration 类歧义、混合导入与大 chunk 警告保留 |
| `cargo fmt --check --manifest-path src-tauri/Cargo.toml` | 通过 |
| `CC_SWITCH_TEST_HOME="$(mktemp -d /tmp/zhishu-cleanup-scope.XXXXXX)" cargo test --offline --manifest-path src-tauri/Cargo.toml --lib zhishu:: --quiet` | 204 项通过，无失败或忽略（取消登录历史清理后移除对应旧测试） |
| `cargo clippy --offline --manifest-path src-tauri/Cargo.toml -- -D warnings` | 通过 |
| `git diff --check` | 通过 |
| 旧接口/清理状态扫描 | 自有源码与测试中 `/api/v1`、`/v1/models`、`credentialsCleared`、旧清理函数无匹配；未恢复客户端 Key 创建/撤销 |

新增回归验证：同平台不同账号及同 ID 不同平台分别保存；清理只删目标账号且其他文件字节不变；旧 v2 迁移保持 0600；迁移写入失败保留旧文件并可重试；损坏/归属不明旧文件保留并报告；过时身份不清理不退出；部分删除保留计数；本机凭据与服务端退出同时失败仍丢弃内存会话；登录页保留账号记录、平台地址和记住密码，不调用删除入口；确认取消无副作用；普通退出不调用清理；清理结果跨门禁弹窗展示、可关闭且关闭后不留常驻内容；清理期间迟到目录响应不重建缓存；切换账号关闭旧详情；供应商同步失败/兼容性警告仅浮动提示，目录自身失败仍可见且可重试。

首轮前端新增测试发现两个当前 Vitest 不支持的 matcher 和一个详情按钮名称匹配错误，修正测试后全部通过，无遗留测试失败。测试使用临时配置目录、内存数据库、模拟凭据库及本机 HTTP 模拟响应，没有操作真实平台账号、真实密码或真实 CLI 配置。

本轮未执行全仓前端、全部 Rust 库/独立集成测试、桌面实机视觉验收、真实系统 keyring、Windows/Linux 运行及真实 CLI 调用；未打安装包、发布或推送。安全保留项、自建供应商、配置修改记录和恢复备份可能保留凭据；归属无法确认或系统凭据库清理失败时显示未完成，不宣称安全擦除。重新登录后的既有自动同步可能重新生成目录和托管供应商。

用户反馈修订：清理结果改为可关闭弹窗；保留登录记录、服务器地址与记住密码，后端不调用历史/密码删除函数，移除未发布报告的 loginHistory 字段并同步契约和五语文案。前端回归仍为 236 项，后端为 204 项。本次测试曾因地址标签误写为“服务器地址”失败，按实际“平台后端地址”翻译键修正后全量智枢前端回归通过。既有已删除的登录记录不做推测恢复，需用户重新填写登录。

## 清理弹窗视觉修订（2026-10-31）

用户反馈确认框正文贴边、结果框过长。此次仅调整自有 `PlatformAccountCleanup`、`PlatformProviderDeletionResult`、五语文案及已有用例：统一正文边距与账号卡片，缩短主要说明，保留范围补充说明可展开，结果显示汇总与状态行、逐应用明细默认折叠，正文独立滚动，标题/关闭按钮/底部操作固定。复用上游 Dialog 和主题 token，未改通用 Dialog 或后端清理行为；L0/L1，ADR-0005 与补丁登记已更新。

已执行：`pnpm typecheck`、`pnpm format:check`、`pnpm build:renderer`、`git diff --check` 均通过；`pnpm test:unit tests/zhishu/components/PlatformLogin.test.tsx tests/zhishu/components/PlatformCatalogPage.test.tsx` 共 61 项通过。构建保留既有浏览器数据过旧、duration 类歧义、混合导入和大 chunk 警告。

通过 agent-browser 对实际 React 组件进行模拟数据视觉检查：1200×800 下确认框、成功结果与部分失败结果；亮/暗主题；13 个应用明细展开；640×480 下结果弹窗高度为 432px、关闭按钮处于视口内；360×600 下无横向溢出且关闭可见。模拟数据无真实账号/Key，不访问平台，不执行真实清理；临时预览代码及服务器、专用浏览器已清理。截图保存在本机 `/tmp/zhishu-cleanup-confirm-final.png`、`/tmp/zhishu-cleanup-result-final.png` 等临时路径。

本次未重跑 Rust 或全仓测试，未进行 Tauri 原生 WebView/真实账号清理验收，未提交或发布。前节后端验证记录仍是该功能最近一次执行结果。

## 全应用供应商删除结果弹窗（2026-10-31）

平台基础数据页的一键删除结果/错误由正文常驻展示改为自有 `PlatformProviderDeletionDialog`：显示当前账号、删除/保留计数、失败提示与默认折叠的逐应用明细，固定关闭操作；关闭重置本次 mutation 结果，后续删除可重新显示。该入口关闭重复 toast，应用页菜单沿用原有 toast。复用既有删除 IPC、身份隔离、局部缓存失效和通用 Dialog，L0 自有组件/Hook + L1 五语翻译；补丁登记与基础数据文档已同步，未改后端或上游核心。

已执行：`pnpm test:unit tests/zhishu/components/PlatformCatalogPage.test.tsx tests/zhishu/hooks/usePlatformProviderDeletion.test.tsx` 32 项通过；`pnpm typecheck`、`pnpm format:check`、`pnpm build:renderer`、`git diff --check` 通过，构建既有警告保留。测试覆盖弹窗不属于目录正文、关闭清除结果、错误关闭后重试、部分失败明细、退出/账号切换后的迟到响应隔离，以及应用级通知兼容。初轮测试补齐了新翻译资源、异步 mutation reset 等待和重复状态文本查询，最终无失败。

使用 agent-browser 对实际组件的模拟 13 应用结果检查：桌面视口摘要及默认折叠明细、640×480 展开后的关闭按钮可见、关闭后结果消失。截图 `/tmp/zhishu-provider-deletion-result.png`；临时预览代码/服务器和专用浏览器已清理。没有连接真实平台或执行实际删除，未重跑后端或全仓测试，未进行原生 WebView 验收，未提交或发布。

## 平台详情 UI 与滚轮修复（2026-10-31）

自有 `PlatformCatalogDetails` 调整为固定标题/页签、独立正文滚动的 600px 响应式抽屉；概览分卡展示基本信息、额度周期用量及显式 Key 入口。新增自有 `PlatformCatalogModels`，支持名称/ID 本地筛选、匹配数量、空态及展开参数。保留白名单字段和未知值语义，切换到模型页签卸载 Key 明文状态，不触发同步或 Key 请求。复用 Sheet/Tabs/Collapsible/Input/主题 token，无后端契约或存储变化；L0 自有展示及测试 + L1 五语文案，补丁登记与功能文档已同步。

用户反馈概览鼠标滚轮失效：内部 SheetBody 的 `scroll-stable` 强制 `overflow-y: scroll`，配合 `overscroll-contain` 截断滚动向父级传递。改为普通内容 div，仅外层页签面板负责滚动；未改通用 Sheet 或全局 CSS。

已执行：`pnpm typecheck`、`pnpm format:check`、`pnpm build:renderer` 通过；`pnpm test:unit tests/zhishu/components/PlatformCatalogPage.test.tsx` 25 项通过，覆盖模型名/ID 筛选、大小写和空白、空结果/清空筛选，以及切换页签清除已显示 Key、无额外 Key/同步调用。构建保留既有浏览器数据库过旧、duration 类歧义、混合导入与大 chunk 警告。

浏览器使用本机临时预览加载实际组件和模拟数据，检查亮/暗主题及窄视口。agent-browser 的低层 wheel 命令在本次会话落到了遮罩，因此改用同一专用 Chromium 会话的 CDP `Input.dispatchMouseEvent`，明确指定内容坐标发送真实 wheel 输入。复现旧嵌套样式时正文滚动位置保持 0；修复后 360×640 概览向下至 228px、1280×700 至 168px，API Key 卡片底部可达；模型列表向下至 350px；反向滚轮回到 0，背景保持 0。1280×900 概览全部容纳，无须滚动。截图 `/tmp/zhishu-details-overview-fixed.png`；预览源码、服务器和专用浏览器已清理。

没有操作真实平台、Keyring 或 CLI 配置，未执行原生 Tauri WebView、Windows/Linux 实机验收；本轮未重跑 Rust 或全仓测试。未提交、推送或发布。原有未提交改动和用户 `out/` 保留。

## zws 提交整理与 Harness 接入剔除（2026-10-31）

在独立整理工作树中，从官方 4.0.21 基线重新组织 zws 的二开成果。删除二开新增的 DeepSeek Harness 应用、原生配置适配、平台同步入口、前端表单和相关测试；保留上游 DeepSeek 供应商/模型预设及 WorkBuddy。原发布和同步文档中的 Harness 描述仅保留为历史记录。

已执行：`./node_modules/.bin/tsc --noEmit` 通过；本次修改的现存前端/测试文件 Prettier 检查通过；`cargo fmt --check --manifest-path src-tauri/Cargo.toml` 通过；9 个相关前端测试文件共 202 项通过；`CC_SWITCH_TEST_HOME` 指向独立临时目录的 `cargo test --offline --manifest-path src-tauri/Cargo.toml --lib zhishu:: --quiet` 共 149 项通过。运行时代码与测试中的 Harness/DSH 标识扫描无匹配。另已编译 Rust 测试目标；未执行全仓测试、Clippy、桌面实机或原生 CLI 验收。

本次不迁移或删除用户机器上的旧 Harness 配置及数据库行。新分支不再识别该应用 ID，旧数据需要在原发布版本管理或迁出。提交重组与最终文件树核对见 [整理记录](commit-reorganization.md) 和 [ADR-0006](../adr/ADR-0006-zws-history-without-harness.md)。

## DSH 平台供应商适配（2026-10-31）

用户场景：已登录平台并取得本机基础数据后，可在 DSH 页面同步托管供应商；全应用同步、按账号删除和清理退出也覆盖 DSH。验收范围为兼容模型映射、稳定分组 ID、完整模型目录、重复同步、Key 轮换、原生 profile 写入和账号归属删除。复用上游 4.0.22 的 DSH ProviderService、`dsh_config` 路径/锁/凭据引用及平台两阶段同步；没有新 IPC、schema、凭据存储或上游核心补丁。改动均在智枢模块和自有测试/文档，侵入等级 L0；`UPSTREAM_PATCHES.md` 无新增条目。

后续按用户反馈复核真实本机配置：手动 `shoudongtianjia` 已在 DSH profile，平台托管项的数据库行却标记为未写入原生配置；两者的协议、网关地址及 Key 相同。因此将 DSH 同步改为新建时原子写入原生 profile，已有数据库项在原生节点缺失时补写。隔离回归覆盖与手动项共存、首次可见、移除后再次同步恢复、重复同步不改写、Key 轮换和按账号删除；未改动真实 DSH profile 或凭据。

已执行：`pnpm typecheck` 通过；DSH 入口与白名单前端测试 16 项通过，本次 TS 文件 Prettier 检查通过；隔离 `CC_SWITCH_TEST_HOME` 的 `cargo test --locked --offline --lib zhishu:: -- --test-threads=1` 151 项通过；隔离目录的 `dsh_commands` 集成测试 3 项通过；`cargo fmt --check`、`cargo clippy --locked --offline -- -D warnings`、`git diff --check` 通过。Rust 测试为 DSH 设置临时 `DSH_HOME`，验证 profile 不含明文 Key、凭据引用文件存储 Key、重复同步不改写文件、轮换和删除；不访问真实用户配置。

仓库级 `pnpm format:check` 未通过，仅报告未修改的 `src/App.tsx`；本次改动的 TS 文件单独检查通过。未执行全仓前端/全部 Rust 测试、真实 DSH CLI 调用、真实平台账号、Windows/Linux 运行、安装升级或发布验收。回退可撤销本次智枢适配与白名单，已产生的 DSH 托管项需按账号使用现有删除入口处理；不自动迁移或清理用户配置。

后续现场复核（2026-10-31）：开发数据库中的手动项和平台项均标记已写入原生配置，`web`、`desktop`、`headless` profile 也均含两项；先前“仅数据库保存”的诊断已不能解释当时的不可见。平台项有 177 个模型，其中 6 个曾含 DSH schema 不接受的 `reasoningEfforts.ultra`，手动项只有 5 个模型。已在智枢 DSH 模型适配中限制思考档位，并在重复同步时清理已有托管模型的非法档位；本轮未直接修改真实 `~/.dsh`。期间用户侧配置发生更新，复核时数据库与原生 profile 已无 `ultra`。当前打开的 DSH 模型选择器已实际显示「智枢平台 · 智枢router-ccs-标准」及其 GPT-6-Luna 等模型，手动项同时可见；不能仅凭这次观察断定是谁触发了重同步。

本轮验证：隔离 `CC_SWITCH_TEST_HOME` 与 `DSH_HOME` 的 `cargo test --locked --offline --lib zhishu:: -- --test-threads=1` 共 152 项通过；最终代码的 `cargo test --locked --offline --lib dsh -- --test-threads=1` 共 66 项通过，包含 DSH 原生 YAML 不含 `ultra`、重复同步不改写、已有非法档位清理；`cargo fmt --check`、`cargo clippy --locked --offline -- -D warnings` 和 `git diff --check` 通过。已只读查看本机 DSH 模型选择器，确认平台分组可见；未执行平台模型实际请求、完整仓库测试或发布。REA 对已安装 DSH 应用包的外部分析被自动审批阻止，因此以本机 `dsh 0.2.1-rc.5` 版本、官方同版本 schema、隔离回归和当前界面观察为依据，不宣称安装包内部已核验。

## 文档分类与源码核对（2026-10-31）

范围：`docs/zhishu` 按 development、features、api、releases、history、adr、upstream-sync 分类，根目录只留总索引。合并两份提交整理映射，旧审查报告的有效结论并入本页；发布指南与各版本证据分离。依据当前源码更新 0.0.3 / 上游 4.0.22 / schema 24、真实模块位置和 DSH 平台同步范围。旧分支记录保留原适用范围，不覆盖前面的历史测试结果。

复用当前 ProviderService/Adapter、已有 IPC 和开发/发布脚本的事实说明，不新增业务实现。L0 文档整理，Vite/开发脚本只修正文档提示路径（固定入口按 L1 复核），`AGENTS.md` 和贡献指南同步引用；补丁登记复核 ZS-006、ZS-222。无数据库、凭据、原生配置、平台请求或跨平台行为变更。

| 本次检查 | 结果 |
| --- | --- |
| 文档索引可达性 | 27 份文档全部可从总索引访问，无孤立文档 |
| 本地 Markdown 引用 | 122 处本地链接及其中 6 个章节锚点有效，含相邻网关仓库引用 |
| 当前文档源码入口 | 59 个完整源码路径存在；历史迁移表和目标示例单独标识 |
| 版本、schema、白名单与 IPC 静态核对 | 源码版本 0.0.3、schema 24；13 项前后端平台白名单一致，含 DSH；21 个前端 wrapper 命令均已实际注册 |
| JSON 示例解析 | 12 段均可解析 |
| `pnpm typecheck` | 通过；初次只读沙箱无法打开 pnpm 存储锁，正常权限重跑成功 |
| 开发脚本回归 | `node --test scripts/zhishu/macos-dev-sign.test.mjs scripts/zhishu/dev-isolated.test.mjs`：9 项通过，1 项真实钥匙串测试按默认设置跳过 |
| 修改的工具入口格式与语法 | Vite / macOS 签名模块 Prettier、Node 语法检查及 `sh -n` 均通过 |
| `git diff --check` | 通过 |

本次未运行全量前端/Rust 测试、构建、真实钥匙串、CLI 请求、安装升级或发布验收；旧记录中的“已执行”不属于本轮结果。保留用户原有 DSH 适配与验证内容、其他源码改动及 `out/`；未创建提交或推送。回退仅恢复本次文档位置/内容与提示引用，不涉及运行时数据。

最终检查期间发现其他任务新增 ADR-0008 及多处前后端未提交修改。上表 27 份文档和 122 处链接包含工作区中的并行 ADR；本次提交仅整理原有 26 份文档，并行 ADR、索引条目及实现说明留在工作区，由对应任务提交。本次类型/脚本测试不作为并行实现的验证证据。
