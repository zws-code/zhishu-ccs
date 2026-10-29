# AGENTS.md — Zhishu CCS AI 开发强制规范

> 本仓库是 **Zhishu CCS（CC Switch Fork）**。任何 Codex、Claude Code 或其他 Agent 修改代码前，必须阅读 `docs/zhishu/SECONDARY_DEVELOPMENT_STANDARD.md`。当某个任务与规范冲突时，应提出冲突点及替代方案，而不是静默突破。

## 1. 总目标

长期保留上游 CC Switch 可合并性。实现“智枢自有模块隔离、上游核心最小补丁、数据可迁移、发布可回滚”。

## 2. 动手前必须做的事

1. 确定当前任务范围、验收标准，以及相关文件。
2. 先搜索现有 CC Switch 代码，识别可复用的 Provider、配置写入、MCP、Skills、安装、代理、DAO、IPC 能力。
3. 给出拟新增自有文件与拟修改上游文件清单。
4. 为每个上游改动说明“为什么无法通过 Adapter、包装器或新增文件解决”。
5. 判定 L0–L4 侵入等级；L3/L4 或涉及高风险数据迁移时先输出 ADR 方案。
6. 评估 Windows/macOS、CLI 外部配置、鉴权/凭据及自动更新影响。

## 3. 代码放置

- React/TS 业务优先置于 `src/zhishu/`。
- Rust/Tauri 业务优先置于 `src-tauri/src/zhishu/`。
- 上游接口封装集中于 `integrations/ccswitch/` 或 Rust 对应 Adapter。
- 允许对 App 导航/入口、`src-tauri/src/lib.rs` 等做最小挂载；必须在补丁登记中追踪。
- 不得将智枢特定用户/平台/权限逻辑散落到上游核心服务、DAO、代理路由中。
- 不得复制上游 Provider/CLI 配置/Skills/MCP 核心再维护独立平行版本。
- 前端采用渐进式改造，不无故重写原版大模块。

## 4. 数据和安全

- 不得直接无备份覆盖用户 `~/.codex`、`~/.claude` 等配置，优先调用 CC Switch 原有受控配置写入机制。
- 不得提交密钥/Token/签名私钥/生产配置，不得记录敏感头或明文密钥日志。
- 平台用户 API Key 必须最小权限、可撤销；新手模式 Key 获取/创建必须幂等。
- 本地数据库及应用身份变更必须具备升级、备份、共存与回退分析。
- 不要把示例代码里的 Adapter 方法名当作上游已存在 API；修改前确认实际签名。

## 5. Git 和补丁

- `origin/main` 为可发布主线，`upstream/main`（或锁定 tag/SHA）为官方同步来源。
- 功能从 `feat/*`、修复从 `fix/*`、同步从 `sync/upstream-*` 开始。
- 禁止重写已发布主线历史和直接在 main 上做带冲突的上游合并。
- 触碰上游已有文件，更新 `docs/zhishu/UPSTREAM_PATCHES.md`；上游重大修改更新 ADR。
- 上游合并时审查冲突、兼容性和数据，不以 `git merge` 成功作为验收。

## 6. 验证

根据改动范围执行；对上游同步 PR 和发布 PR，应执行完整检查：

```bash
pnpm typecheck
pnpm format:check
pnpm test:unit
pnpm build:renderer
cd src-tauri
cargo fmt --all -- --check
cargo clippy -- -D warnings
CC_SWITCH_TEST_HOME="$(mktemp -d)" cargo test
```

- 在 macOS/Windows 目标平台运行与变更相关的实际功能/安装/更新回归。
- 业务逻辑新增单测；配置写入/迁移涉及破坏性风险时增加恢复测试。
- 如由于环境限制不能运行检查，明确列出**未运行项与原因**，不得宣称测试通过。

## 7. 交付时必须按此模板汇报

```text
【需求】
【上游复用点】
【新增自有文件】
【修改上游文件（没有则写无）】
【侵入等级】L0/L1/L2/L3/L4
【数据/安全/跨平台/兼容风险】
【测试结果（含未运行项）】
【补丁登记及文档变化】
【后续未完成事项】
```

## 8. 修改本规则

不允许因一次任务便利直接删改本规范红线。若必须例外，先提出 ADR / 规范修订方案与原因。
