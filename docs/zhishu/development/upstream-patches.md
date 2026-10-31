# Zhishu CCS 上游补丁登记

> 这是按基线/事件累积的补丁台账。222 项清单针对 2026-10-29 的 4.0.21 快照，后续变更按事件追加；旧路径、版本和分支描述仅适用于该段，不是当前 4.0.22 的完整重新统计。当前现状见 [代码基线](baseline.md)。

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
