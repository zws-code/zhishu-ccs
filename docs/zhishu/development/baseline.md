# Zhishu CCS 当前代码基线与限制

> Git、版本与远程核对：2026-11-24（分支 `reorg/upstream-4.0.40`）；上游 v4.0.40 tag 使用本地已获取的对象，与主线 `ccs-dev` 合入的是同一提交；未重新核验生产发布状态。其余模块说明沿用各次任务的核对范围，具体实现以当前代码为准。

## 版本、上游与远程

| 项目 | 当前本地事实 | 依据 |
| --- | --- | --- |
| 产品版本 | `0.0.11` | `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` |
| 产品分支 / 同步起点 | 本文描述 `reorg/upstream-4.0.40`：`reorg/upstream-4.0.33`（v4.0.33 上的 8 个线性提交）→ 合并 v4.0.40 的 merge 提交 → `fix(workbuddy)` → 文档提交。`ccs-dev` 未改动，仍保留二开 WorkBuddy 适配；两条线代码差异只在 WorkBuddy | `git log`、[提交整理记录](../history/commit-reorganization.md) |
| 已合入上游 | `v4.0.40` / 提交 `f19a9d4f4f08bda03106a7be87c77cbcdf55b019`（tag 对象 `3da554daf7d5db5feb530f975181371b3ce41133`） | merge 提交第二父即该 tag；见 [同步报告](../upstream-sync/2026-11-24-reorg.md) 与 [ADR-0018](../adr/ADR-0018-merge-4.0.40-upstream-workbuddy-coexistence.md) |
| 官方 remote | `upstream` → `git@github.com:farion1231/cc-switch.git` | `git remote -v` |
| 自有 GitHub | `origin` → `git@github.com:zws-code/zhishu-ccs.git` | 同上；发布 tag 使用此 remote |
| 自有 GitLab | `gitlab` → `http://172.16.32.4:8929/hunanzs/zhishu-ccs.git` | 同上 |
| Rust / pnpm | Rust `1.95`；pnpm `10.12.3` | `rust-toolchain.toml`、`package.json` |

2026-11-03 处于开发测试阶段，按用户要求在本地把 `ccs-dev` 自有提交线性重建：以 `a3eafa408ae679db2e3401fca767ed2b89d9a2b2`（v4.0.25）为第一父，28 条自有提交合并为 10 条功能提交，最终树与原 `a234bc66` 完全一致（tree `1b8d2021de963426949d5ed5e17123996ac2f384`）。`v0.0.4`–`v0.0.7` tag 保持原指向、未移动，因此不再位于重建后的 `ccs-dev` 祖先链上；整理前历史保留在 `backup/ccs-dev-before-reorg-20261103`，远程 `origin/ccs-dev` 当时仍为 `c5d3f2c9`，远程同步另按确认执行。详见[提交整理记录](../history/commit-reorganization.md)。

2026-11-03 按用户授权执行 `git remote rename`：旧 `origin` → `upstream`、旧 `zhishu-git` → `origin`、旧 `zhishu-origin` → `gitlab`。fetch/push 地址不变，远程跟踪引用随改名迁移；本地分支、tag 和 HEAD 保持不变。本任务未 fetch、pull、push 或修改服务器配置。

重命名任务当时的初检保留原目标：`main` → `upstream/main`，`ccs-dev` → `gitlab/ccw-dev`。当时最终复核发现并行操作将 `ccs-dev` 改为跟踪 `origin/ccs-dev`，并将该远程缓存引用从 `59b437af` 更新至 `c5d3f2c9`；该历史结果不代表此后的远端同步状态。2026-11-03 本轮同步开始时，本地产品 HEAD 已为 `1da9fa8b`，相对 `origin/ccs-dev` 缓存领先 4、落后 1；本轮未获取自有远程或处理既有分歧。改名和上游同步都不解决 GitLab 分歧。发布脚本只接受指向 `zws-code/zhishu-ccs` 的 `origin` 唯一 push URL，仍拒绝官方地址和多个 push URL。

其他克隆应先核对旧配置再按上述顺序迁移，脚本与文档须一起更新。回退顺序：`git remote rename origin zhishu-git` → `git remote rename upstream origin` → `git remote rename gitlab zhishu-origin`，并恢复本轮发布脚本/当前操作文档；无需改动提交历史或真实用户数据。

2026-11-06 同步 v4.0.26 起点为 `8e4ffbf4`，相对 `origin/ccs-dev` 缓存领先 55、落后 26；未 fetch 自有远程或处理该分歧。只核验并合入官方稳定 tag，不移动已有产品发布 tag。

2026-11-09 同步 v4.0.28 起点为 `a04f9406`（0.0.9），相对 `origin/ccs-dev` 缓存领先 97、落后 26；同样未 fetch 自有远程或处理该分歧。数据库 schema 保持 24；上游新增的会话正文索引是独立 `session-index.db`，默认关闭。

2026-11-11 同步 v4.0.29 起点为 `1c86ce63`（0.0.10），相对 `origin/ccs-dev` 缓存领先 164、落后 26；未 fetch 自有远程或处理该分歧。主数据库 schema 升至 25，仅新增 Copilot MCP/Skills 独立开关。

2026-11-17 按用户要求在新分支上以 v4.0.33 为基点线性重建：先在 `reorg/target-4.0.33` 合并 v4.0.33、删除二开 WorkBuddy 并改用上游实现，再把自有改动拆成身份、发布、核心补丁/共存、配置历史、平台登录、平台目录、简易模式与文档 8 个提交，每个提交单独通过全部检查。`ccs-dev`、其他分支、所有 tag 与远程均未移动，未推送。详见 [提交整理记录](../history/commit-reorganization.md)。

历史 [4.0.22 同步报告](../upstream-sync/2026-10-31.md)、[ADR-0007](../adr/ADR-0007-merge-4.0.22.md) 及版本发布证据中的远程旧名保留原事实，不表示当前配置。v4.0.29 合并依据 [ADR-0014](../adr/ADR-0014-merge-4.0.29.md)；当前 v4.0.33 与 WorkBuddy 决策依据 [ADR-0017](../adr/ADR-0017-merge-4.0.33-upstream-workbuddy.md)。

2026-11-21 从产品 `56cba5065`（0.0.11）合入官方 v4.0.37，目标 SHA 与 tag 经 `upstream` 联网核验；起点相对 `origin/ccs-dev` 缓存领先 193、落后 26，没有 fetch 自有远程或处理分歧。schema 27、官方删除的本次用户授权及兼容边界见 [ADR-0015](../adr/ADR-0015-merge-4.0.37.md)。

2026-11-24 从 `12cfb27a9`（0.0.11/v4.0.37）合入 v4.0.40。起点相对 `origin/ccs-dev` 缓存领先 431、落后 26，未处理自有远程分歧。schema 仍为 27，接入 WAL/缓存及 SQLite 一致性快照，回退须考虑 WAL；见 [ADR-0016](../adr/ADR-0016-merge-4.0.40.md)。

## 身份、目录与凭据

| 内容 | 当前实现 | 代码入口 |
| --- | --- | --- |
| 应用身份 | productName `Zhishu CC Switch`、identifier `com.zhishu.ccswitch.desktop`、binary `zhishu-ccs`、深链 `zhishu-ccswitch` | `src-tauri/tauri.conf.json` |
| 应用数据库 | 默认 `~/.zhishu-ccs/cc-switch.db`；目录覆盖仍由既有函数解析 | `src-tauri/src/config.rs`、`app_store.rs` |
| 数据库结构 | schema `27`；WAL/读缓存/一致性快照，保留 WorkBuddy 开关、仓库主机及历史分支协调 | `src-tauri/src/database/mod.rs`、`schema.rs` |
| OAuth2 授权码 / PKCE verifier / access token | Rust 内存 | `src-tauri/src/zhishu/services/platform_auth.rs` |
| refresh token | 独立系统 keyring service；会话元数据不含 token，密码登录及历史存储已删除 | `src-tauri/src/zhishu/storage/platform_session.rs` |
| 平台目录与模型 Key | 账号独立的 `platform-catalogs/<账号摘要>.json` v2；Unix 0600 私密原子写入，仍为明文 | `src-tauri/src/zhishu/services/platform_models/base_data.rs` |
| 设备文件位置 | 使用 Tauri `app_local_data_dir()` | `src-tauri/src/zhishu/storage/platform_storage.rs` |
| 普通供应商 Key | SQLite 的 `settings_config`，生效后可能进入 CLI 原生配置 | 上游 ProviderService、DAO 与配置适配器 |

平台模型 Key 的目录、供应商行、原生配置及部分历史/备份副本不等于 keyring 加密存储。标准 §4.2 是安全目标，当前实现仍有差异；本次整理不迁移、删除或改写任何真实凭据。详情见 [功能说明](../features/platform-managed-models.md) 和 [ADR-0005](../adr/ADR-0005-account-local-cleanup.md)。

`pnpm dev` 使用正常数据和工具配置路径，不注入目录覆盖；测试需使用独立 `CC_SWITCH_TEST_HOME` 及目标工具覆盖变量。见 [开发启动](development.md) 和 [ADR-0010](../adr/ADR-0010-remove-development-isolation.md)。

## 当前能力与已知边界

- 上游 v4.0.40 接入 WAL、查询缓存、一致性备份、SQL 导入和代理/脚本安全加固、Grok 官方账号及 Skills/测试模块拆分；智枢共存保护与平台契约保留。实机与跨平台状态以同步报告为准。

- 上游 v4.0.31–v4.0.37 接入 WorkBuddy 原生支持（官方账号、MCP/Skills、会话/用量）、德语、`APP_SPECS` 能力表、隐私过滤、IP 诊断、Skills GitLab/Gitea/Forgejo 主机和后台更新。本分支 WorkBuddy 只用上游实现：二开适配器已删除，上游 `workbuddy_config.rs` 仅有归属字段 `zhishuCcsProviderId` 与设置页目录覆盖两处补丁；平台同步经上游写入（`url` 为完整 `/chat/completions` 地址），并认领旧版智枢写入的未标记平台条目；原版 `ccSwitchProviderId` 条目不接管。德语中的智枢/历史兼容文案沿用主线的英语回退。平台白名单不扩大，真实工具、语言与跨平台升级仍待验收。

- 上游 v4.0.29 新增 Copilot CLI 原生卡片、MCP、Skills、提示词和会话/用量；Copilot 未加入智枢平台同步白名单或简易模式接入页。新增韩语/俄语，七语字典包含智枢扩展；语言实机验收待执行。
- 平台 HTTP 只允许 `/api/zhishu/v1/client` 的八个方法/路径组合；Key 唯一来源为 `POST /access`。客户端不创建或撤销共享模型 Key。
- 登录或恢复成功后只尝试一次目录快照同步，不改写供应商列表；接入由用户确认的「接入到工具」向导（模式应用可逐应用选择直连/路由/聚合）或手动本地同步执行，手动本地同步只读取快照。前后端平台白名单为 13 个应用，包含 `workbuddy` 和 `dsh`，不包含 `agy`。定义见 `src/zhishu/models/platformConfig.ts` 与 Rust Adapter 的 `AGENTS`。
- 平台登录允许 HTTP(S)，移除 URL 用户信息、查询和片段；HTTP 不是加密传输。生产构建地址检查与登录输入校验分别执行，HTTPS 证书校验及拒绝重定向仍保留。
- 普通退出保留本机模型 Key；显式清理退出仅处理当前账号目录、托管供应商和本次会话，安全保留项及其他账号不删除。没有多活动会话或未验证离线入口。
- 发布工作流构建 macOS universal 与 Windows x64/ARM64 NSIS EXE；Linux 暂停新包。操作入口见 [发布指南](../releases/README.md)，历史验签与例外见各版本记录。
- OAuth keep-alive 替换测试曾出现 `IncompleteMessage` 偶发失败；v4.0.25 已吸收上游对失效连接的测试重试修复，保留历史失败及复测结果，不把单次复测等同稳定性问题消失。
- `src-tauri/src/commands/misc.rs` 仍有便携更新地址 TODO；原便携版不在当前新包矩阵中，后续恢复该渠道时须独立验收。
- 真实 keyring、跨平台安装升级、CLI 模型调用和共存验证按具体记录判断；本次只核对源码和文档，不补记未执行的验收。

补丁登记见 [upstream-patches.md](upstream-patches.md)。旧分支重组、原 SHA 和恢复依据见 [提交整理记录](../history/commit-reorganization.md)；不能将旧整理分支“剔除 Harness”的结果误认为当前 4.0.22 分支不支持 DSH。
