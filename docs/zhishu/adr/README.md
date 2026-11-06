# 架构决策记录（ADR）

涉及 L3/L4、持久化数据、应用身份、签名及自动更新策略的变更必须建立 ADR。模板包含背景、目标、候选方案、选择与理由、上游合并影响、风险、测试、迁移与回滚、批准状态及复审日期。

| 记录 | 范围与后续关系 |
| --- | --- |
| [ADR-0001：4.0.21 合并兼容](ADR-0001-merge-4.0.21.md) | 历史合并与 v23 OMP 兼容；当前 schema 已到 v24，见 ADR-0007 |
| [ADR-0002：模块归位与历史整理](ADR-0002-structure-and-history.md) | 2026-10-29 的目录迁移映射，旧路径仅作追溯 |
| [ADR-0003：Windows EXE 与 Linux 暂停](ADR-0003-windows-exe-release.md) | 当前打包策略；具体发布例外按版本记录 |
| [ADR-0004：客户端专用 API](ADR-0004-client-api-only.md) | 替代客户端 Key 生命周期；账号目录位置由 ADR-0005 延伸 |
| [ADR-0005：账号目录与清理退出](ADR-0005-account-local-cleanup.md) | 当前账号存储、旧 v2 迁移及清理边界 |
| [ADR-0006：独立历史整理分支](ADR-0006-zws-history-without-harness.md) | 当时剔除二开 Harness；不代表当前分支不支持上游 DSH |
| [ADR-0007：合并上游 4.0.22](ADR-0007-merge-4.0.22.md) | 当前上游 DSH 与 v24 迁移；状态仍按原决策保留，不等于发布批准 |
| [ADR-0008：平台与上游边界收敛](ADR-0008-upstream-patch-convergence.md) | Claude 平台档位、平台适配/夹具迁出、通用元数据更新和无效补丁撤回 |
| [ADR-0009：产品共存隔离](ADR-0009-product-coexistence.md) | 派生资源命名、配置归属、后台恢复、云同步与共享 Skills 保护 |
| [ADR-0010：移除开发隔离](ADR-0010-remove-development-isolation.md) | 正常开发路径、保留签名/测试隔离、旧开发数据不自动迁移 |
| [ADR-0011：合并上游 v4.0.25](ADR-0011-merge-4.0.25.md) | 4.0.23 → v4.0.25，代理/MCP/投影/用量更新与智枢身份、共存边界 |
| [ADR-0012：合并上游 v4.0.26](ADR-0012-merge-4.0.26.md) | v4.0.25 → v4.0.26，DAO/后台 IPC/窗口查询/代理/DSH 与平台边界 |

ADR 保留做出决策时的事实；当前代码能力见 [基线](../development/baseline.md)，实际执行结果见 [验证记录](../history/validation.md)。
