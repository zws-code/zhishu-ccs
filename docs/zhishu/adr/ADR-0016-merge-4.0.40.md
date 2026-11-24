# ADR-0016：合并上游 v4.0.40

- 日期：2026-11-24；等级：L3。
- 状态：按用户“合并上游 tag v4.0.40”执行本地同步；不代表发布批准。
- 复审：下次上游同步或实际跨平台升级验收时。

## 背景与目标

产品 0.0.11 已包含官方 v4.0.37，起点 `12cfb27a9ee630ffa4ac4675358b40777d0e90f7`。官方 v4.0.40 提交为 `f19a9d4f4f08bda03106a7be87c77cbcdf55b019`，带来 SQLite WAL/缓存、一致性备份、SQL 导入和代理安全加固、Grok 官方账号及测试/Skills 模块拆分。目标为保留上游祖先与智枢既有行为，并完成自动回归。

## 方案与选择

1. 仅挑选功能提交：易漏安全修复且损失稳定 tag 的祖先，不采用。
2. 以官方整文件覆盖：会丢失产品身份、平台展示、备份路径及共存保护，不采用。
3. 独立 `codex/sync-upstream-4.0.40` 三方合并，按语义迁移补丁，再快进本地 `ccs-dev`：采用。复用原 Provider、DAO、代理、Skills、IPC 与上游安全实现，不引入平行核心。

核心修改不能仅靠新增包装解决：SQLite 连接初始化/备份、Skills 内部目标写入及删除、macOS 原生托盘依赖与固定 command/产品入口都在上游内部。智枢既有 Adapter 保持，Skills 指纹及所有目标的归属检查迁入 `skill/sync.rs`、`update.rs`、`backup.rs`，原回归迁入上游拆分的测试文件。保留 crate 根公开导出、`copy_to_app` 与原 `providerConfigUtils` 导入路径及旧用量类型/helper（新日志优先使用后端 freshInputTokens，避免重复扣减缓存 token）；保留上游此次删除的 `claude.yml`，不把 v4.0.37 的单次删除授权扩大到本轮。

## 数据与安全

主数据库 schema 仍为 27；采用上游 WAL、索引和读缓存。DAO 的 update/rollback/authorizer hook 统一失效缓存，智枢 metadata 写入仍走同一连接。备份/导出沿用 SQLite Backup API 获取一致快照，不直接复制运行中的主数据库文件。关闭/重启复用 checkpoint；临时备份和恢复保持上游错误处理。

智枢设备备份路径与 `.zhishu-ccs` 仍保留。产品版本、identifier、binary、深链和更新渠道不变；平台 OAuth/keyring、账号目录、两阶段同步、13 应用白名单和原生配置归属不变。上游 Host、SSRF、SQL 导入等安全校验完整接入。测试使用临时目录和回环服务，不访问真实账号或修改 CLI 配置。

## 验证与回退

实际结果见 [同步报告](../upstream-sync/2026-11-24.md)；要求 frozen install、TypeScript/格式、前端全量、renderer 构建、Rust fmt/Clippy all-targets/隔离全量、发布模拟及身份/文档核对。

代码回退用 `git revert -m 1 <本轮 merge 提交>`，第一父为上述产品起点，不重写历史。schema 未升版不等于可以随意覆盖数据库：实际运行后须先关闭应用并确认 WAL checkpoint 或用应用一致性导出/备份；不得遗漏仍含已提交数据的 `-wal`，不得在运行中删除 `-wal`/`-shm`。需要恢复时使用独立完整备份，保持用户配置、pending、历史恢复备份和 keyring 的各自边界，不整目录回填。

真实系统凭据库、跨平台构建/安装升级、macOS 全屏托盘与 CLI 端到端验收仍需独立执行；自动测试不代替实机验收。
