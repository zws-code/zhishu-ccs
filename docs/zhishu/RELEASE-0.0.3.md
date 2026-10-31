# 0.0.3 发布与验证记录

日期：2026-10-31（Asia/Shanghai）。状态：发布准备中，尚未推正式 tag。

## 范围与实施说明

- 用户要求正式发布 0.0.3，并启用 COS 国际/全球加速上传。
- 业务基线：`abd1f3f9212af65c8b143a549e40877f518f5208`；相对 0.0.2 包含客户端专用 API、账号独立目录与清理退出、清理结果弹窗、模型搜索及详情滚动修复。
- 上游基线：4.0.21 / `c4db63c60d2a1ddcb4bd9f81152b9c6126413659`，本次通过 merge-base 复核。本次不混入上游同步；0.0.2 记录中的恒定时间认证比较、代理自循环保护等待吸收项仍是已知风险，未宣称完成最新上游安全审计。
- 复用既有 GitHub Actions、Tauri updater 签名、COS 不可变上传与稳定清单发布；平台业务继续复用 ProviderService、DAO、配置写入和恢复链路。
- L1：统一 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 版本；Cargo 使用 `update --workspace --offline` 同步锁文件，仅改变本包版本。固定工具入口无法由新增 Adapter 替代。
- L1：`.github/workflows/release.yml` 仅为 `refs/tags/v0.0.3` 强制启用 `COS_UPLOAD_ACCELERATE=true`，其余 ref 沿用仓库变量/default false；添加当前发布分支作为构建演练入口，分支推送不上传 COS。
- L0：新增三条四语应用公告和本记录，更新补丁登记。保留用户原有未跟踪 `out/`。
- 不改应用身份、生产更新地址、公钥、安装格式、数据库 schema 或凭据策略。发布矩阵仍为 macOS universal 与 Windows x64/ARM64 NSIS EXE，Linux 暂停。

## 验收与风险

完成标准：本地回归、GitHub 平台构建、签名/哈希验证、加速上传成功、两个稳定清单和 GitHub Release 一致，保留原 payload。

上传加速继续使用既有 `cos.accelerate.myqcloud.com`，累计读取上限 3 GB（包含重试）；客户端仍用 COS 国内 HTTPS 更新/下载地址。只为本次版本启用，不修改后续发布的仓库变量。

生产客户端 `/api/zhishu/v1/client/config` 返回 HTTP 200/code 0，未认证 `/account` 返回 401。仅检查公开端点，不代表真实登录、2FA、keyring、账号清理和实际 CLI 已验收。

当前生产构建配置沿用平台 IP HTTP 地址、macOS adhoc（未公证）；Tauri 更新签名独立保留。Windows/macOS 实机安装升级、真实账户与 CLI 验收尚未完成，不能以编译成功代替。

## 本次验证

| 检查 | 本次结果 |
| --- | --- |
| `pnpm typecheck` / `pnpm format:check` | 通过 |
| `pnpm test:unit` | 283 个文件、3613 项全部通过 |
| `pnpm test:release` | 36 项通过 |
| `pnpm test:release-cos` | 初轮 23 项、2 项因缺 SDK 跳过；临时 venv 安装固定 SDK 后 23 项全部通过 |
| `pnpm build:renderer` | 通过，保留既有 chunk 等构建警告 |
| 生产公开配置 `release:check v0.0.3` | 通过，未连接腾讯云 |
| 工作流 YAML 解析 / `git diff --check` | 通过 |
| `cargo fmt --check` | 通过 |
| Rust 全量隔离回归 / Clippy | 运行中，尚不宣称通过 |

GitHub 现存运行 `41193536620` 是旧 `codex/cos-validation` / `7e8b707` 的分支演练，包含旧 Linux 矩阵，与本次 0.0.3 无关。本次不取消或覆盖它，使用独立发布分支。

## 恢复

发布前可撤销准备提交；正式上传后保留 tag 和不可变版本目录。失败复用原签名 payload，重跑失败作业或使用恢复流程；代码修复发更高版本，不覆盖同版本、不降级稳定清单。两份稳定清单按顺序切换，不宣称跨对象事务。
