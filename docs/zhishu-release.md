# 智枢客户端：GitHub Actions + 腾讯云 COS/CDN 发布

客户端正式发布只使用本流程：GitHub Actions 构建和签名 → COS 保存版本文件并切换更新清单 → GitHub Release 附上同一批安装包。客户端使用 COS HTTPS 更新源，有自有域名后再接 CDN；GitHub Releases 提供额外的安装包下载入口。此文档是唯一发布操作入口。

仓库：<https://github.com/zws-code/zhishu-ccs>。旧本地打包后 SSH 上传的二开脚本和配置示例已删除；本仓库在 COS 发布成功后自动创建智枢 GitHub Release，不运行上游 R2 镜像。上游相关实现保留，仅允许在上游仓库执行。

本次作为首次正式发布，不兼容旧测试包的更新入口；旧测试用户直接安装正式包。后续正式版本必须沿用本次的应用身份、Windows 安装身份及更新公钥。

## 0.0.1 正式发布结果（2026-10-29）

- `v0.0.1` 已正式发布，GitHub 发布时间为北京时间 **2026-10-29 15:21**。源码 tag 固定在 `b7619e916f3d7d1feb2d001c4ca279a99d8e6889`；[原正式构建](https://github.com/zws-code/zhishu-ccs/actions/runs/40825474319)完成全部平台编译与签名，上传参数不兼容的问题修复后，[复用原包的恢复发布](https://github.com/zws-code/zhishu-ccs/actions/runs/40834709561)中 COS 与 GitHub Release 两个作业均成功。没有重新编译或替换同版本安装包。
- [GitHub Release v0.0.1](https://github.com/zws-code/zhishu-ccs/releases/tag/v0.0.1) 已标记为 Latest；25 个附件的大小和 SHA-256 全部与原发布记录匹配。
- COS 桶为广州 `zhishu-ccs-release-1252121424`，版本目录为 `ccs/stable/v0.0.1/`。24 个发布文件和独立 `release.json` 已上传；24 个文件均通过公网 HEAD 大小检查。
- 根目录 `latest.json` 与 `client-release.json` 均已切到 `0.0.1`，并与版本目录对应清单逐字节一致；更新清单含 10 个平台/格式条目，网页清单含 12 个下载条目。
- 已实际下载 45,320,138 字节的 macOS 更新包，SHA-256 为 `db4f238b4196a4552699b3c05e36f06bc637a605b171add32f7c3a2580d277f3`，用生产公钥验证 Tauri 更新签名成功。包内版本为 `0.0.1`，identifier 为 `com.zhishu.ccswitch.desktop`，包含 Intel 与 Apple Silicon 架构；已在实际二进制中核对 COS 更新地址和生产公钥。
- 正式包已启用启动时检查更新和“设置 → 关于”检查/安装更新；本次为首次正式版本，尚未做两个正式版本间的实际安装升级验收，也未完成各系统的人工安装、启动与登录验收。旧测试包直接安装本次正式包。
- macOS 使用 `adhoc`，未经过 Apple 公证；Windows 未配置系统代码签名，Tauri 更新签名独立保留。
- 当前无自有域名、无 CDN；客户端使用 COS 默认 HTTPS 域名。用户已开启桶全球加速，并授权本次加速费用预算 5 元；仅本次恢复运行显式启用上传加速，累计上传读取上限 3 GB（含重试），未为后续发布自动启用加速。没有部署或重启平台网关，平台原下载页仍需单独接入。

直接下载安装包：

| 平台 | GitHub 下载 | COS 国内直连 |
| --- | --- | --- |
| macOS（Intel / Apple Silicon 通用） | [DMG](https://github.com/zws-code/zhishu-ccs/releases/download/v0.0.1/Zhishu-Ccs-v0.0.1-macOS.dmg) | [DMG](https://zhishu-ccs-release-1252121424.cos.ap-guangzhou.myqcloud.com/ccs/stable/v0.0.1/Zhishu-Ccs-v0.0.1-macOS.dmg) |
| Windows x64 | [MSI](https://github.com/zws-code/zhishu-ccs/releases/download/v0.0.1/Zhishu-Ccs-v0.0.1-Windows.msi) | [MSI](https://zhishu-ccs-release-1252121424.cos.ap-guangzhou.myqcloud.com/ccs/stable/v0.0.1/Zhishu-Ccs-v0.0.1-Windows.msi) |
| Windows ARM64 | [MSI](https://github.com/zws-code/zhishu-ccs/releases/download/v0.0.1/Zhishu-Ccs-v0.0.1-Windows-arm64.msi) | [MSI](https://zhishu-ccs-release-1252121424.cos.ap-guangzhou.myqcloud.com/ccs/stable/v0.0.1/Zhishu-Ccs-v0.0.1-Windows-arm64.msi) |

Linux、便携包及全部签名见上述 Release 的 **Assets**。

当前下载根地址：`https://zhishu-ccs-release-1252121424.cos.ap-guangzhou.myqcloud.com/ccs/stable`。

## 日常手动发布：以后照此操作

这里的“手动发布”指手动准备版本、提交并推送 tag；编译、签名、COS 上传和 GitHub Release 由 GitHub Actions 自动完成，不需要本机打包或手工往桶里传文件。

### 第一步：准备下一个版本

当前首次正式版本为 `0.0.1`。下一次可以使用 `0.0.2`，以下以它为例；**已经正式发布的版本号不能重复使用**。

1. 在本机仓库完成本次要发布的代码，先用 `git status --short` 核对改动。
2. 将以下文件中的应用版本统一改为 `0.0.2`：
   - `package.json` 的顶层 `version`。
   - `src-tauri/tauri.conf.json` 的顶层 `version`。
   - `src-tauri/Cargo.toml` 的 `[package] version`。
3. 修改 Cargo.toml 后，用 Cargo 同步锁文件，再核对 `src-tauri/Cargo.lock` 中 `name = "zhishu-ccs"` 对应的版本；不要手改锁文件或批量升级依赖：

```sh
cargo update --workspace --manifest-path src-tauri/Cargo.toml
```

4. 新增 `src/whats-new/0.0.2.json`。如无应用内公告，可使用下面的内容；要展示公告时遵循同目录 README 的多语言格式。

```json
{
  "version": "0.0.2",
  "items": []
}
```

保持应用 identifier、产品名、Windows 安装身份、更新地址和更新公钥稳定。不要为每次发布重新生成更新密钥，也不要把私钥或腾讯云凭据提交到 Git。

### 第二步：检查并提交到自己的仓库

在项目根目录执行：

```sh
pnpm typecheck
pnpm test:release
pnpm test:release-cos
git diff --check
git remote -v
```

确认 `zhishu-git` 指向 `git@github.com:zws-code/zhishu-ccs.git`。本机 `origin` 是上游，不能用它发布二开代码。`pnpm test` 会使用真实用户配置启动应用，不用于本步骤。

先用 `git add <具体源码文件>` 加入本次需要发布的代码，再加入版本文件和新公告。执行 `git diff --cached` 审查本次暂存内容后，再继续提交和推送：

```sh
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock src/whats-new/0.0.2.json
git diff --cached
git commit -m "chore(release): prepare 0.0.2"
git push zhishu-git HEAD:refs/heads/ccw-dev
```

如需先演练，可到 [Actions 发布工作流](https://github.com/zws-code/zhishu-ccs/actions/workflows/release.yml) 点击 **Run workflow**，选择 `ccw-dev`。**Run workflow 只演练，不会正式上传 COS，也不会创建 GitHub Release。**

### 第三步：推送正式 tag

确认代码已提交、版本一致，而且不存在同名正式 tag，再执行：

```sh
git tag v0.0.2
git push zhishu-git v0.0.2
```

推送 tag 才是正式发布开关。之后在 [Actions](https://github.com/zws-code/zhishu-ccs/actions) 打开 `v0.0.2` 对应运行，等待构建、COS 发布和 **Publish GitHub Release after COS** 全部成功。如环境显示 **Review deployments**，由已配置的审核者批准本次部署。

不要在工作流运行时取消任务、移动 tag 或重新上传同版本文件。首个 `v0.0.1` 曾有未成功的测试 tag，首次上线前做过一次修正；这不适用于以后已正式发布的 tag。

### 第四步：从三个位置验收

| 验收位置 | 应该看到什么 |
| --- | --- |
| [GitHub Releases](https://github.com/zws-code/zhishu-ccs/releases) | `智枢 CCS v0.0.2`，标记 Latest；下方 Assets 可直接下载 `.dmg`、`.msi`、Linux 包和签名 |
| 腾讯云 COS 控制台 | 打开 `zhishu-ccs-release-1252121424`，进入 `ccs/stable/v0.0.2/`，看到安装包和清单；`ccs/stable/` 下有 `latest.json`、`client-release.json` |
| 客户端 | 用正式安装版启动，启动时会检查更新；也可进入“设置 → 关于”检查更新，有新版时按提示安装并重启 |

公网更新清单为 [latest.json](https://zhishu-ccs-release-1252121424.cos.ap-guangzhou.myqcloud.com/ccs/stable/latest.json)，网页下载清单为 [client-release.json](https://zhishu-ccs-release-1252121424.cos.ap-guangzhou.myqcloud.com/ccs/stable/client-release.json)。清单中的版本应等于本次发布版本，下载链接应指向该版本目录。

第一次安装选择 macOS `.dmg` 或 Windows `.msi`。Windows 便携版使用手动下载方式，不将其当作安装版自动更新验收；Linux 使用相应包格式的更新行为。`0.0.1` 是首次正式版本，同版本检查应显示已是最新；完整升级验收需要后续 `0.0.2` 从正式 `0.0.1` 实际完成下载、验签、安装与重启。

当前 macOS 使用 `adhoc`，没有 Apple 公证；Tauri 更新签名仍启用。当前分发使用 COS 默认域名，CDN 尚未接入。

上传器同时传输最多四个独立文件，仍在全部文件完成、签名与公网检查通过后才切换稳定清单。日志中的 `Upload stream` 是数据流读取进度，`COS verified` 表示对象已写入并核对大小与哈希元数据。

GitHub 的海外构建机直传广州桶可能很慢；本次普通上传遇到了这一情况。若希望以后每次正式 tag 发布也加速，可自行在 Repository Variables 设置 `COS_UPLOAD_ACCELERATE=true`；不设置则默认 `false`。上传才使用 `cos.accelerate.myqcloud.com`，桶检查与客户端更新/下载地址继续使用原域名。此选项需要桶已开启全球加速，并会额外按流量计费；当前跨境上行公布价为 1.25 元/GB，实际以[腾讯云控制台与定价](https://buy.cloud.tencent.com.cn/price/cos)为准。

也可只在下面的恢复工作流中勾选一次 `accelerate`，不改变后续发布默认行为。当前实现对每次加速运行限制累计上传读取量 3 GB（包含 SDK 重试）；触及上限时终止，不切换清单。这个保护限制本次上传的数据量，不是整个腾讯云账户的账单限额。

### 第五步：失败时如何处理

- 打开 Actions 中失败的作业和红色步骤，先保存错误信息；不要反复点击发布，也不要重建同版本覆盖。
- 配置或编译阶段失败、还没上传 COS：修好后根据失败原因处理。不要把后续新提交误当成旧 tag 自动包含；重跑旧运行仍使用原提交。
- COS 上传或清单切换阶段失败：保留该次构建产物，使用 **Re-run failed jobs** 复用原包；凭据/权限改在 GitHub Settings 或腾讯云 CAM，不在聊天里发送密钥。
- 只有 GitHub Release 失败：使用 **Re-run failed jobs**，只补发 Release，COS 已上线的文件保持原样。
- 代码需修复、同版本已预留或上传：发布更高版本，不能移动已发布 tag 或覆盖桶内版本文件。

### 上传脚本修复后：用原包恢复发布

如果正式 tag 的安装包已构建成功并保存，但需要修复上传脚本，重跑旧的正式工作流仍使用旧脚本。此时到 [Resume verified COS release](https://github.com/zws-code/zhishu-ccs/actions/workflows/resume-cos-release.yml)，点击 **Run workflow**，选择已修复脚本所在的 `ccw-dev` 分支，填写原正式构建的 **run_id**。桶全球加速已开启且愿意承担本次流量费用时，可勾选 `accelerate`；默认不勾选。

run_id 是原 Actions 页面地址末尾的数字。例如本次原正式运行是 `40825474319`，网址为 `https://github.com/zws-code/zhishu-ccs/actions/runs/40825474319`。恢复入口会核对原工作流、正式 tag、原提交和未过期的 `cos-payload-v<版本>`，再次验证签名与哈希后实际上传 COS，再创建 GitHub Release。它不编译、不移动 tag、不生成另一套同版本包；原产物已过期时不能使用，应发新版本。

注意：**这个恢复工作流会实际发布**，与 `Release to Tencent COS` 的手动演练不同。后台也可推送专用分支 `codex/cos-resume` 触发恢复，但提交说明必须带单独一行 `Resume-Release-Run: <原正式run_id>`；本次需要加速时再加单独一行 `Resume-Release-Accelerate: true`。日常优先使用上述页面输入方式。

更详细的首次云端配置、权限与签名说明见下文。

## 1. 准备腾讯云 COS 和 CDN

### 当前接入：COS 默认域名直连

当前无自有域名，先使用上述 COS 默认 HTTPS 地址，`COS_PREFIX=ccs/stable`，`ZHISHU_UPDATE_URL` 为下载根地址加 `/latest.json`。此模式下桶使用公有读私有写，让客户端匿名下载公开安装包和清单；私钥和云凭据只保存到 GitHub Secrets，不上传到桶。

默认域名的强制下载响应头不表示禁止程序读取 JSON；仍需实际验证更新清单和安装包下载。这个阶段没有 CDN 缓存/加速，不把 COS 直连描述为 CDN 已接入。

### 后续接入：自有域名与 CDN

以下步骤用于准备 CDN；当前直连模式不执行改回私有桶，避免客户端无法下载。

1. 在腾讯云 COS 控制台创建**专用普通存储桶**，选择实际使用的地域，例如 `ap-guangzhou`。记录完整桶名（含 APPID 后缀）和地域。
2. 桶使用私有读写、标准存储；**不启用 COS 版本控制**。发布脚本使用不可变的版本目录和禁止覆盖写入保护历史产物，启用桶版本控制会让 COS 的禁止覆盖头失效，因此脚本会拒绝此类桶。
3. 准备下载域名，例如 `download.你的域名`。在 CDN 控制台接入域名，以该 COS 桶为源站，开启 CDN 服务授权和私有桶回源鉴权；配置 CNAME 和有效 HTTPS 证书。
4. 本发布前缀保存公开安装包，用户访问 CDN 时应允许匿名 GET/HEAD；不要使用会过期的临时签名下载地址、登录门禁或依赖 Referer 的防盗链规则。源站 COS 保持私有。
5. CDN 缓存配置：`/ccs/stable/v*/` 为长期缓存；`/ccs/stable/latest.json` 和 `/ccs/stable/client-release.json` 的有效缓存时间最多 60 秒。不要为 404 设置长时间负缓存。发布器会等待短缓存过期并核对公开清单，不依赖刷新 API 权限。
6. 保留历史版本，不配置会自动删除正式版本的生命周期规则。

中国境内/全球 CDN 接入域名需要完成 ICP 备案；境外加速要求不同，以[腾讯云域名接入要求](https://intl.cloud.tencent.com/zh/document/product/228/31476?lang=zh)为准。[COS/CDN 回源授权说明](https://cloud.tencent.com/document/product/436/18670)。没有自有域名时可先使用上述 COS 默认 HTTPS 地址；只有直连下载与签名验收通过后才推正式 tag。

对象路径和下载 URL 一一对应，不配置 CDN 路径重写：

```text
COS 桶
└── ccs/stable/
    ├── latest.json                  # Tauri 自动更新清单
    ├── client-release.json          # 网页下载清单
    └── v1.0.0/                      # 版本路径不可覆盖
        ├── release.json             # 本版本文件大小、SHA-256 与输入记录
        ├── latest.json
        ├── client-release.json
        ├── Zhishu-Ccs-v1.0.0-macOS.dmg
        ├── Zhishu-Ccs-v1.0.0-macOS.tar.gz
        ├── Zhishu-Ccs-v1.0.0-macOS.tar.gz.sig
        └── Windows/Linux 安装包与签名
```

`COS_PUBLIC_BASE_URL` 例如 `https://download.你的域名/ccs/stable`；其 URL 路径必须与 `COS_PREFIX` 一致。以上域名与版本仅是示例，不能直接当成已配置的地址。

## 2. 创建发布凭证

在腾讯云 CAM 创建专用发布子账户或凭证，仅授权目标桶。桶级需要 `cos:GetBucketVersioning`；对象级限定到 `ccs/stable/*`，允许 `cos:HeadObject`、`cos:GetObject`、`cos:PutObject`。本实现使用最多四路并发的单文件 PUT，不需要分块上传、删除对象、修改 ACL 或管理其他云资源的权限；单个文件超过 5 GiB 时拒绝发布。

将 `SecretId`、`SecretKey` 直接填写到 GitHub Secrets。不要提交到 Git、应用配置或聊天，不使用腾讯云主账户密钥。若使用临时凭证，还需设置 `COS_SESSION_TOKEN`，并确保发布期间凭证有效。

## 3. 配置 GitHub Variables 和 Secrets

打开仓库 `Settings → Secrets and variables → Actions`。**共享构建配置和更新签名凭据放 Repository 级**，因为预检和多平台作业都需要读取。

### Repository Variables

| 名称 | 内容 |
| --- | --- |
| `ZHISHU_PLATFORM_API_URL` | 正式平台 API origin，不含 `/api/v1` 或其他路径 |
| `ZHISHU_PLATFORM_WEB_URL` | 正式平台网页 origin |
| `ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP` | `true` 或 `false`；平台仍是 HTTP 时显式设为 `true` |
| `ZHISHU_UPDATE_URL` | `https://下载域名/ccs/stable/latest.json` |
| `ZHISHU_UPDATE_PUBLIC_KEY` | 更新公钥 `.pub` 文件的完整 base64 内容 |
| `COS_BUCKET` | 完整桶名，例如 `zhishu-downloads-1234567890` |
| `COS_REGION` | 实际地域，例如 `ap-guangzhou` |
| `COS_UPLOAD_ACCELERATE` | 可选，默认 `false`；显式 `true` 仅加速上传，会产生额外流量费，须先开启桶全球加速 |
| `COS_PREFIX` | `ccs/stable`，未设置也使用此值 |
| `COS_PUBLIC_BASE_URL` | `https://下载域名/ccs/stable`，不带末尾斜杠 |
| `ZHISHU_MACOS_SIGNING_MODE` | 默认 `developer-id`；显式设置 `adhoc` 可构建未公证包 |

COS/CDN 更新地址强制 HTTPS。平台 API 的 HTTP 开关不影响更新地址，不应把更新源也降级为 HTTP。

### Secrets

| 名称 | 内容 |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | 更新私钥文件的完整内容；也兼容上游 helper 支持的格式 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 更新私钥口令；无口令可不设置 |
| `COS_SECRET_ID` | 腾讯云发布凭证 SecretId |
| `COS_SECRET_KEY` | 腾讯云发布凭证 SecretKey |
| `COS_SESSION_TOKEN` | 可选，仅临时凭证需要 |

COS 凭据建议放在 `cos-production` Environment Secrets，构建作业无需读取；也支持 Repository Secrets。预检阶段不连接腾讯云，真正发布前上传器会检查凭据与权限。

现有密钥位置为 `~/.tauri/zhishu-ccs.key` 和 `.pub`，可以用于这次首次正式发布。若决定为正式版新建身份，仅在首次发布前执行一次：

```sh
pnpm tauri signer generate -w ~/.tauri/zhishu-ccs-production.key
```

妥善备份正式私钥、公钥及口令。正式版发布后不得随意替换公钥；更新签名私钥不是 Apple 代码签名证书。

### macOS Developer ID 签名与公证

`developer-id` 模式需要以下 Repository Secrets：

| 名称 | 内容 |
| --- | --- |
| `APPLE_CERTIFICATE` | Developer ID Application 的 `.p12` 证书 base64 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` 导出口令 |
| `KEYCHAIN_PASSWORD` | CI 临时 keychain 使用的密码 |
| `APPLE_ID` | 公证使用的 Apple ID |
| `APPLE_PASSWORD` | Apple App 专用密码 |
| `APPLE_TEAM_ID` | Apple Developer Team ID |

默认缺少证书/公证凭据会在编译前失败。没有证书时可以明确选择 `adhoc`：保留 Tauri 更新签名与 ad-hoc 代码签名，跳过 Apple 公证和 Gatekeeper 公证验收；首次下载可能被系统拦截，不得宣称已公证。面向公众正式分发优先完成 Developer ID 证书配置。

Windows 更新包始终有 Tauri 更新签名；这不表示已经配置 Windows 系统代码签名或消除 SmartScreen 提示。首次正式发布前确认 Tauri 的 productName、identifier、协议、安装身份，后续保持一致。

### 发布审批

在 `Settings → Environments` 创建 `cos-production`。如仓库套餐支持，可设置 Required reviewers 和 tag 部署规则。这样推 tag 会自动构建，验证通过后由批准者决定何时切换稳定更新。

GitHub Release 使用工作流自动提供的 `GITHUB_TOKEN`，无需新增个人访问令牌；仅独立 Release 作业申请 `contents: write`，构建与 COS 作业继续使用只读仓库权限。如组织策略限制令牌写入，需要仓库管理员允许该权限。

手动演练使用 `release-validation` 环境，不上传 COS、不创建 GitHub Release；已有构建作业继续使用 `release` 环境。不要只把共享变量或签名私钥放在 `cos-production`，构建阶段无法读取它们。

## 4. 首次演练

先把源码与工作流推到自己的 GitHub 仓库。`origin` 仍可能指向上游，**不要用默认 origin 推送二开代码**：

```sh
git remote -v
# 本机已存在 zhishu-git → git@github.com:zws-code/zhishu-ccs.git
git push zhishu-git <你的发布分支>
```

版本必须在 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 保持一致；Rust 构建会更新 Cargo.lock，提交版本变更时也应同步核对。新增 `src/whats-new/<版本>.json`，无应用内公告时使用真实版本与 `items: []`。

也可以通过专用分支启动相同演练，无需操作浏览器：

```sh
git push zhishu-git HEAD:refs/heads/codex/cos-validation
```

该分支推送使用 `release-validation` 环境；COS 上传和 SDK 安装步骤仍要求 `push` 事件且 `ref_type=tag`，分支推送不会发布 COS 或创建 GitHub Release。只在需要演练时推送该专用分支。

在 GitHub `Actions → Release to Tencent COS → Run workflow` 选择该分支。手动运行会完成配置检查、多平台构建、签名校验和本地发布预演，产物保存为 Actions artifact；**不会上传 COS、不会替换正式清单、不会创建 GitHub Release**。

支持的构建产物：

| 构建机 | 产物 |
| --- | --- |
| macOS | Intel/Apple Silicon universal DMG、ZIP、签名 tar.gz 更新包 |
| Windows x64 / ARM64 | MSI、签名及继承的便携包 |
| Linux x64 / ARM64 | AppImage、DEB、RPM 和各自签名 |

工作流使用 `package.json` 指定的 pnpm 和 `rust-toolchain.toml` 指定的 Rust，不在本机模拟其他系统已完成构建。

## 5. 正式发布

1. 更新并提交一致的版本号、真实变更说明与待发布代码。
2. 在已推送的提交上创建新的稳定版本 tag，仅接受 `vX.Y.Z`。
3. 推送 tag 到自己的仓库：

```sh
git tag v<新版本号>
git push zhishu-git v<新版本号>
```

工作流会：

1. 检查 tag、三个源码版本、应用内公告及发布配置。
2. 构建全部平台，并按选择完成 Apple 签名/公证。
3. 验证安装包与每个更新包的 minisign 签名；生成 CDN URL、真实文件大小、SHA-256 和两份不同用途的清单。
4. 保存已验证的 `cos-payload-v<版本>` artifact，供失败后使用原产物重试。
5. 检查 COS 旧版清单，拒绝降版本、漏已发布平台和同版本不同内容。
6. 先预留不可变版本输入，再上传版本目录，检查 COS 对象和 CDN 公网文件。
7. 公网检查通过后，依次发布根目录 `client-release.json`、`latest.json`，读回校验并等待 CDN 清单与本地一致。
8. COS 作业成功后，独立作业下载同一份 `cos-payload-v<版本>` artifact，创建正式、非预发布的 GitHub Release，设为 Latest，并附上全部已验证安装包、签名及清单。在仓库右侧 **Releases** 或工作流摘要中的下载链接进入，在版本页面展开 **Assets** 选择安装包。macOS 发布说明按实际签名模式生成，不把 `adhoc` 称为 Apple 公证。

两个 COS 对象不能作为一个事务同时替换。若第二份清单发布失败，网站可能先看到新版，但客户端仍使用旧更新清单；此时应重试同一批原产物，不自动回滚已验证的安装包。正式客户端的更新入口最后切换。

发布阶段在仓库内串行执行，并在切换前再次检查线上版本；只允许这一个工作流写该发布前缀，不同时从其他仓库或本机修改稳定清单。

## 6. 失败与重试

| 情况 | 处理 |
| --- | --- |
| 缺少配置/版本不一致 | 修好 Variables、版本和摘要后先手动演练 |
| 缺少 Apple 凭据 | 配齐 Developer ID 凭据；无证书需显式选择 `adhoc` 并接受未公证分发限制 |
| COS 权限失败 | 检查桶名、地域、凭据和 CAM 对应前缀权限；不要给主账户全权限兜底 |
| 开启 COS 版本控制 | 使用未开启版本控制的专用普通桶 |
| CDN 403/404 或大小不符 | 检查回源授权、CNAME、HTTPS、匿名访问和负缓存；未验证前不会切换更新清单 |
| 清单 CDN 验证失败 | 检查清单 TTL 是否超过 60 秒；修复后重试，不要重编译覆盖原版本 |
| 同版本输入不同 | 若原版本已预留/上传，使用更高版本，不能覆盖历史产物 |
| 旧版本晚于新版构建完成 | 拒绝把稳定源降回旧版本；使用更高补丁版本修复 |

GitHub Release 失败不会回滚已经发布的 COS 更新源。如果只有 GitHub Release 作业失败，在 Actions 使用 **Re-run failed jobs**，只重试 Release 作业，继续下载原 COS payload，不重编译或再次上传 COS。已存在的同名 GitHub 附件不覆盖；缺少的附件继续上传。

发布失败时在 Actions 使用 **Re-run failed jobs**，复用原构建 artifacts。清单时间取提交时间，重试不会因当前时间改变生成不同的输入。不要重跑全部构建后用不同安装包覆盖已预留的版本；超过 artifact 保存期限时优先发新版本。

本地只读验证与隔离回归：

```sh
pnpm release:check                 # 需已设置相关环境变量；只校验配置，不连接腾讯云
pnpm test:release                  # Node 配置、签名与产物回归
pnpm test:release-cos              # Python 上传/失败/重试回归，无需安装 COS SDK
python3 -B scripts/publish-cos.py <已验证的cos-payload目录> --dry-run
```

## 打包命令

构建配置仍由 `release.config.local.json` 或现有环境变量读取，`release:prepare` 与底层 `build`/`build:unsigned`/`build:editable` 保留用于开发和内部验证，不是正式发布入口。正式 GitHub 构建使用 Repository Variables，不提交本机配置。

| 命令 | 更新签名 | 用途 |
| --- | --- | --- |
| `pnpm build` | 开启，需私钥 | 本机正式配置验证，不上传产物 |
| `pnpm build:unsigned` | 关闭 | 内部测试 |
| `pnpm build:editable` | 关闭 | 可编辑服务器地址的内部测试 |
| `pnpm release:prepare` | 仅生成配置 | 校验配置，不编译 |

底层参数由包装脚本传给 Tauri，例如 `pnpm build -- --target universal-apple-darwin --bundles app,dmg --ci`。不得额外传 `--config` 覆盖发布策略。默认目标为本机架构，默认 bundle 来自 `tauri.conf.json`；Windows/Linux 安装包需对应系统构建。产物在 `src-tauri/target/[<target>/]release/bundle/`，设置 `CARGO_TARGET_DIR` 时以该目录为准。

配置环境变量沿用 `VITE_PLATFORM_API_URL`、`VITE_PLATFORM_WEB_URL`、`ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP`、`ZHISHU_UPDATE_URL`、`ZHISHU_UPDATE_PUBLIC_KEY`、`ZHISHU_UPDATE_ALLOW_INSECURE_HTTP`。私钥只通过 `TAURI_SIGNING_PRIVATE_KEY` 和可选的 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 注入，不写入应用配置。

## 7. 官网下载页和安装验收

Tauri 客户端读取当前下载源的 `latest.json`；目前为 COS 默认域名，接入 CDN 后使用 CDN 域名。网页下载列表读取的是 `client-release.json`，两者不能互换。

若继续使用智枢平台原有 `/download` 页面，需要让平台网关的 `/client-release.json` 转发/读取新的 CDN 下载清单；这是平台部署配置的单独接入项，本客户端工作流不连接 SSH、不改网关、不重启平台。未完成这项接入时，先使用本次 artifact/版本清单里的实际 CDN 安装包 URL 进行安装验收，不能声称旧下载页已接通。

首次正式包手动安装验收 macOS、Windows、Linux 的安装、启动、登录和工具接入。以后用两个真实正式版本验证发现更新、下载、验签、安装和重启。脚本和模拟回归通过不代表跨平台安装、Apple 公证、真实模型请求或计费已验收。

参考：[Tauri Updater](https://tauri.app/plugin/updater/)、[腾讯云 PUT Object 禁止覆盖规则](https://intl.cloud.tencent.com/document/product/436/7749?lang=en)、[COS Python SDK](https://cloud.tencent.com/document/product/436/12269)。
