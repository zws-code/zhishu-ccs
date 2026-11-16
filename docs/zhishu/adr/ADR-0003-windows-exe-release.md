# ADR-0003：Windows 仅发布 EXE，暂停 Linux 打包

- 日期：2026-10-29
- 状态：按用户确认的 ARM64 + x64 范围实施；正式发布前须完成 Windows 安装/升级实机回归。
- 侵入等级：L1（构建入口）；因涉及安装格式和更新平台策略，单独记录 ADR。

## 背景与目标

现有流程构建 Windows MSI、便携 ZIP 及 Linux 两种架构的 AppImage/DEB/RPM。用户要求 Windows 仅保留 ARM64 和 x64 的 EXE，暂不打包 Linux。macOS universal 产物沿用原流程。

验收：Windows 每架构只发布一个 NSIS EXE 及其 .sig；更新清单保留 windows-x86_64/windows-aarch64；Linux 不参与正式构建和新版本清单；COS 可从旧的全平台清单升级到新策略，但不能丢失 macOS/Windows；旧签名包仍能恢复发布。

## 备选方案与选择

1. 仅删 CI 矩阵：不可行，产物校验、更新清单和 COS 的平台完整性保护仍要求 Linux/MSI。
2. 发布裸 EXE：不采用，会失去安装器和现有 Tauri 自动更新流程。
3. 复用 Tauri NSIS、签名验证及 COS 发布器，统一调整矩阵、命名与清单（采用）。NSIS 使用 currentUser，与原 MSI 的 perUser 安装范围一致。保留历史 MSI/Linux 格式用于旧包恢复，不纳入当前构建目标。

## 文件和上游边界

- 新增本 ADR；修改自有 scripts/zhishu/release-artifacts.mjs、cos-release.mjs、publish-cos.py 及关联测试，恢复工作流和发布文档。
- `.github/workflows/release.yml`：固定 CI 入口必须修改矩阵、打包命令和清单生成；Adapter 不能取消已有作业。
- `src-tauri/tauri.windows.conf.json`：Tauri 自动读取的平台配置，指定 NSIS 为默认产物并保留当前用户安装范围；新增业务模块不能修改默认打包行为。
- `scripts/generate-download-manifest.mjs`：现有清单生成器必须识别 EXE 格式；复用原生成器，避免维护平行清单实现。
- 上述上游修改均登记在 UPSTREAM_PATCHES.md。

## 兼容、安全和迁移

应用 identifier、productName、mainBinaryName、更新公钥/端点、数据库及 CLI 配置路径均沿用现值。不访问用户 ~/.codex、~/.claude 或真实凭据。Tauri 更新签名继续验证，不能视为 Windows Authenticode 签名。

Windows MSI → NSIS 的安装记录、旧安装目录清理、深链/快捷方式、更新后重启和卸载须在 x64/ARM64 实机验证；不能仅以打包成功认定跨安装器升级兼容。当前用户安装范围不变，但不承诺安装器格式的身份完全等价。

COS 仅允许明确列出的 6 个旧 Linux updater key 暂停，仍拒绝其他平台丢失、版本降级和同版本内容变化。新清单不向旧 Linux 客户端提供新版本；旧版包和不可变版本清单继续保留，恢复 Linux 时恢复构建及验收后再发布更高版本。

新收据以 packaging=nsis-v1 记录产物策略；无此标记的历史收据按旧 MSI/Linux 策略验签。不能重新构建或替换 v0.0.1。新版官网清单使用 kind=exe；官网消费者必须接受该格式后再切换清单。

## 测试与发布门槛

运行 Node 发布配置/产物/恢复回归、Python COS 发布与失败恢复测试、工作流 YAML/脚本语法检查。覆盖 EXE 下载清单、两种 Windows 签名缺失/损坏、旧包恢复、Linux 暂停以及其他平台保护。macOS/Windows 实际构建和安装更新需要 CI/目标平台，未执行时明确记录。

## 回退与复审

代码可回退本次提交；正式发布后只能通过更高版本恢复旧打包策略，不能覆盖已发布 tag、同版本 COS 文件或直接降级稳定清单。MSI/NSIS 逆向切换也须实机验证，用户数据库不因本次构建调整发生迁移。首次 EXE 发布前及 Linux 恢复打包时复审。


## 本次实施与验证记录

实现分支：CCS `feat/windows-exe-release`；官网 `feature/windows-exe-download`。官网改动已获用户明确授权，仅修改代码，不部署。

| 检查 | 结果 |
| --- | --- |
| `node --test scripts/zhishu/{release-config,release-artifacts,cos-release,resume-release}.test.mjs`（与 `pnpm test:release` 相同入口） | 35 项通过；新 EXE 签名/清单、历史 MSI/Linux 验签、非预期产物拒绝 |
| `python3 -B -m unittest discover -s scripts/zhishu -p 'test_publish_cos.py'` | 23 项中 21 项通过、2 项跳过；本机未装 COS SDK，真实 SDK 契约仍需在安装依赖的环境补跑 |
| 修改过的 JS/JSON Prettier 检查 | 通过 |
| 发布与恢复工作流 YAML 解析 | 通过；发布构建矩阵仅两种 Windows runner |
| 发布工作流内嵌清单生成 JS 语法检查 | 通过 |
| Windows 配置对已安装 Tauri JSON Schema 校验 | 通过；Ajv 忽略 Schema 中额外的 Rust 数值 format，类型/枚举/结构校验有效 |
| CCS → 官网清单契约 | 实际运行清单生成器，官网 parseClientRelease 成功解析四项下载，Windows 分别为 ARM64/x64 EXE |
| 官网 `make test-frontend` 的等价现有工具检查 | ESLint、vue-tsc 通过；读取 Makefile 的关键测试列表，32 文件 / 610 项通过；测试输出含既有 jsdom 导航提示 |
| 官网 `vue-tsc -b` + `vite build` | 通过；i18n 检查在关键测试列表中通过；保留原有大 chunk 提示 |
| 两仓库 `git diff --check` | 通过 |

未运行：客户端全量 `pnpm typecheck`、`pnpm format:check`、`pnpm test:unit`、`pnpm build:renderer`、Rust fmt/clippy/test（本次未改客户端 React/Rust 业务）；官网 Go 测试/竞态/集成/构建和真实模型转发（仅前端解析扩展）。本次不是上游同步或正式发布，正式发布仍须完整门禁。

未运行目标平台打包及安装/更新：当前为 macOS 本地环境，未启动远端 Actions，未构建签名安装器，未执行 x64/ARM64 Windows 的 MSI→NSIS 实机升级。未执行 COS 真实上传、官网部署或生产下载页验证。

本次使用已安装工具以避免包管理器自动安装或修改锁文件；仅本地测试与官网生产构建，未增加依赖。
