# ADR-0014：合并上游 v4.0.29

- 日期：2026-11-11
- 状态：已实施，本机自动验证通过；不构成发布批准
- 等级：L3（Copilot 原生配置、Provider/Skills/MCP、schema 迁移和代理）
- 产品起点：`1c86ce632e3a5eafec5296322d3437b9007bfbe7`，0.0.10
- 上游基线：v4.0.28 / `8656ebd91f3eba631837126e29df71e8353b7e43`
- 官方目标：v4.0.29；tag 对象 `8ad4317a8e949b95457e7441ff1bc8421e0b584a`，提交 `efbec38f0e1ecaee2a488d619190b7ec4075238b`；已 fetch 并通过 `git ls-remote upstream` 核验。

## 场景、验收与选择

复用上游新增的 Copilot CLI 原生 BYOK/官方卡、MCP、Skills、提示词、会话与用量能力，吸收 Codex WSL 目录指针、官方模型透传、Responses 工具结果和输出上限修复，接入韩语/俄语。同时保留智枢产品身份、简易模式、平台 OAuth2、两阶段同步、WorkBuddy、配置历史与 ADR-0009 共存保护。验收要求双方祖先保留，冲突按行为合并，自动回归通过；真实平台缺少验收时明确记录。

选择从当前产品提交在 `sync/upstream-4.0.29` 三方合并，然后快进本地 `ccs-dev`。整文件覆盖会丢失智枢扩展；挑选提交会丢失完整祖先并遗漏 Copilot 跨模块依赖。Provider 分发、原子写入、Skills 目标保护、迁移和代理均在内部固定入口，单纯新增 Adapter 无法替换这些内部调用；继续复用上游服务，不复制平行核心。

文件范围包括上游 Copilot 新模块及对应前后端组件/测试，现有 app_config、settings、Provider、MCP、Skills、schema、代理（L3），应用枚举、入口与语言包（L1/L2），清单和产品身份（L1）；新增本 ADR、同步报告并更新补丁/基线/验证索引（L0）。精确范围以本次合并相对第一父 diff 为准。

## 数据、安全与兼容

主数据库由 schema 24 升至 25，仅为 MCP/Skills 增加独立 `enabled_copilot` 开关，默认关闭；保留智枢 v22/v23 幂等协调、事务、失败回滚与旧标志。新设置使用既有序列化。Copilot 配置沿用上游路径解析、整份验证与局部条目修改，保留其他条目和顺序；本轮不操作真实配置。

Copilot 是上游管理应用，不因出现在列表中自动加入智枢平台白名单或简易模式接入页。平台层契约与账号归属不变，不新增平台 API 或 Key 生命周期操作。WorkBuddy 分支保留；配置写入仍留审计；Skills 跨产品目标保护继续应用全部受管应用。Codex 派生文件保留智枢文件名与归属规则。

保留产品 0.0.10、独立数据目录、更新源、深链、OAuth2/keyring 边界。新增 ko/ru 字典须兼容智枢自有文案，不能直接宣称上游七语测试即证明平台翻译完整。保持已有未跟踪 `.zcode/` 的内容与状态。不改 remote、不重写历史、不处理 `origin/ccs-dev` 缓存的既有分歧（起点领先 164、落后 26），不推送或发布。

## 验证与回退

执行 frozen install、TypeScript、Prettier、前端全量/renderer；Rust fmt/check/Clippy 与临时 `CC_SWITCH_TEST_HOME` 下全量测试，重点覆盖 schema 旧分支升级/幂等/失败、Copilot 配置和 Provider、MCP/Skills、Codex 金标与代理、平台/简易模式。工具变量使用既有隔离夹具，不更改智能体 HOME/CODEX_HOME；本轮不启动真实桌面应用、CLI、OAuth2 或系统 keyring。

Windows/Linux 编译与 CLI、WSL、真实凭据/安装升级未验证时须记录，不能用本机测试替代。验证结果、冲突与限制见 [同步报告](../upstream-sync/2026-11-11.md)。代码可通过 `git revert -m 1 <本次合并提交>` 回退至第一父行为，不重写共享历史；若以后真实运行已将数据库升至 25，必须先备份并恢复兼容数据，不能将 Git 回退当作数据库降级。旧二进制对 v25 的行为另按 schema 版本检查评估。

本轮结论：15 个冲突已按双方行为处理；新增 schema 迁移、旧分支协调及失败回滚回归均通过。ko/ru 各补齐 314 条智枢翻译，ja/zh-TW/tr 各补齐 8 条登录标签，七语一致性通过。前端 319 文件 / 4082 项、Rust 5063 项通过 / 15 忽略，类型/格式、Clippy、renderer、Node 57 项与 Python 21 项模拟通过（Python 另 2 跳过）。首轮新增 Codex 测试写死官方目录名，已按智枢常量适配并全量复测通过。平台自有源码与白名单不变；没有真实数据迁移或发布。
