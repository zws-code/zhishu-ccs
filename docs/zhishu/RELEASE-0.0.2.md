# 0.0.2 发布结果与验证

日期：2026-10-30（Asia/Shanghai）。已正式发布，GitHub 发布时间为 **18:41:15**；COS、GitHub Release 和公网验收均已完成。

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
| 发布前线上版本只读核对 | COS 与 GitHub 最新版本仍为 0.0.1，远端没有 v0.0.2 tag |
| 官网 EXE 解析兼容 | 实际读取生产服务器下载页资源 `ClientDownloadView-BULWN5Ze.js`，其 Windows 格式白名单含 exe/msi/portable |

隔离目录 Rust 全量回归已完成：4361 项单元测试 + 212 项集成测试通过，11 项忽略，无失败。使用 `CC_SWITCH_TEST_HOME` 独立临时目录，命令为 `cargo test --locked --manifest-path src-tauri/Cargo.toml -- --test-threads=1`。

云端演练（已成功）：`41027890897`，候选提交 `c397fd1b0d4e58735976ef78e766f9d82b4feffc`，分支 `codex/release-0.0.2`。手动演练仅构建/验签，不上传 COS，不创建 Release。

GitHub Actions 多平台构建与最终产物校验已通过；真实 COS SDK 契约已在本机临时环境补测通过。

## 待完成

- x64/ARM64 Windows 的 MSI→NSIS 安装、更新、重启、卸载实机回归；macOS 0.0.1→0.0.2 实际升级。
- `api.hnzhishu.cn` 的域名访问返回备案提示页，需另行处理域名接入；当前服务器 IP 下载入口、COS 和 GitHub 下载已验证正常。

## 风险和恢复

本次版本准备不引入数据库或凭据迁移。应用身份、公钥及更新源沿用既有正式配置，不能使用本机旧测试 HTTP 更新配置替代 GitHub Repository Variables。

Windows 跨安装器升级尚未验收，不能将构建成功描述为升级验证通过。macOS 当前沿用 adhoc 模式时不得宣称 Apple 公证。本次按下述 EXCEPTION-RELEASE-002 延期真实平台升级验收，未将其标为通过。

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

截至上述候选演练完成时，尚未创建或推送 `v0.0.2`、上传 COS 或切换稳定清单；后续正式结果见下文。正式发布须完成 ADR-0003 所要求的 Windows 跨安装器安装/升级验证；如果用户明确批准本次延期实机验收，应先登记批准范围与待补验收项，再推正式 tag。候选演练产物不能伪装成正式 tag 构建来源供恢复流程发布。

## 本次实机升级验收延期批准（EXCEPTION-RELEASE-002）

2026-10-30，用户在本次发布会话中，在已明确说明 Windows MSI→EXE 升级兼容性尚未实机确认、自动测试/多平台构建/签名校验已通过后，回复“可以”，明确批准先正式发布 0.0.2，之后补做旧版本升级测试。

批准范围仅限本次将 Windows x64/ARM64 的旧版安装、升级、重启、卸载，以及 macOS 0.0.1→0.0.2 实际升级验收延期；不免除版本一致性、签名/哈希验证、COS 不可变文件保护、禁止降级、平台完整性或公网发布检查。维护者负责在后续实际升级验收中补充结果；该批准不自动适用于后续版本，也不代表已验证升级兼容性。后续跟踪入口为本文件“待完成”清单。

按既有正式流程在本次提交创建 `v0.0.2`，推送到 `zhishu-git`，由正式 tag 运行构建并发布。线上安装包来源必须是正式 tag 构建的已验证 payload；演练原产物保留，不作为恢复发布的来源。

## 正式发布运行

正式 tag `v0.0.2` 已创建并推送到 `zhishu-git`，固定源码提交 `1aba22d89c9afd10ef8959af5aa2d9259d385e58`（annotated tag object `129a074e82c4ce64b4bfc0e1b1940e07e475dbf0`）。[正式运行 41032256949](https://github.com/zws-code/zhishu-ccs/actions/runs/41032256949) 由 tag push 触发，全部构建成功；普通上传过慢，后按用户指示取消，并通过下述加速恢复完成发布。

发布前再次读取根目录 `latest.json`、`client-release.json` 均为 0.0.1；已保存到本机临时验证目录。现有公开更新密钥与旧版四个 macOS/Windows 平台签名中的 key ID 一致；发布后已对下载后的新包执行完整签名验证。

本次正式运行上传步骤的实际环境已核对：`COS_UPLOAD_ACCELERATE: false`。桶虽已开启全球加速，本次工作流未启用加速地址；随后用户明确批准停止慢速任务，使用同一来源的原包加速恢复，详见下文。

已提前下载并核对正式 macOS ZIP：45,506,687 字节，SHA-256 `6a9f05e80f315a2abc2f7397bcff3384e3ee64a59a90b5a5fe98519d530618b4`。包内版本 `0.0.2`、应用 identifier `com.zhishu.ccswitch.desktop`、Intel/Apple Silicon 两架构，以及实际二进制中的生产 COS 更新地址和既有公钥均匹配。此项仅检查包内容，不代表安装或升级实测。

## 切换国际加速恢复（用户明确授权）

2026-10-30，普通上传长时间未完成，用户明确指示“不等了 直接走国际加速”，授权停止当前慢速上传并以加速方式复用原包恢复。原正式 run 为 `41032256949`，已保存 payload artifact `12967646233`（162611279 字节），来源 tag/commit 保持不变。

恢复脚本原先只接受 success/failure，现增加 cancelled 分支，但额外要求原 COS 作业的 `Save verified payload for retries` 步骤成功；继续拒绝仍在运行、未保存完整产物、错误工作流、测试分支、移动 tag、产物缺失/过期/歧义。下游仍重新验签和校验全部哈希。仅涉及自有 `scripts/zhishu/resume-release.mjs`、对应回归和发行文档，L1；不修改客户端、上游核心、数据或凭据。加速仅用于本次恢复，累计上传读取上限 3 GB（含重试）。

加速恢复补丁已执行 `pnpm test:release`，36 项全部通过；恢复脚本格式及 `git diff --check` 通过。原正式运行已按本次指示取消，原 payload 保存步骤明确为 success，所有构建作业已成功。

## 最终发布与公网验收（已完成）

- [国际加速恢复 41042420751](https://github.com/zws-code/zhishu-ccs/actions/runs/41042420751) 使用恢复脚本提交 `3744806ff5550f3a19ee090239244fa14e7a889a`，来源仍为原正式 run `41032256949` 的 `cos-payload-v0.0.2`（artifact `12967646233`）。COS 和 GitHub Release 作业均 success。
- 恢复上传步骤实际环境为 `COS_UPLOAD_ACCELERATE: true`。最后一个 macOS 更新包共 46,411,346 字节，18:40:35 开始传输，18:40:39 完成 COS 校验；18:40:55 完成稳定清单发布。已存在对象经核验后复用，没有重新编译、替换同版本文件或移动 tag。
- [GitHub Release v0.0.2](https://github.com/zws-code/zhishu-ccs/releases/tag/v0.0.2) 于 2026-10-30 **18:41:15**（Asia/Shanghai）发布；`releases/latest` 返回该版本，非 draft、非 prerelease。11 个附件（10 个 payload 文件及 `release.json`）的大小和 SHA-256 全部与原收据匹配。
- COS 全部 10 个文件均经过实际公开下载、HEAD 大小和 SHA-256 检查，共 163,149,578 字节。根目录两个稳定清单与版本目录文件逐字节相同，版本均为 0.0.2。
- 更新清单含 `darwin-aarch64`、`darwin-x86_64`、`windows-aarch64`、`windows-x86_64` 四个条目；网站清单为 macOS universal DMG/ZIP 以及 Windows x64/ARM64 EXE 四项；Linux 本版暂停。
- 三个独立更新包（macOS tar.gz、两个 Windows EXE）已用既有生产公钥完整验签，四个平台条目与签名结果一致。macOS 更新包中的主程序与前述 ZIP 内双架构主程序逐字节相同。
- 服务器 `http://43.136.64.205/client-release.json` 返回 0.0.2，内容与 COS 网站清单一致。`http://api.hnzhishu.cn/download` 和该域名的清单路径返回 HTML，页面 title 为“备案 / 温馨提示”；不把这个域名入口记为验收通过，也未在本次改动域名或服务器配置。
- macOS 原构建使用 adhoc，未经过 Apple 公证。Windows 更新签名不等于系统 Authenticode 签名。真实安装、旧版升级、重启和卸载验收仍按用户批准延期。

### 公网文件 SHA-256

| 文件 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `Zhishu-Ccs-v0.0.2-Windows-arm64.exe` | 12245897 | `ff4b148f4fa2f7fcf7f0edae7e389f9fd6a47deaf7a60d43f491160c17af8a6e` |
| `Zhishu-Ccs-v0.0.2-Windows-arm64.exe.sig` | 432 | `3325bb55534be9613667614d95f3b7c74a059f745f827d1667269507937d502d` |
| `Zhishu-Ccs-v0.0.2-Windows.exe` | 13618956 | `36ea0c924ff0f9b6e226c434b9ab5f142114ba8c579b25619a77ae7673da751a` |
| `Zhishu-Ccs-v0.0.2-Windows.exe.sig` | 428 | `3a3e972e54cab6734b6ca5dfc8ebc803ab147c0d4d6c4a72a62d0e7e0ca96732` |
| `Zhishu-Ccs-v0.0.2-macOS.dmg` | 45361262 | `52665a24b0a589b73ebd01f5c2156df607d84393046c5febac5af28a1c98333b` |
| `Zhishu-Ccs-v0.0.2-macOS.tar.gz` | 46411346 | `25c3db63814212a69801d18a1d7eee118075306573fd25d2385d97db8d753000` |
| `Zhishu-Ccs-v0.0.2-macOS.tar.gz.sig` | 416 | `a360f777fb4e692f6ef6e4c0cead52deb9e95c7c2227d36925f279dc3c5b50d5` |
| `Zhishu-Ccs-v0.0.2-macOS.zip` | 45506687 | `6a9f05e80f315a2abc2f7397bcff3384e3ee64a59a90b5a5fe98519d530618b4` |
| `client-release.json` | 1596 | `7ddd3735a7a1eb16ce982d031c5653756b63f7364676e63a80bfa9d771d28b4d` |
| `latest.json` | 2558 | `d7d1e8939c02bae2a3de19471700dae0fad1aa34cab4bd7c2e2682f412c90655` |

`release.json` SHA-256：`893451e1270572a0987a1ea7f271d8b7325a29afc4c2a209a8f10858c238fdfa`。
