# 智枢开发与维护入口

## 功能归属

| 功能 | 前端 | Rust |
|---|---|---|
| 平台登录、双重验证、会话 | `src/zhishu/auth`、`api` | `zhishu/services/platform_auth.rs`、`zhishu/storage/platform_*` |
| 目录、模型、订阅、供应商同步/删除 | `src/zhishu/models` | `zhishu/services/platform_models` |
| 上游能力适配 | `src/zhishu/integrations/ccswitch` | `zhishu/integrations/ccswitch` |
| 配置历史、差异、还原 | `src/zhishu/config-history` | `zhishu/storage/config_history.rs`、`zhishu/commands/config_history.rs` |
| DeepSeek Harness / WorkBuddy | `src/zhishu/tools` | `zhishu/integrations/ccswitch`、`zhishu/services/dsh_state.rs` |
| 应用导航与品牌 | `src/zhishu/app`、`assets` | Tauri 注册入口薄挂载 |
| 自有测试 | `tests/zhishu` | 自有模块内 `#[cfg(test)]`，复用上游集成测试 |
| 开发与发行 | `scripts/zhishu` | 继续使用 Tauri 工具链 |

前端复用上游 UI、查询缓存和 API；Rust Adapter 复用 ProviderService 及原子写入/恢复机制。
保留必要 AppType、序列化、核心观察点和入口补丁，详见逐文件登记。
`src/.env.example`、`src/whats-new/0.0.1.json`、Tauri icons、根发行示例因加载器/打包器约定保持原位置。

## 文档

- [强制开发规范](SECONDARY_DEVELOPMENT_STANDARD.md)
- [实际基线与限制](BASELINE.md)
- [上游补丁登记](UPSTREAM_PATCHES.md)
- [目录与历史整理 ADR](adr/ADR-0002-structure-and-history.md)
- [新旧提交映射与回退](HISTORY_REWRITE.md)
- [测试执行记录](VALIDATION.md)
- [平台登录](platform-login.md)、[目录与托管供应商](platform-managed-models.md)
- [开发隔离](dev-isolation.md)、[macOS 开发签名](macos-dev-signing.md)
- [DSH 原生接入](tools/dsh-native.md)、[发行说明](zhishu-release.md)
