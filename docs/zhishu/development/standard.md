# Zhishu CCS 二次开发实施标准

> **规范编号**：ZCCS-DEV-001  
> **版本**：v1.0  
> **生效日期**：2026-10-29  
> **适用项目**：Zhishu CCS（基于 CC Switch 的 Fork / 二次开发）  
> **规范级别**：项目级强制执行  
> **核心目标**：在实现智枢独立产品需求的同时，维持可预测、可测试、可持续的上游合并能力。  
> **使用方式**：本文位于 `docs/zhishu/development/standard.md`；与仓库根目录 `AGENTS.md` 一并执行，现有规则须合并保留。
> **代码核对（2026-10-31）**：目录、remote、数据与能力现状见 [当前基线](baseline.md)。本文的目标架构与模板不等于功能已实现；核心红线保持不变。平台业务按 ADR-0004/0005 执行下述已生效修订。

---

## 0. 执行摘要与红线

### 0.1 五项基本原则

1. **上游可合并**：CC Switch 是可持续同步的产品底座，而非一次性复制的源码。
2. **业务可隔离**：智枢功能优先新增于独立命名空间，不直接混入上游业务实现。
3. **改动可追踪**：任何触碰上游核心的修改均有原因、边界、测试和升级记录。
4. **数据可迁移**：用户数据、CLI 配置、安全凭据和应用身份都可备份、恢复、迁移。
5. **发布可回滚**：macOS/Windows 签名更新包、发布版本与兼容性验证可追溯。

### 0.2 强制红线（MUST NOT）

- 不得为了方便，直接在上游 `services/`、`proxy/`、`database/`、`src/App.tsx` 中散落智枢业务逻辑。
- 不得复制整套 Provider / MCP / Skills / CLI 配置系统用于平行维护。
- 不得绕过 CC Switch 的受控配置写入服务，直接无保护覆写 `~/.codex`、`~/.claude` 等工具配置。
- 不得在 Git 历史中提交 API Key、登录 Token、签名私钥、真实用户配置或数据库。
- 不得使用原版 CC Switch 的产品身份、更新频道或签名发布智枢正式版本；任何变更都必须先评估存量数据迁移和共存影响。
- 不得在存在未处理冲突、核心回归失败或数据库不可恢复风险时发布。
- 不得为合并上游长期重写或强推 (`push --force`) 已发布的 `main` 历史。
- 不得将“能成功 Git merge”当成“升级兼容”。必须验证行为、数据和发布链路。

### 0.3 规范用语

- **MUST / 必须**：合并发布必备条件。
- **SHOULD / 应**：默认执行，例外需要解释。
- **MAY / 可以**：按场景选择。
- **上游补丁**：修改原 CC Switch 已有文件（而非新增智枢自有文件）的差异。
- **兼容层 / Adapter**：封装上游接口变化，为智枢业务提供稳定接口。

---

## 1. 项目架构与边界

### 1.1 已核实的上游背景

CC Switch 使用 React + TypeScript 前端、Tauri 2 / Rust 后端、SQLite 持久化；关键调用链为：

```text
React UI -> Hooks / TanStack Query -> src/lib/api -> Tauri invoke
                                                 |
                               Commands -> Services -> DAO -> SQLite
                                         \-> 受控本地配置文件写入
                                         \-> 本地代理 / 路由 / 格式转换
```

上游的主要能力包括供应商管理、Provider 切换、MCP、Skills、会话查看、CLI 配置写入和本地路由。上述是对上游 `main` 在制定规范时的核验结果，后续升级必须重新以实际仓库为准。

上游参考：
- https://github.com/farion1231/cc-switch
- https://github.com/farion1231/cc-switch/blob/main/CONTRIBUTING.md
- https://github.com/farion1231/cc-switch/blob/main/package.json

### 1.2 分层职责

```text
┌──────────────────────────────────────────────────────────┐
│ Zhishu UI / 登录 / 新手模式 / 模型选择 / 平台账号中心       │
├──────────────────────────────────────────────────────────┤
│ Zhishu Application Services                              │
│ 用户流程编排、幂等、一键配置、错误处理、状态机             │
├───────────────────────┬──────────────────────────────────┤
│ Zhishu Proxy API       │ CC Switch Adapter               │
│ 登录/模型/Key/分组     │ Provider/安装/配置/路由能力适配  │
├───────────────────────┴──────────────────────────────────┤
│ 原 CC Switch 内核（尽量不改）                            │
│ 前端公共组件、Tauri IPC、Rust Services、DAO、代理等        │
└──────────────────────────────────────────────────────────┘
```

**依赖方向**：智枢功能依赖 Adapter；Adapter 调用上游公共服务。不得让上游通用核心反向依赖具体智枢平台服务。极少数启动注册处允许薄接入，但必须登记。

### 1.3 自有业务与上游责任划分

| 能力 | 智枢负责 | 上游负责 / 优先复用 |
|---|---|---|
| 用户登录 | 智枢账户、Token 刷新、授权态 | Tauri 安全存储接口（若适配） |
| 模型列表 | Zhishu Proxy API、模型可用性和分组约束 | 本地供应商/模型配置结构 |
| 新手模式 | 流程编排、幂等 Key、协议校验、恢复 | Provider CRUD、工具配置写入与切换 |
| Codex / Claude Code 等安装 | 智枢引导、安装策略 | 已有安装/检测能力（先调查） |
| MCP / Skills | 智枢推荐目录、预设 | 已有安装/同步框架 |
| 模型协议与路由 | 平台兼容性判定及产品策略 | 上游已有代理/转换机制（先验证） |
| 品牌首页 | 独立视图、导航和 UI 设计 | 通用 UI 组件，可选复用 |
| 更新发布 | 自有签名、发行渠道、COS/CDN | Tauri updater 机制 |

---

## 2. 代码结构与侵入边界

### 2.1 目标组织方式（建议增量创建，不要求一次搬迁所有旧代码）

```text
zhishu-ccs/
├── src/                              # CC Switch 原前端
│   ├── components/                   # 保留原业务组件
│   ├── hooks/                        # 保留原业务 hooks
│   ├── lib/api/                      # 保留原 Tauri API 桥接
│   └── zhishu/                       # 新增：智枢前端命名空间
│       ├── api/                      # Zhishu Proxy 客户端
│       ├── auth/                     # 登录、授权态
│       ├── onboarding/               # 新手引导
│       ├── models/                   # 模型展示及筛选
│       ├── provisioning/             # 一键配置任务 UI / 编排
│       ├── integrations/ccswitch/    # 上游 TS 适配器
│       ├── components/               # 智枢自有组件
│       └── app/                      # 导航、视图装配
├── src-tauri/src/
│   ├── lib.rs                        # 上游 Tauri 注册入口，尽量薄改
│   ├── commands/                     # 上游 Commands
│   ├── services/                     # 上游 Services
│   ├── database/                     # 上游数据库/迁移
│   ├── proxy/                        # 上游代理核心
│   └── zhishu/                       # 新增：智枢 Rust 命名空间
│       ├── mod.rs
│       ├── commands/
│       ├── services/
│       ├── integrations/             # 上游 Rust 服务适配
│       └── storage/                  # 智枢存储与迁移
├── tests/zhishu/                     # 前端测试
├── docs/zhishu/                      # 开发规范/ADR/升级报告
├── scripts/zhishu/                   # 自有脚本
└── .github/                          # CI/CD，保留或调整上游工作流
```

注意：这是**目标目录设计**，不是当前分支现有目录的断言；先核对本地代码再动手。

### 2.2 改动分级

| 等级 | 适用改动 | 处理要求 |
|---|---|---|
| L0 | 新增 `zhishu/` 自有模块、文档、测试 | 默认允许；测试覆盖 |
| L1 | 品牌资源、导航/路由挂载、Tauri Command 注册 | 限定为入口薄改，记录文件 |
| L2 | 原有上游 UI 页面少量插槽/扩展接口 | 先证明不可包装；登记补丁并测试 |
| L3 | 直接修改 Rust 核心 Services、DAO、代理/迁移机制 | 必须有 ADR、备选方案、受影响测试和回退计划 |
| L4 | 复制并替代整套上游核心、无迁移方案的数据库破坏、重写通用配置体系 | 默认禁止；需单独架构决策批准 |

**最小补丁原则**：能增加独立文件就不改原文件；能包一层 Adapter 就不改内核；能集中改一个注册处就不改多个业务模块。不要为了“改动行数少”牺牲可靠性、可读性或安全性。

### 2.3 Adapter 接口示例（仅是建议契约，不代表上游存在同名 API）

```ts
export type ManagedTool = 'codex' | 'claude-code' | 'opencode';

export interface ProviderConfigRequest {
  tool: ManagedTool;
  providerName: string;
  baseUrl: string;
  apiKeyRef: string;  // 安全存储中的引用，非明文密钥
  modelId: string;
  protocol: string;
}

export interface CcSwitchAdapter {
  listInstalledTools(): Promise<ManagedTool[]>;
  upsertProvider(request: ProviderConfigRequest): Promise<string>;
  activateProvider(tool: ManagedTool, providerId: string): Promise<void>;
  backupLiveConfig(tool: ManagedTool): Promise<string>;
  restoreLiveConfig(tool: ManagedTool, backupId: string): Promise<void>;
}
```

Adapter 的具体调用需由当期上游源码确认。明确区分 TS 前端编排与 Rust 后端执行；敏感密钥不要在 React 状态、日志中长期持有。

### 2.4 UI 改造标准

- **第一阶段**：登录、首页、新手模式和智枢服务页独立开发；现有 Provider/MCP/Skills/Settings 等原页面先复用。
- **第二阶段**：以包装组件、统一设计 token、局部页面替换完成改版；优先不碰上游状态/存储逻辑。
- **第三阶段**：只有维护成本数据明确证明有必要时，才考虑针对单个模块重新实现；不得无规划整体重写前端。
- 主题、国际化、响应式、Windows/macOS 差异和无障碍应纳入验收。

---

## 3. Zhishu Proxy 集成与平台同步

> 本节按 [ADR-0004](../adr/ADR-0004-client-api-only.md)、[ADR-0005](../adr/ADR-0005-account-local-cleanup.md) 的既有业务决策归并，替代旧版“客户端确保 Key／单模型接入”示例，没有新增规范例外。

### 3.1 当前用户路径

打开客户端 → 登录或验证恢复会话 → `/access` 更新当前账号本机基础数据 → 从本机目录同步支持工具的供应商。登录/恢复后自动尝试一次两个阶段，失败可手动重试；账户轮询不重复自动同步。

手动“同步平台基础数据”只联网保存目录；手动“同步服务提供商列表”只读本机快照，通过 Adapter 调用上游 ProviderService。两个阶段必须保持独立，不能隐式联网补齐本地同步。

### 3.2 强制业务约束

1. 授权分组、模型与 Key 生命周期由服务端决定；客户端只调用专用 API，不推断前缀授权、不创建/替换/删除共享 Key，不回退旧接口。
2. 工具协议能力由当前 Adapter 校验；平台未报告的能力保持未知，工具本地默认值不得写回为服务端声明。
3. 供应商和原生配置继续复用受控写入、应用锁、原子发布与既有恢复。平台同步不新增同步前检测、确认或全量备份；通用首写备份、pending 恢复和 WorkBuddy 局部恢复规则仍保留。
4. 同步调和当前账号托管项，保护自建项、其他账号和失败分组；当前项无安全备用供应商时保留并报告。
5. 普通退出只清登录会话；显式清理退出按 ADR-0005 限定账号范围并报告部分失败，不能撤销服务端共享模型 Key。
6. 错误和日志脱敏；报告区分 Provider 数据库行变化与实际文件变化，不以同步成功代替真实 CLI 运行验收。

### 3.3 后续新手模式

其他章节提及的新手引导、工具安装和配置任务状态机仍为扩展方向，不能由本文推断已有对应页面或独立 IPC。实施前先核对现有能力，再按本标准和新的 ADR 评审；不得恢复已废弃的客户端建 Key 链路。

---

## 4. 数据、凭据、应用身份与系统配置

### 4.1 持久化策略

当前智枢使用 `~/.zhishu-ccs` 及账号独立目录，见 [当前基线](baseline.md)。下述 `~/.cc-switch` 仅描述上游来源，不能用于定位本产品数据。

- 上游目前以 `~/.cc-switch/cc-switch.db` 和 `~/.cc-switch/settings.json` 等路径作为持久化来源。**不能在不了解路径引用与迁移机制时随意改目录。**
- 智枢自有业务数据优先使用独立 `zhishu_*` 表或独立智枢存储。若使用同一个 SQLite，需独立迁移命名空间、唯一迁移版本号/登记机制、备份测试，并尊重上游的升级方式。
- 对正式独立产品，**推荐设计独立的 Zhishu CCS 应用数据目录**，但需要先完成路径清单、备份/导入、首次升级迁移、原版共存测试后再启用。不要只改一个路径字符串。
- **重要边界**：独立应用数据库不意味着独立 `~/.codex` / `~/.claude` 等外部工具配置；这些文件可能被原版 CC Switch、其他配置工具和用户手工编辑，共享写入必须具备备份、检测冲突和明确的归属策略。

### 4.2 安全与机密

当前 refresh token 和记住密码使用 keyring；平台分组 Key 与普通 Provider/CLI Key 仍存在明文副本，与本节安全目标有差异。私密权限不等于加密；本次文档整理不执行凭据迁移，后续须经独立 ADR 评审。

- 生产 Token / 用户 API Key 使用经过评审的系统安全凭据存储（例如 OS Keychain/Credential Manager 或经过审计的加密方案）；不要以明文保存在普通 localStorage、SQLite 或日志。
- 不在前端包/仓库内固定任何平台主 API Key、签名私钥、内部管理接口凭据。
- 网络通信默认 HTTPS；证书校验不可随意关闭；避免在日志中输出 `Authorization`、完整连接串或用户隐私配置。
- Rust IPC Commands 严格进行参数校验，按需限制文件路径、可执行程序和下载来源；不要引入可执行任意命令的宽权限接口。
- 安装 Codex、Claude Code、其他工具优先复用可信的上游安装/校验流程；必要时校验来源、版本、哈希/签名；禁止默认执行不可信远程脚本。

### 4.3 身份和共存

上线前必须独立评审：Tauri `identifier`、`productName`、窗口/托盘行为、图标、深链、单实例锁、进程名、安装路径、应用数据路径、安装器升级行为。先验证与原版 CC Switch 是否可以并存，以及从 Zhishu CCS 已发布旧版本如何迁移。

### 4.4 数据变更变更单

任何新增表/字段/配置项需填写：

- 数据归属（智枢或上游）与读写方；
- 兼容的旧版本/降级策略；
- 前向迁移、失败恢复、备份/还原测试；
- 是否影响与原版共存、同步或导入导出。

---

## 5. Git 策略与上游同步标准

### 5.1 仓库和分支

以下为目标命名与示例。当前 `origin` 是官方、`zhishu-git` 是自有 GitHub、`zhishu-origin` 是自有 GitLab，且没有 `upstream`；先核对 [真实远程](baseline.md)，不得照抄示例改 remote 或推送。

- `origin`：公司自己的 Zhishu CCS 仓库；主分支 `main` 是受保护、可发布的智枢代码。
- `upstream`：官方 CC Switch 仓库，保持只读，不在其上直接开发。
- `feat/<scope>-<description>`：正常功能分支。
- `fix/<scope>-<description>`：缺陷修复。
- `sync/upstream-<version-or-date>`：一次上游同步专用分支。
- `release/<version>`：可选稳定发布分支，用于集中发布验证。

**默认策略**：长期同步使用 `merge upstream/main`，保留祖先关系；不要对共享/已发布的 `main` 进行反复 rebase。若选择同步上游稳定 tag，则在合并记录中注明 tag 与 SHA。

### 5.2 首次建立 upstream

```bash
# 确认 origin 和当前工作树，先提交或暂存自己的改动
git status --short
git remote -v

# 仅当 upstream 尚不存在时执行
git remote add upstream https://github.com/farion1231/cc-switch.git

git fetch upstream --tags
git config rerere.enabled true

# 检查与上游的提交差异
git rev-list --left-right --count upstream/main...main
git log --oneline upstream/main..main
```

**如果现有代码是无历史关联的源码复制**，不可直接假设 `git merge` 有公共祖先。先确认 `git merge-base upstream/main main`；无公共祖先应制定“恢复 Fork 历史/迁移差异”专项方案，禁止直接 `--allow-unrelated-histories` 硬拼。

### 5.3 每次上游同步 SOP

```bash
# 0. 确认本地修改已提交，且 main 受保护
git status --short

git fetch origin
git fetch upstream --tags

git switch main
git pull --ff-only origin main

git switch -c sync/upstream-20261029

git merge --no-ff upstream/main
# 有冲突：逐文件审查、解决、运行测试，再完成 merge commit
# 无冲突：仍须运行全部回归、升级/安装与兼容测试

# 上传同步分支并发起 PR
git push -u origin sync/upstream-20261029
```

如果团队选择固定 upstream tag，以上命令里的 `upstream/main` 应替换为经评估的具体 tag/SHA，并记录理由；同步后 PR 需记录：原上游 SHA、目标 SHA、冲突列表、补丁变化、测试结果、回滚方式。重要：**遇到上游 API / 数据模型变化，不允许只因为构建成功就合并。**

### 5.4 上游补丁登记表

必须维护 `docs/zhishu/development/upstream-patches.md`，每条至少包含：

| ID | 上游文件 | 原因 | 替代方案为何不可行 | 风险级别 | 对应测试 | 最后复核 |
|---|---|---|---|---|---|---|
| PATCH-001 | `src/App.tsx` | 智枢页面挂载 | 上游入口无插件挂载能力 | L1 | 新手模式 E2E | 待填 |

以上示例仅用于示意，只有经本地 diff 确认的真实补丁才能写入正式登记表。

上游已吸收的同类能力，要主动删除智枢重复实现或旧补丁，不能一直叠加历史兼容层。

### 5.5 冲突处理准则

1. **先理解上游改动意图**，再判断自有功能有无必要保留。
2. 不以 `-X ours` / `-X theirs` 全局批量掩盖冲突；可以对确定的单文件处理并说明原因。
3. 冲突解决后进行功能与数据回归。
4. `git rerere` 只负责记录冲突决议的复用，不能代替重新检查。
5. 每次同步建立文档 `docs/zhishu/upstream-sync/YYYY-MM-DD.md`，留存影响面和结果。

### 5.6 上游同步频率

- **每周**：检查 Release、安全公告、新提交及潜在影响，先做影响分析，不一定立刻合并。
- **每月或每个稳定上游版本**：在同步分支评估合并；变更较大可分批验证。
- **重大安全修复**：优先快速评估和吸收，不机械等待月度周期。
- **每个 Zhishu CCS 发布前**：记录上游基线 SHA、尚未吸收的关键变更及风险。

---

## 6. 需求开发 SOP（Codex / 人工统一遵循）

### 6.1 开始前：先调查，不要直接编码

每个需求先产生一份简短实施说明，至少回答：

1. 要解决什么用户场景、验收标准是什么？
2. 上游是否已有相同/相近能力？对应实现入口与代码位置？
3. 方案 A（纯新增/复用）是否可行？若不可行，原因是什么？
4. 计划新增哪些智枢文件、修改哪些上游文件？按 L0–L4 分级。
5. 是否影响本地配置、数据库、鉴权、自动更新或 Windows/macOS？
6. 本次需增加哪些单测、回归、手工验证？

### 6.2 实施期间

- 先把业务逻辑写入 `zhishu/`，然后按需做入口适配。
- 不为一个智枢功能顺手重构无关上游文件。
- 每次新增上游补丁都要说明“为何无法仅靠扩展实现”。
- 数据更新、配置写入、安全凭据和用户已存在配置必须设计回滚。
- 每个阶段都可构建、可测试；大功能按可 review 的小 PR 拆分。
- 引入新依赖需说明许可、安全与上游重叠关系。

### 6.3 PR 模板（所有功能合并前必填）

```markdown
## 目标与验收
- 需求：
- 验收：

## 上游复用与影响
- 复用的上游代码/能力：
- 新增智枢文件：
- 修改上游文件（没有则写“无”）：
- 侵入等级（L0/L1/L2/L3/L4）：
- 不能通过 Adapter 解决的原因（适用时）：

## 风险与兼容
- 数据库/配置/凭据：
- macOS/Windows 差异：
- 与原版 CC Switch 共存：
- 回滚方式：

## 测试
- [ ] pnpm typecheck
- [ ] pnpm format:check
- [ ] pnpm test:unit
- [ ] pnpm build:renderer
- [ ] cargo fmt --all -- --check
- [ ] cargo clippy -- -D warnings
- [ ] cargo test（隔离测试目录）
- [ ] 相关业务场景和配置回归

## 上游同步影响
- [ ] 已更新 docs/zhishu/development/upstream-patches.md（如果触碰上游文件）
- [ ] 已确认合并上游能力未被不必要破坏
```

---

## 7. 自动化检查、质量门禁与回归

### 7.1 基础静态检查

以仓库当前 `package.json` 和 Rust 工具链为准。当前上游参考：

```bash
pnpm install --frozen-lockfile
pnpm typecheck
pnpm format:check
pnpm test:unit
pnpm build:renderer

# Rust backend（建议隔离测试真实工具配置，尤其在本机运行时）
cd src-tauri
cargo fmt --all -- --check
cargo clippy -- -D warnings
CC_SWITCH_TEST_HOME="$(mktemp -d)" cargo test
```

注意：在某些干净环境，Rust 构建前须先生成 `dist/`（例如执行 `pnpm build:renderer`）。具体系统依赖和 CI 矩阵应参照上游 CONTRIBUTING 与本 Fork 工作流。

### 7.2 必测场景（按改动面选择；相关模块改动必测）

| 范围 | 核心验收 |
|---|---|
| 登录 | 首次登录、Token 过期/刷新、退出登录、无网络、重启恢复 |
| 平台同步/后续新手模式 | 授权模型与协议兼容、不重复创建 Provider、两阶段边界与失败恢复 |
| 工具配置 | Codex/Claude Code/OpenCode 配置写入、配置保留、原子替换、备份回滚 |
| 上游 Provider | 新建、修改、删除、切换、回填、排序、原有模式无回归 |
| MCP / Skills | 安装、移除、同步、路径冲突及权限检查 |
| 本地路由 | 启停、目标转发、代理环境、错误恢复、协议映射、计费统计（适用时） |
| 数据升级 | 旧版本升级、新版本启动、配置备份恢复、损坏迁移处理 |
| 共存 | 与原版 CC Switch 同机安装、应用数据隔离、外部 CLI 配置冲突防护 |
| UI | Windows/macOS 显示、布局、中文/英文、安装和更新后状态 |
| 发布 | 签名验证、下载/更新、失败回滚、更新元数据及 CDN 缓存策略 |

### 7.3 CI 最低门槛

- 在每个 PR 上执行前端类型/格式/测试/构建与 Rust 格式/静态检查/测试。
- Rust / Tauri 集成测试安排 Linux/macOS/Windows 相关矩阵；资源紧张可拆为快速 PR 阶段 + 发布前完整矩阵，但**不得未经目标平台验证就发该平台正式包**。
- 上游同步 PR 必须通过升级回归清单；发布候选版必须实际检查 macOS 和 Windows 安装/更新。
- 保护 `main`：禁止直接 push；至少完成必须检查与代码审查。
- GitHub Actions 权限最小化，发布任务单独授权；签名凭据存入 Actions Secrets（或等效安全服务），不出现在普通 PR 工作流环境。

---

## 8. 版本、构建、签名、COS/CDN 发布

### 8.1 独立版本控制

- Zhishu CCS 使用**独立产品版本号**（例如 `4.0.21-zhishu.1` 仅为示意；实际需校验 Tauri/平台打包器版本格式约束并采用有效版本）。
- 同步上游 ≠ 自动发布；上游版本号和 SHA 作为构建元数据登记。
- 统一维护 package.json、Tauri 版本配置及其他实际涉及的版本位置，避免客户端更新版本比较异常。

### 8.2 发版最低步骤

1. 合并通过全部门禁的功能与同步 PR；锁定 commit + 上游基线 SHA。
2. 由 GitHub Actions 或受控 CI 构建 macOS / Windows 对应架构安装包；不要依赖个人电脑最终签名打包。
3. 按 Tauri updater 机制使用智枢自己的签名密钥；公钥客户端内置，私钥留在 CI 安全存储。
4. 产物存腾讯云 COS，CDN 加速；采用不可变版本路径；发布清单与更新元数据受控生成，最后再切换 latest 指针。
5. 校验更新清单的版本、URL、签名、平台适配与 HTTPS 可用性；不要使用不可控、过期的临时下载链接当作固定更新源。
6. 先内部/灰度发布、测试下载与自动更新，再全量；失败可将更新元数据回滚至上一个**允许且可安全降级**的版本。注意用户数据迁移一旦不可逆，回滚客户端并不保证可回滚数据。
7. 保存 SHA256、构建环境、变更日志、上游同步记录和签名元数据；对旧安装包保留必要的安全下载途径。

**发布口径**：生产发布不允许混用原版 CC Switch 的 updater、签名、应用身份。对腾讯云 COS 私有回源或公共下载的具体访问策略，按 CDN 鉴权架构单独评审；不要把“桶公有读/私有写”视为唯一正确方案。

---

## 9. 接手已有二开代码的迁移计划

> **不推倒重来**，先固化当前可用功能，再逐步把现有登录和新手模式迁到标准架构。

以下是规范制定时的迁移路线模板；完成状态以代码和记录为准，未勾选不表示当前分支尚未实现。旧平台设计以第 3 节为准。

### P0 — 建立基线（先执行）

- [ ] 识别 `origin`、`upstream`、实际 merge-base 和当前上游版本。
- [ ] 列出新增文件、修改过的上游文件、潜在机密提交。
- [ ] 记录当前登录、模型使用、工具配置、数据目录和更新链路。
- [ ] 建立 `docs/zhishu/development/upstream-patches.md` 与首次 `docs/zhishu/development/baseline.md`。
- [ ] 跑通当前前后端检查和可用安装包构建；失败项登记为基线缺陷。

推荐调查命令（根据实际当前分支、仓库历史调整）：

```bash
git remote -v
git status --short
git merge-base upstream/main HEAD
git log --oneline --max-count=12
BASE="$(git merge-base upstream/main HEAD)"
git diff --stat "$BASE" HEAD
git diff --name-status "$BASE" HEAD
```

### P1 — 独立模块化

- [ ] 新建 `src/zhishu/`、`src-tauri/src/zhishu/`、`docs/zhishu/`。
- [ ] 将登录/平台 API/模型选择按**小 PR**渐进迁移，不同时重写 UI 与逻辑。
- [ ] 在 Adapter 中封装 Provider 创建、切换和工具配置写入。
- [ ] 收敛 App/Command 注册入口上游补丁并登记。

### P2 — 可靠的一键使用模型

- [ ] 服务器驱动可用模型与分组；客户端通过 `/access` 获取服务端管理的受限 Key。
- [ ] 协议兼容校验、备份与回滚、错误码、必要时取消/重试。
- [ ] 覆盖 Codex、Claude Code 等当前支持工具的完整回归场景。
- [ ] 验证手工配置、原版 CC Switch 与智枢共存行为。

### P3 — 上游同步与发布闭环

- [ ] 演练 `sync/upstream-*` 分支合并，解决冲突并形成升级报告。
- [ ] 补齐保护主分支/CI/发布矩阵。
- [ ] 独立应用身份、路径、安全密钥与更新渠道。
- [ ] GitHub Actions → 签名 → COS/CDN → 灰度更新 → 回滚演练。

---

## 10. 决策、例外与版本治理

### 10.1 ADR 标准

L3/L4 或可能影响持久化数据和更新行为的方案，必须创建 `docs/zhishu/adr/ADR-XXXX-<topic>.md`，内容包括：背景、备选方案、做出的选择、上游冲突评估、兼容/安全影响、测试、回退、复审日期。

### 10.2 例外处理

由于紧急缺陷临时突破规范时，必须登记例外编号、影响范围、责任人、修复/偿还条件和跟踪 Issue。例外不得用于跳过凭据保护、用户数据安全或发布签名验证。

### 10.3 每个功能最终交付格式

```text
【需求】
【上游复用点与代码位置】
【新增智枢模块/文件】
【修改上游文件及其理由】
【侵入级别】L0/L1/L2/L3/L4
【兼容/迁移/安全风险】
【测试执行与结果】
【是否更新补丁登记】
【剩余风险及后续事项】
```

### 10.4 维护本规范

本文 v1.0 为 Zhishu CCS 的默认工程标准。修改核心红线、目录层级、数据库隔离或同步策略，应通过 ADR 和独立规范 PR 完成，不因单个 Agent 的临时偏好自动改变。

---

## 附录 A：首次落地的最小文件集合

```text
AGENTS.md

docs/zhishu/
  README.md
  development/
    standard.md
    baseline.md
    upstream-patches.md
  features/
  api/
  releases/
  history/
  upstream-sync/
  adr/
```

## 附录 B：首个基线文档模板

```markdown
# Zhishu CCS Fork 基线
- 评估日期：
- origin URL / HEAD SHA：
- upstream URL / HEAD SHA：
- merge-base SHA：
- 当前 Zhishu CCS 版本：
- 产品身份 / 数据路径：
- 修改的上游文件清单：
- 当前独立业务模块清单：
- 已知 CI/构建缺陷：
- 可能影响上游同步的风险：
- 第一次上游合并演练结论：
```

## 附录 C：当前上游核验说明

本文件中的 CC Switch 架构、开发命令、测试目录与存储路径参考 2026-10-29 可检索到的官方 `CONTRIBUTING.md` 和 `package.json`。**所有具体命令与接口名称以你实际 Fork 中锁定的 commit 为准**；禁止把范例接口误认为上游真实存在的 API。
