# 智枢开发与维护文档

> 核对日期：2026-10-31。当前说明以本地工作区的代码、IPC 注册、脚本和测试定义为准；历史记录中的执行结果仅适用于对应版本和分支，不代表本次重新验收。

## 按用途查阅

| 类型 | 文档 | 用途 |
| --- | --- | --- |
| 开发规范 | [二次开发实施标准](development/standard.md) | 强制工程边界、侵入等级与开发流程 |
| 开发基线 | [当前基线与限制](development/baseline.md) | 产品版本、上游基线、真实目录、存储和已知差异 |
| 上游补丁 | [补丁登记](development/upstream-patches.md) | 上游文件修改原因、关联决策与历史复核 |
| 开发环境 | [隔离开发](development/dev-isolation.md)、[macOS 开发签名](development/macos-dev-signing.md) | 启动入口、隔离范围与签名准备 |
| 功能 | [平台登录与会话](features/platform-login.md)、[平台基础数据与供应商](features/platform-managed-models.md) | 当前用户流程、凭据边界与本地同步 |
| 接口 | [客户端专用 API 契约](api/client-api.md) | HTTP 路由、DTO、字段映射和失败语义 |
| 发布 | [打包、发布与更新](releases/README.md) | 唯一发布操作入口；含各版本证据索引 |
| 架构决策 | [ADR 索引](adr/README.md) | 决策背景、范围、替代方案与回退 |
| 上游同步 | [同步报告](upstream-sync/README.md) | 各次合并来源、冲突、验证与风险 |
| 历史记录 | [验证记录](history/validation.md)、[提交整理映射](history/commit-reorganization.md) | 当时的测试结果、旧提交映射和恢复依据 |

## 当前代码入口

以下路径均相对仓库根目录；Rust 路径包含完整的 `src-tauri/src/` 前缀。

| 功能 | 前端 | Rust |
| --- | --- | --- |
| 平台登录、双重验证、会话 | `src/zhishu/auth/`、`src/zhishu/api/platformAuth.ts` | `src-tauri/src/zhishu/services/platform_auth.rs`、`src-tauri/src/zhishu/storage/platform_session.rs`、`platform_login_history.rs` |
| 目录、订阅、供应商同步与删除 | `src/zhishu/models/`、`src/zhishu/api/platformModels.ts`、`platformUsage.ts` | `src-tauri/src/zhishu/services/platform_models/`、`platform_cleanup.rs`、`client_api.rs` |
| 上游能力适配 | `src/zhishu/integrations/ccswitch/` | `src-tauri/src/zhishu/integrations/ccswitch/` |
| 配置历史、差异与还原 | `src/zhishu/config-history/` | `src-tauri/src/zhishu/storage/config_history.rs`、`src-tauri/src/zhishu/commands/config_history.rs` |
| WorkBuddy | `src/zhishu/tools/workbuddy/` | `src-tauri/src/zhishu/integrations/ccswitch/workbuddy.rs`、`workbuddy_config.rs` |
| 应用导航与品牌 | `src/zhishu/app/`、`src/zhishu/assets/` | `src-tauri/src/lib.rs` 薄注册 |
| 自有测试 | `tests/zhishu/` | 自有模块同文件测试，以及 `src-tauri/tests/` 集成测试 |
| 开发与发行 | `scripts/zhishu/`、`package.json` | Tauri 工具链与 `.github/workflows/` |

前端复用上游 UI、TanStack Query 和 API；Rust Adapter 复用 ProviderService 及既有配置写入与恢复。DSH 原生适配来自上游 4.0.22，平台同步接线在智枢 Adapter 中。应用可见性、MCP、Skills、路由、聚合模式和平台同步分别使用各自能力判断。

## 维护规则

- 当前行为更新到开发、功能、接口或发布操作文档；保留代码入口，不能用文档中的设计代替实现核查。
- ADR、上游同步报告、发布记录和验证记录保留发生时的事实；已被替代的内容标明适用分支及后续决策，不作为当前操作步骤。
- 本次合并两份提交整理记录；旧二开审查报告的有效结论合入验证记录，删除重复文件。发布指南中的版本证据移入对应版本记录，移除旧版默认下载表。
- 修改文档位置时同步修复仓库引用；不保留内容重复的旧入口。
