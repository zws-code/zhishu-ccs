# Zhishu CCS 上游补丁登记

> 本文件记录**真实存在**的对上游 CC Switch 已有文件的修改。只有审查实际 `git diff` 后才能填入记录。

| ID | 文件 | 修改目的 | 替代方案评估 | 侵入级别 | 验证方式 | 上游同步复核 |
|---|---|---|---|---|---|---|

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
