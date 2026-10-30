# 0.0.2 发布准备与验证

日期：2026-10-30（Asia/Shanghai）。此记录尚不表示正式发布完成。

## 范围与基线

- 待发布业务基线：`edb5f895fe741f773ca88e37b79d0c4e9c359267`。
- 已合入上游：4.0.21 / `c4db63c60d2a1ddcb4bd9f81152b9c6126413659`；本次不合并新的上游提交。2026-10-30 通过 GitHub compare 核对，上游 main 为 `9941a6455ef9405cf9d3620057fddd283223a607`，相对该基线新增 59 个提交（含 4.0.22）。
- 复用现有 Provider、平台会话、目录同步及配置写入实现；本次准备仅统一 package/Tauri/Cargo/lock 版本，修正公告中的旧版本号，并补充发行记录。
- L1：固定构建入口的版本字段必须就地调整；补丁登记 ZS-044、046、047、148 已复核。
- 发行矩阵沿用 ADR-0003：macOS universal，Windows x64/ARM64 NSIS EXE；Linux 暂停。
- 当前工作区已有 `AGENTS.md` 修改和 `out/`，不纳入版本提交。
- 本地整理后的历史与已发布 `ccw-dev` 分叉，采用独立 `codex/release-0.0.2` 分支，不强推旧分支或移动既有 tag。

## 本次已执行

| 验证 | 结果 |
| --- | --- |
| `pnpm typecheck` | 通过 |
| `pnpm format:check` | 通过 |
| 公告契约测试 | 3 项通过；0.0.2 文件内部版本已修正 |
| `pnpm test:unit` | 283 文件、3591 项通过；存在既有 MSW 等测试诊断 |
| `pnpm build:renderer` | 通过；保留既有大 chunk 提示 |
| `pnpm test:release` | 35 项通过 |
| `pnpm test:release-cos` | 首轮 21 项通过、2 项因缺少 COS SDK 跳过；临时 venv 安装固定 SDK 后完整 23 项全部通过 |
| `cargo fmt --check --manifest-path src-tauri/Cargo.toml` | 通过 |
| `cargo clippy --locked --manifest-path src-tauri/Cargo.toml -- -D warnings` | 通过 |
| `git diff --check` | 通过 |
| 线上版本只读核对 | COS 与 GitHub 最新版本仍为 0.0.1，远端没有 v0.0.2 tag |
| 官网 EXE 解析兼容 | 实际读取生产服务器下载页资源 `ClientDownloadView-BULWN5Ze.js`，其 Windows 格式白名单含 exe/msi/portable |

隔离目录 Rust 全量回归已完成：4361 项单元测试 + 212 项集成测试通过，11 项忽略，无失败。使用 `CC_SWITCH_TEST_HOME` 独立临时目录，命令为 `cargo test --locked --manifest-path src-tauri/Cargo.toml -- --test-threads=1`。

云端演练（已成功）：`41027890897`，候选提交 `c397fd1b0d4e58735976ef78e766f9d82b4feffc`，分支 `codex/release-0.0.2`。手动演练仅构建/验签，不上传 COS，不创建 Release。

GitHub Actions 多平台构建与最终产物校验已通过；真实 COS SDK 契约已在本机临时环境补测通过。

## 待完成

- x64/ARM64 Windows 的 MSI→NSIS 安装、更新、重启、卸载实机回归；macOS 0.0.1→0.0.2 实际升级。
- 正式 tag、COS 不可变产物、稳定清单、GitHub Release 与公网下载验收。

## 风险和恢复

本次版本准备不引入数据库或凭据迁移。应用身份、公钥及更新源沿用既有正式配置，不能使用本机旧测试 HTTP 更新配置替代 GitHub Repository Variables。

Windows 跨安装器升级尚未验收，不能将构建成功描述为升级验证通过。macOS 当前沿用 adhoc 模式时不得宣称 Apple 公证。上线前遵守 ADR-0003 的目标平台验收要求。

发布失败复用原签名产物，仅重试失败作业；同版本不可覆盖、不移动已发布 tag、不降级稳定清单。正式发布后的代码修复使用更高版本。

## 尚未吸收的上游变更

发布前只读核对了官方 `c4db63c6...main` 提交列表，重点审阅以下补丁，并与本地实现比对；本次发布准备不混入上游同步。

- `1718a261`：Claude Desktop gateway token 恒定时间比较与统一认证失败响应。本地仍使用普通字符串比较，尚未吸收；保留时序侧信道风险，不能声称具备该安全加固。
- `cd9c7210`：拒绝转发到代理自身监听地址。本地尚无该保护；错误配置上游地址仍可能造成自循环请求和资源消耗。
- `ddcb1117`：TOML array-of-tables section 检测；`9941a645`：invalid_grant 永久失效分类及 .env export 清理等修复尚待独立兼容性评估。
- 官方 DeepSeek Harness Web/Desktop 扩展涉及本项目已有 desktop 适配，不能在发布过程中直接覆盖。

以上是提交列表与两个关键补丁的有限审阅，不是上游全部 59 项变更的完整安全审计。

## 发布检查路径修正

发现正式发布与恢复工作流的 COS SDK 契约测试仍指向迁移前的 `scripts`，本次实测原命令执行 0 个测试。两个入口改为 `scripts/zhishu`，复用原有两项 SDK 测试；L1 固定工作流接入修复，补丁登记 ZS-003 已补充。该改动不影响正在构建的候选安装包代码。

修正后验证：临时虚拟环境安装 `cos-python-sdk-v5==1.9.44`，相同发现命令执行 2 项契约测试全部通过；完整 COS 套件 23 项全部通过、无跳过、无真实上传。工作流使用项目已有 Prettier YAML 解析器通过语法/格式稳定性检查；初始直接导入未安装的 `yaml` 包失败，未因此增加项目依赖。

## 云端候选构建结果（2026-10-30 17:00，Asia/Shanghai）

[构建演练 41027890897](https://github.com/zws-code/zhishu-ccs/actions/runs/41027890897) 的预检、macOS 两架构编译、universal 打包、Windows x64/ARM64 EXE 构建及发布产物校验全部成功。正式上传步骤未执行，GitHub Release 作业按手动演练条件跳过；线上仍为 0.0.1。

已保存 `cos-payload-v0.0.2`（artifact `12965377474`，162613662 字节）以及各平台原产物。签名/哈希校验后的 dry-run 输出：`Validated v0.0.2; no network requests or uploads`。macOS 日志确认 `MACOS_SIGNING_MODE: adhoc`，`.app` 和 DMG 的 codesign 检查通过，未执行 Apple 公证。

后续提交 `b0c781fb` 只修正正式发布/恢复流程的 COS SDK 测试发现目录并更新文档，未改变这批候选安装包的应用代码。该修复已执行真实 SDK 两项契约与全部 23 项 COS 测试，工作流 YAML 检查通过。

仍未创建或推送 `v0.0.2`，尚未上传 COS 或切换稳定清单。正式发布须完成 ADR-0003 所要求的 Windows 跨安装器安装/升级验证；如果用户明确批准本次延期实机验收，应先登记批准范围与待补验收项，再推正式 tag。候选演练产物不能伪装成正式 tag 构建来源供恢复流程发布。

## 本次实机升级验收延期批准（EXCEPTION-RELEASE-002）

2026-10-30，用户在本次发布会话中，在已明确说明 Windows MSI→EXE 升级兼容性尚未实机确认、自动测试/多平台构建/签名校验已通过后，回复“可以”，明确批准先正式发布 0.0.2，之后补做旧版本升级测试。

批准范围仅限本次将 Windows x64/ARM64 的旧版安装、升级、重启、卸载，以及 macOS 0.0.1→0.0.2 实际升级验收延期；不免除版本一致性、签名/哈希验证、COS 不可变文件保护、禁止降级、平台完整性或公网发布检查。维护者负责在后续实际升级验收中补充结果；该批准不自动适用于后续版本，也不代表已验证升级兼容性。后续跟踪入口为本文件“待完成”清单。

按既有正式流程在本次提交创建 `v0.0.2`，推送到 `zhishu-git`，由正式 tag 运行构建并发布。线上安装包来源必须是正式 tag 构建的已验证 payload；演练原产物保留，不作为恢复发布的来源。
