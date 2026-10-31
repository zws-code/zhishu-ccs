# Zhishu CCS 当前代码基线与限制

> 核对日期：2026-10-31。依据本地工作区、Git 引用和配置文件；未联网 fetch 或重新核验生产发布状态。

## 版本、上游与远程

| 项目 | 当前本地事实 | 依据 |
| --- | --- | --- |
| 产品版本 | `0.0.3` | `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` |
| 分支 / 核对时 HEAD | `ccs-dev` / `2fea90d7eb5d215bea390d041d3ef47aee4e1f9c`；含未提交改动 | `git branch --show-current`、`git rev-parse HEAD`、`git status --short` |
| 已合入上游 | `4.0.22` / `b76426741ef93069e2f374ac01ca7ef5760e1d0e` | 本地 tag；`git merge-base HEAD 4.0.22` 等于该提交 |
| 官方 remote | `origin` → `git@github.com:farion1231/cc-switch.git` | `git remote -v` |
| 自有 GitHub | `zhishu-git` → `git@github.com:zws-code/zhishu-ccs.git` | 同上；发布 tag 使用此 remote |
| 自有 GitLab | `zhishu-origin` → `http://172.16.32.4:8929/hunanzs/zhishu-ccs.git` | 同上 |
| Rust / pnpm | Rust `1.95`；pnpm `10.12.3` | `rust-toolchain.toml`、`package.json` |

当前无 `upstream` remote。标准中的 `origin=自有仓库 / upstream=官方` 是目标命名，不能原样套用其示例推送命令；本次保留真实远程和历史不变。上游合并依据见 [4.0.22 同步报告](../upstream-sync/2026-10-31.md) 与 [ADR-0007](../adr/ADR-0007-merge-4.0.22.md)。

## 身份、目录与凭据

| 内容 | 当前实现 | 代码入口 |
| --- | --- | --- |
| 应用身份 | productName `Zhishu CC Switch`、identifier `com.zhishu.ccswitch.desktop`、binary `zhishu-ccs`、深链 `zhishu-ccswitch` | `src-tauri/tauri.conf.json` |
| 应用数据库 | 默认 `~/.zhishu-ccs/cc-switch.db`；目录覆盖仍由既有函数解析 | `src-tauri/src/config.rs`、`app_store.rs` |
| 数据库结构 | schema `24`；保留智枢 v23 缺失 OMP 字段的幂等协调 | `src-tauri/src/database/mod.rs`、`schema.rs` |
| 登录 access / 2FA token | Rust 内存 | `src-tauri/src/zhishu/services/platform_auth.rs` |
| refresh token / 记住密码 | 两个独立的系统 keyring service；元数据文件不含 token 或密码 | `src-tauri/src/zhishu/storage/platform_session.rs`、`platform_login_history.rs` |
| 平台目录与模型 Key | 账号独立的 `platform-catalogs/<账号摘要>.json` v2；Unix 0600 私密原子写入，仍为明文 | `src-tauri/src/zhishu/services/platform_models/base_data.rs` |
| 设备文件位置 | 正常使用 Tauri `app_local_data_dir()`；开发隔离沿用 `get_app_config_dir()` | `src-tauri/src/zhishu/storage/platform_storage.rs` |
| 普通供应商 Key | SQLite 的 `settings_config`，生效后可能进入 CLI 原生配置 | 上游 ProviderService、DAO 与配置适配器 |

平台模型 Key 的目录、供应商行、原生配置及部分历史/备份副本不等于 keyring 加密存储。标准 §4.2 是安全目标，当前实现仍有差异；本次整理不迁移、删除或改写任何真实凭据。详情见 [功能说明](../features/platform-managed-models.md) 和 [ADR-0005](../adr/ADR-0005-account-local-cleanup.md)。

`pnpm dev` 仅隔离应用数据与 Codex，其他 CLI 仍可写真实用户目录；测试需使用独立 `CC_SWITCH_TEST_HOME` 及目标工具覆盖变量。见 [隔离开发](dev-isolation.md)。

## 当前能力与已知边界

- 平台 HTTP 只允许 `/api/zhishu/v1/client` 的八个方法/路径组合；Key 唯一来源为 `POST /access`。客户端不创建或撤销共享模型 Key。
- 登录或恢复成功后尝试一次目录同步，再自动同步全部支持工具；手动本地同步只读取快照。前后端平台白名单为 13 个应用，包含 `workbuddy` 和 `dsh`，不包含 `agy`。定义见 `src/zhishu/models/platformConfig.ts` 与 Rust Adapter 的 `AGENTS`。
- 平台登录允许 HTTP(S)，移除 URL 用户信息、查询和片段；HTTP 不是加密传输。生产构建地址检查与登录输入校验分别执行，HTTPS 证书校验及拒绝重定向仍保留。
- 普通退出保留本机模型 Key；显式清理退出仅处理当前账号目录、托管供应商和本次会话，安全保留项及其他账号不删除。没有多活动会话或未验证离线入口。
- 发布工作流构建 macOS universal 与 Windows x64/ARM64 NSIS EXE；Linux 暂停新包。操作入口见 [发布指南](../releases/README.md)，历史验签与例外见各版本记录。
- OAuth keep-alive 替换测试曾出现 `IncompleteMessage` 偶发失败；保留历史失败及复测结果，不把单次复测等同稳定性问题消失。
- `src-tauri/src/commands/misc.rs` 仍有便携更新地址 TODO；原便携版不在当前新包矩阵中，后续恢复该渠道时须独立验收。
- 真实 keyring、跨平台安装升级、CLI 模型调用和共存验证按具体记录判断；本次只核对源码和文档，不补记未执行的验收。

补丁登记见 [upstream-patches.md](upstream-patches.md)。旧分支重组、原 SHA 和恢复依据见 [提交整理记录](../history/commit-reorganization.md)；不能将旧整理分支“剔除 Harness”的结果误认为当前 4.0.22 分支不支持 DSH。
