# 目录与功能提交整理验证

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

范围、复现条件及遗留风险见 [审查报告](REVIEW-2026-10-29.md)。本节记录后续修复，不覆盖上面目录整理阶段的历史结果。

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

测试只使用本机模拟平台、临时目录和模拟凭据库。未运行全量测试、Clippy、前端类型检查或构建，也未进行真实平台登录、真实凭据库、Windows/Linux 与安装包验证。HTTP 明文传输及地址附带内容的清除行为见 [平台登录文档](platform-login.md)。


## 客户端专用 API 改造（2026-10-31）

范围：按 [客户端专用接口契约](client-api-integration.md) 将平台登录、恢复、账户、基础同步与额度查询统一到 `/api/zhishu/v1/client`，删除自有旧平台调用、Key 查找/创建/替换/撤销与网络模型发现逻辑，不保留旧接口回退。前后端契约、额度与订阅展示、版本 2 私密快照、网关地址适配及相关文档同步更新。

侵入等级为 L0 自有业务、L1 注册/翻译、L2 可选共享 Provider 元数据；复用 ProviderService、应用锁、路径解析、原子配置发布/恢复、keyring 与账户查询隔离。不修改上游 DAO/schema、代理或通用配置引擎。决策、兼容与回退见 [ADR-0004](adr/ADR-0004-client-api-only.md)，上游薄接入见 [补丁登记](UPSTREAM_PATCHES.md) CLIENT-20261031-01～03。

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
