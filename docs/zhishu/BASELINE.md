# Zhishu CCS Fork 基线

- 评估日期：2026-10-29（Asia/Shanghai）
- 整理前分支：`feat/zhishu-structure-cleanup` / `f3485e065c25b06152ae3a7994f863ddaca4ef04`
- 官方已合入基线 / merge-base：`c4db63c60d2a1ddcb4bd9f81152b9c6126413659`，4.0.21。
- 本地官方跟踪引用 `origin/main`：`ddcb111765be66ed9e7e43e240497fd2c9576688`；本轮未联网 fetch，不声称是远端最新状态。
- 实际 `origin`：`git@github.com:farion1231/cc-switch.git`；当前没有 `upstream`。
- 智枢远程：`zhishu-git`（GitHub zws-code/zhishu-ccs），`zhishu-origin`（公司 Git 服务）。
- 规范要求 origin=公司仓库、upstream=官方；当前命名不符合。本轮为避免改动既有发布/推送配置保持远程不动，后续需选定正式公司 origin 再调整。
- 本地 main 指向 `d8983bb1`；本轮不更新 main、不合入更新的 upstream，不发布。
- 产品：`zhishu-ccs` 0.0.1；Tauri identifier `com.zhishu.ccswitch.desktop`，productName `Zhishu CC Switch`。
- 应用数据默认 `~/.zhishu-ccs/cc-switch.db`，设置同一应用目录；设备数据及自定义覆盖规则见 `config.rs`。
- 开发启动仅隔离应用数据和 Codex；Claude/Gemini 等仍沿用原目录，不能宣称全工具隔离。
- 认证会话与登录历史保存在设备本地目录，敏感令牌经系统凭据库；开发隔离目录规则不变。
- 额外工具、平台 Key、配置历史写入继续复用受控服务和现有备份/恢复实现。
- 上游已有文件差异：见 [UPSTREAM_PATCHES.md](UPSTREAM_PATCHES.md)，新增实现见 [README.md](README.md)。
- 原 HEAD 相对固定基线为 343 个差异文件；已迁移 100 个自有文件，本轮没有重写完整上游 UI。
- Git 历史与回退见 [HISTORY_REWRITE.md](HISTORY_REWRITE.md)。

## 已知问题和边界

1. 严格 clippy 原有两个 dead-code 错误：commands/misc.rs 的平台启动函数；原合并报告已经记录。
2. 上游 OAuth keep-alive 替换测试存在 IncompleteMessage 不稳定性；本轮再次出现，验证记录必须保留失败及复测结果。
3. 便携更新下载地址仍有上游 URL TODO；本轮不改变更新协议，正式发布前需独立修复/验证。
4. 库 schema v23 的 OMP 兼容按 ADR-0001 保留；未以本次目录整理引入迁移或更换应用身份。
5. Windows/macOS 实机安装、自动更新、Keychain 真机交互和共存回归不等同于本次自动化测试。
6. 历史机密检查为有限启发式审查，不能承诺整个上游及所有备份分支历史均已完成安全审计。
7. 所有现有公开 IPC 名称、存储格式与路径保持兼容；新增模块可见性只在 crate 内扩展所需校验调用。

## 上游合并演练

本轮没有再次合并上游。已存在的 4.0.21 合并结果与压缩后的合并节点 tree 完全相同，
保留官方第二父节点。原实际合并与风险报告见 [upstream-sync/2026-10-29.md](upstream-sync/2026-10-29.md)。
