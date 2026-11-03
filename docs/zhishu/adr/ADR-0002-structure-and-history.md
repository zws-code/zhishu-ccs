# ADR-0002：自有模块归位与功能历史整理

> 适用范围：迁移表为 2026-10-29 的历史映射，旧路径与当时计划原样保留；当前位置以文档总索引和源码为准。

- 日期：2026-10-29
- 状态：实施方案；按用户要求整理当前功能分支
- 侵入等级：L3（保留已有核心扩展，迁移实现与调用边界）
- 上游锁定：CC Switch 4.0.21 / c4db63c60d2a1ddcb4bd9f81152b9c6126413659
- 整理前：f3485e06；备份分支 `backup/zhishu-before-structure-20261029`

## 背景与验收

平台认证、目录、配置历史及额外工具实现散落在上游目录；发布修复提交较多。
目标是保持已有功能和受控写入链路，将自有实现迁入命名空间、同步调用与测试，
登记实际相对上游的全部补丁，并将当前功能分支历史按功能归组。
不重写已发布主线，不推送或强推；保留上游祖先和本地历史备份。
工作区原有 AGENTS.md 与规范文档纳入治理提交，out/ 为既有产物，不纳入提交。

## 备选方案与选择

1. 只登记不迁移：不能解决目录污染，拒绝。
2. 复制核心建立第二套实现：违反 L4 红线，拒绝。
3. 移动自有文件、修正引用、保留薄挂载与必要扩展：采用。

前端在 src/zhishu 下按 auth、models、config-history、tools、app 分区；
调用上游查询/供应商能力集中于 integrations/ccswitch。
Rust 在 zhishu 下区分 commands、services、storage、integrations/ccswitch。
原始 ProviderService、MCP/Skills、配置写入与切换锁保持单一实现。
既有混合 IPC/service 模块本轮保持内部逻辑，统一从 zhishu::commands 导出。

## 无法仅通过新增文件解决的上游边界

- src/App.tsx、src/main.tsx、Sidebar：没有插件注册机制，必须挂载自有视图/认证。
- ProviderForm/Card/List、Settings：内部表单分派和状态必须引用自有组件；不复制整个页面。
- src-tauri/src/lib.rs：Tauri 编译期命令注册、状态初始化及公开 API 必须修改。
- commands/mod.rs、services/mod.rs、services/provider/mod.rs：移除旧声明，保留工具服务分派。
- ProviderService 的配置校验、用量凭据归一化原为子模块私有访问；迁到 Adapter 后仅扩展为 crate 可见。
- app_config/settings/provider 等枚举及序列化字段：既有存量协议，本轮保留并登记，不能通过包装替代穷举匹配。
- config/live/mode/MCP/Skills：既有配置历史观察点必须覆盖核心实际写入，外部包装不能捕获全部写入。
- package.json、Tauri 清单、CI：工具链固定入口，迁移脚本后必须更新路径。
- 数据库 schema：已有兼容补丁沿用 ADR-0001，本次不改变版本或迁移行为。

## 数据、安全、跨平台和更新

仅调整源文件与导入路径；保持 IPC 命令名、JSON 字段、存储路径、Keychain service/account、
应用标识、版本、端口、签名、更新 URL 与下载协议。
macOS/Windows 脚本须修正目录深度，Node/Python 单测验证；本机不能代替 Windows 安装验证。
原配置写入、备份、回滚、锁顺序保持原样。测试使用隔离 CC_SWITCH_TEST_HOME，不读写真实 CLI 配置。
不复制生产配置、token 或密钥到新文件。历史机密扫描仅做启发式检查，不能等同专业审计。

## 测试与回退

类型、格式、前端单测、renderer 构建，Rust fmt/clippy/隔离 test，开发/发布脚本单测。
迁移映射、遗留引用与补丁登记作静态复核。记录真实结果与未运行项。
原 HEAD 和工作区修改已备份；可切回备份分支恢复原代码。此次不执行运行时数据迁移。
历史整理前后应核对完整 tree 等价；目录迁移差异单独审核。
复审日期：本次交付前、下次上游合并前。

## 计划文件迁移清单

| 原位置 | 新位置 |
|---|---|
| `docs/architecture/dsh-native.md` | `docs/zhishu/tools/dsh-native.md` |
| `docs/dev-isolation.md` | `docs/zhishu/dev-isolation.md` |
| `docs/macos-dev-signing.md` | `docs/zhishu/macos-dev-signing.md` |
| `docs/platform-login.md` | `docs/zhishu/platform-login.md` |
| `docs/platform-managed-models.md` | `docs/zhishu/platform-managed-models.md` |
| `docs/zhishu-release.md` | `docs/zhishu/zhishu-release.md` |
| `scripts/build-release.mjs` | `scripts/zhishu/build-release.mjs` |
| `scripts/cos-release.mjs` | `scripts/zhishu/cos-release.mjs` |
| `scripts/cos-release.test.mjs` | `scripts/zhishu/cos-release.test.mjs` |
| `scripts/cos-requirements.txt` | `scripts/zhishu/cos-requirements.txt` |
| `scripts/dev-isolated.mjs` | `scripts/zhishu/dev-isolated.mjs` |
| `scripts/dev-isolated.test.mjs` | `scripts/zhishu/dev-isolated.test.mjs` |
| `scripts/dev-macos.sh` | `scripts/zhishu/dev-macos.sh` |
| `scripts/fixtures/macos-keychain-probe.c` | `scripts/zhishu/fixtures/macos-keychain-probe.c` |
| `scripts/macos-dev-sign-runner.sh` | `scripts/zhishu/macos-dev-sign-runner.sh` |
| `scripts/macos-dev-sign.mjs` | `scripts/zhishu/macos-dev-sign.mjs` |
| `scripts/macos-dev-sign.test.mjs` | `scripts/zhishu/macos-dev-sign.test.mjs` |
| `scripts/prepare-release.mjs` | `scripts/zhishu/prepare-release.mjs` |
| `scripts/publish-cos.py` | `scripts/zhishu/publish-cos.py` |
| `scripts/release-artifacts.mjs` | `scripts/zhishu/release-artifacts.mjs` |
| `scripts/release-artifacts.test.mjs` | `scripts/zhishu/release-artifacts.test.mjs` |
| `scripts/release-config.mjs` | `scripts/zhishu/release-config.mjs` |
| `scripts/release-config.test.mjs` | `scripts/zhishu/release-config.test.mjs` |
| `scripts/resume-release.mjs` | `scripts/zhishu/resume-release.mjs` |
| `scripts/resume-release.test.mjs` | `scripts/zhishu/resume-release.test.mjs` |
| `scripts/test_publish_cos.py` | `scripts/zhishu/test_publish_cos.py` |
| `src-tauri/src/commands/config_history.rs` | `src-tauri/src/zhishu/commands/config_history.rs` |
| `src-tauri/src/commands/deepseek_harness.rs` | `src-tauri/src/zhishu/commands/deepseek_harness.rs` |
| `src-tauri/src/commands/platform_auth.rs` | `src-tauri/src/zhishu/services/platform_auth.rs` |
| `src-tauri/src/commands/platform_http_log.rs` | `src-tauri/src/zhishu/services/platform_http_log.rs` |
| `src-tauri/src/commands/platform_login_history.rs` | `src-tauri/src/zhishu/storage/platform_login_history.rs` |
| `src-tauri/src/commands/platform_models.rs` | `src-tauri/src/zhishu/services/platform_models/mod.rs` |
| `src-tauri/src/commands/platform_models/adapter.rs` | `src-tauri/src/zhishu/integrations/ccswitch/platform_models.rs` |
| `src-tauri/src/commands/platform_models/base_data.rs` | `src-tauri/src/zhishu/services/platform_models/base_data.rs` |
| `src-tauri/src/commands/platform_models/catalog.rs` | `src-tauri/src/zhishu/services/platform_models/catalog.rs` |
| `src-tauri/src/commands/platform_models/connection.rs` | `src-tauri/src/zhishu/services/platform_models/connection.rs` |
| `src-tauri/src/commands/platform_models/delete.rs` | `src-tauri/src/zhishu/services/platform_models/delete.rs` |
| `src-tauri/src/commands/platform_models/sync_report.rs` | `src-tauri/src/zhishu/services/platform_models/sync_report.rs` |
| `src-tauri/src/commands/platform_session.rs` | `src-tauri/src/zhishu/storage/platform_session.rs` |
| `src-tauri/src/commands/platform_storage.rs` | `src-tauri/src/zhishu/storage/platform_storage.rs` |
| `src-tauri/src/config_history.rs` | `src-tauri/src/zhishu/storage/config_history.rs` |
| `src-tauri/src/deepseek_harness_config.rs` | `src-tauri/src/zhishu/integrations/ccswitch/deepseek_harness_config.rs` |
| `src-tauri/src/services/dsh_state.rs` | `src-tauri/src/zhishu/services/dsh_state.rs` |
| `src-tauri/src/services/provider/deepseek_harness.rs` | `src-tauri/src/zhishu/integrations/ccswitch/deepseek_harness.rs` |
| `src-tauri/src/services/provider/workbuddy.rs` | `src-tauri/src/zhishu/integrations/ccswitch/workbuddy.rs` |
| `src-tauri/src/workbuddy_config.rs` | `src-tauri/src/zhishu/integrations/ccswitch/workbuddy_config.rs` |
| `src/assets/icons/logo.png` | `src/zhishu/assets/logo.png` |
| `src/components/platform/PlatformBulkSyncButtons.tsx` | `src/zhishu/models/components/PlatformBulkSyncButtons.tsx` |
| `src/components/platform/PlatformCatalogDetails.tsx` | `src/zhishu/models/components/PlatformCatalogDetails.tsx` |
| `src/components/platform/PlatformCatalogKey.tsx` | `src/zhishu/models/components/PlatformCatalogKey.tsx` |
| `src/components/platform/PlatformCatalogPage.tsx` | `src/zhishu/models/components/PlatformCatalogPage.tsx` |
| `src/components/platform/PlatformConnectButton.tsx` | `src/zhishu/models/components/PlatformConnectButton.tsx` |
| `src/components/platform/PlatformLogin.tsx` | `src/zhishu/auth/PlatformLogin.tsx` |
| `src/components/platform/PlatformProviderDeletionResult.tsx` | `src/zhishu/models/components/PlatformProviderDeletionResult.tsx` |
| `src/components/providers/PlatformUsageFooter.tsx` | `src/zhishu/models/components/PlatformUsageFooter.tsx` |
| `src/components/providers/forms/DeepSeekHarnessFormFields.tsx` | `src/zhishu/tools/deepseek-harness/DeepSeekHarnessFormFields.tsx` |
| `src/components/providers/forms/WorkBuddyProviderForm.tsx` | `src/zhishu/tools/workbuddy/WorkBuddyProviderForm.tsx` |
| `src/components/providers/forms/helpers/deepseekHarnessFormUtils.ts` | `src/zhishu/tools/deepseek-harness/deepseekHarnessFormUtils.ts` |
| `src/components/providers/forms/hooks/useDeepSeekHarnessFormState.ts` | `src/zhishu/tools/deepseek-harness/useDeepSeekHarnessFormState.ts` |
| `src/components/settings/AppConfigHistoryButton.tsx` | `src/zhishu/config-history/AppConfigHistoryButton.tsx` |
| `src/components/settings/ConfigChangeHistoryPanel.tsx` | `src/zhishu/config-history/ConfigChangeHistoryPanel.tsx` |
| `src/components/settings/ProviderSyncReportView.tsx` | `src/zhishu/config-history/ProviderSyncReportView.tsx` |
| `src/components/shell/SidebarTopBar.tsx` | `src/zhishu/app/SidebarTopBar.tsx` |
| `src/config/deepseekHarnessProviderPresets.ts` | `src/zhishu/tools/deepseek-harness/deepseekHarnessProviderPresets.ts` |
| `src/contexts/PlatformAuthContext.tsx` | `src/zhishu/auth/PlatformAuthContext.tsx` |
| `src/hooks/useConfigMembership.ts` | `src/zhishu/integrations/ccswitch/useConfigMembership.ts` |
| `src/hooks/usePlatformProviderDeletion.ts` | `src/zhishu/models/usePlatformProviderDeletion.ts` |
| `src/lib/api/configHistory.ts` | `src/zhishu/config-history/api/configHistory.ts` |
| `src/lib/api/dsh.ts` | `src/zhishu/tools/deepseek-harness/api/dsh.ts` |
| `src/lib/api/platformAuth.ts` | `src/zhishu/api/platformAuth.ts` |
| `src/lib/api/platformModels.ts` | `src/zhishu/api/platformModels.ts` |
| `src/lib/api/platformUsage.ts` | `src/zhishu/api/platformUsage.ts` |
| `src/lib/configHistoryDiff.ts` | `src/zhishu/config-history/configHistoryDiff.ts` |
| `src/lib/query/additiveLiveState.ts` | `src/zhishu/integrations/ccswitch/additiveLiveState.ts` |
| `src/lib/query/configHistory.ts` | `src/zhishu/config-history/query/configHistory.ts` |
| `src/lib/query/dsh.ts` | `src/zhishu/tools/deepseek-harness/query/dsh.ts` |
| `src/lib/query/platformCatalog.ts` | `src/zhishu/models/query/platformCatalog.ts` |
| `src/lib/query/platformCatalogKeys.ts` | `src/zhishu/models/query/platformCatalogKeys.ts` |
| `src/lib/query/platformProviders.ts` | `src/zhishu/integrations/ccswitch/platformProviders.ts` |
| `src/lib/query/platformUsage.ts` | `src/zhishu/models/query/platformUsage.ts` |
| `tests/components/ConfigChangeHistory.test.tsx` | `tests/zhishu/components/ConfigChangeHistory.test.tsx` |
| `tests/components/DeepSeekHarnessFormFields.test.tsx` | `tests/zhishu/components/DeepSeekHarnessFormFields.test.tsx` |
| `tests/components/DeepSeekHarnessProviderForm.test.tsx` | `tests/zhishu/components/DeepSeekHarnessProviderForm.test.tsx` |
| `tests/components/DshSurface.test.ts` | `tests/zhishu/components/DshSurface.test.ts` |
| `tests/components/PlatformBulkSyncButtons.test.tsx` | `tests/zhishu/components/PlatformBulkSyncButtons.test.tsx` |
| `tests/components/PlatformCatalogPage.test.tsx` | `tests/zhishu/components/PlatformCatalogPage.test.tsx` |
| `tests/components/PlatformConnectButton.test.tsx` | `tests/zhishu/components/PlatformConnectButton.test.tsx` |
| `tests/components/PlatformLogin.test.tsx` | `tests/zhishu/components/PlatformLogin.test.tsx` |
| `tests/components/PlatformUsageFooter.test.tsx` | `tests/zhishu/components/PlatformUsageFooter.test.tsx` |
| `tests/components/ProviderCard.platformBadge.test.tsx` | `tests/zhishu/components/ProviderCard.platformBadge.test.tsx` |
| `tests/components/WorkBuddyProviderForm.test.tsx` | `tests/zhishu/components/WorkBuddyProviderForm.test.tsx` |
| `tests/config/deepseekHarnessProviderPresets.test.ts` | `tests/zhishu/config/deepseekHarnessProviderPresets.test.ts` |
| `tests/config/platformConfig.test.ts` | `tests/zhishu/config/platformConfig.test.ts` |
| `tests/hooks/useDeepSeekHarnessFormState.test.tsx` | `tests/zhishu/hooks/useDeepSeekHarnessFormState.test.tsx` |
| `tests/hooks/usePlatformProviderDeletion.test.tsx` | `tests/zhishu/hooks/usePlatformProviderDeletion.test.tsx` |
| `tests/hooks/useTauriEvent.test.tsx` | `tests/zhishu/hooks/useTauriEvent.test.tsx` |
| `tests/lib/configHistoryDiff.test.ts` | `tests/zhishu/lib/configHistoryDiff.test.ts` |
| `tests/lib/navigation.test.ts` | `tests/zhishu/lib/navigation.test.ts` |
| `tests/lib/platformAuth.test.ts` | `tests/zhishu/lib/platformAuth.test.ts` |
| `tests/lib/platformModels.test.ts` | `tests/zhishu/lib/platformModels.test.ts` |

## 验证期间的边界复核

- App 的平台删除菜单展示迁至自有组件；变更仍由原 Hook 管理，菜单关闭不取消操作。
- 持久化 PlatformManagedProvider 类型仍保留原位置，不改变其定义、序列化或 API。
- clippy 检出原 commands/misc.rs 中 launch_platform_agent/platform_agent_launch 两个无调用函数。自动审批拒绝删除旧实现及元数据迁移，故保留原代码，本轮记录为基线问题，不通过 allow(dead_code) 掩盖。

- 平台 Provider 创建、更新、切换、删除、当前状态及 live 读取统一通过 `zhishu/integrations/ccswitch/providers.rs` 转交真实上游服务；不另建持久化或写盘实现。
- TS 的配置成员查询归入 `integrations/ccswitch/useConfigMembership.ts`，复用已有 Pi/OMP、OpenClaw、Hermes 查询。
