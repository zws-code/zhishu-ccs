# ADR-0008：平台扩展与上游补丁收敛

- 日期：2026-10-31
- 状态：用户已授权实施；以本次验证记录为准，不代表发布批准
- 上游基线：4.0.22 / b76426741ef93069e2f374ac01ca7ef5760e1d0e

## 场景、范围与验收

平台同步负责组装 Provider，再通过 Adapter 调用上游服务。用户确认保留完整配置历史和还原、完整 WorkBuddy 支持，以及事件监听清理、macOS Dock 和 OpenCode V2 凭据读取的独立修复。Claude 模型档位补齐仅用于平台供应商。

本次撤销通用 Claude 投影、直连写入、编辑器和退出代理中的二开自动补齐；平台 Adapter 已组装全部档位，继续复用原有 ProviderService 和关键字段发布路径。迁出 OpenCode、Pi、MCode、Kimi 的平台测试辅助函数，Kimi 选择读取迁到 Adapter。恢复 GeneralSection 未使用的 simplified 分支、presentation 未赋值变量及 API 索引无调用者的自有重导出对应的上游实现，减少 WorkBuddy 成功操作的重复查询。

涉及 live/project/claude.rs、services/provider/{mod,claude_direct,claude_editor}.rs、mode/controller.rs、四个工具 config 文件、database/dao/providers.rs、四个前端文件及对应测试；新增逻辑集中在 zhishu/integrations/ccswitch/。自有代码和测试为 L0，前端补丁收敛为 L2，Rust 核心恢复和通用元数据入口为 L3。无 L4 改动。

## 元数据更新方案

旧订阅标记迁移从旧快照直接 save_provider，会保存整行，可能覆盖并发编辑或重建已删除项。

候选方案：直接调用 ProviderService::update 会触发原生配置发布；仅增加平台锁不能约束手动操作；将平台字段 SQL 放入上游会污染业务边界。本次选择增加通用 ProviderService 元数据修改入口，复用应用切换锁和 DAO。DAO 在同一数据库连接锁内读取最新原始 JSON，仅 UPDATE meta，不插入、不改其他列；平台归属判断留在 Adapter。保留未知元数据，缺失项返回未修改。路由应用继续等待既有 pending 恢复，其他应用使用既有切换锁。

## 风险与验证

不迁移凭据、不改变数据库 schema、IPC 或真实 CLI 配置；不改 WorkBuddy 局部恢复、历史还原和 DSH 原生同步。主要风险是 Claude 退出代理或编辑预览的行为回归、Kimi 模型别名解析，以及元数据更新误改其他账号。通过手动/平台 Claude、模式恢复与金标、平台全应用同步、元数据并发/未知字段/不存在项、前端查询和配置历史测试验证。测试使用隔离目录，不以模拟测试代替真实平台、多系统 keyring 和 CLI 验收。

## 上游合并、回退与复审

四个工具配置文件及无效 UI 补丁尽量恢复上游，原有上游函数和 API 保留。新增核心 API 无平台字段依赖。补丁登记与验证记录同步更新。回退时仅反向应用本次代码差异，无 schema 降级或数据迁移；回退会恢复通用 Claude 补齐行为，需单独确认。工作区既有文档整理、开发签名和构建配置改动不属于本次修改。下次上游同步复审元数据 API 是否可被上游吸收。
