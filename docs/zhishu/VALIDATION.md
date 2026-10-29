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
