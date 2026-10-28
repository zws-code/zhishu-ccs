# 智枢客户端下载、打包、发布与更新

这是客户端发布的唯一操作文档。所有命令由你执行；客户端发布脚本只上传安装包和清单，首次网关接入单独处理。

## 日常发布：只用两条命令

在客户端仓库执行，把版本号改为本次的新版本：

```sh
cd /Users/wenszou/IdeaProjects/zhishu/zhishu-cc-switch
pnpm release:build 4.0.20
pnpm release:publish
```

第一条自动同步版本号、打包和签名；第二条使用刚才打包的版本，上传安装包并更新网页下载、客户端自动更新清单。macOS 默认生成 Intel/Apple Silicon 通用包。

首次使用先完成下面的准备。已经上线的版本不可覆盖，改代码后使用新版本号；同一批产物发布失败时可以重试。

发布前可用 `pnpm release:publish --dry-run` 做本地预演，不连接服务器；它不会执行真正发布。`pnpm release:help` 显示各命令用途。

## 首次准备（一次即可）

安装项目指定的 Node.js、pnpm、Rust 与系统构建依赖，然后：

```sh
pnpm install --frozen-lockfile
pnpm release:init
```

`release:init` 创建本机配置和专用更新密钥，不连接服务器。这台 Mac 已完成初始化，换电脑要使用原密钥与公钥，不要重新生成另一套更新身份。

**备份 `~/.tauri/zhishu-ccs.key` 和 `.pub`。私钥不提交 Git、不上传服务器。** 默认私钥不带口令、权限 600；使用带口令密钥时，在打包进程设置 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。

macOS/Linux 的发布回归会调用 Python 3；Windows 运行跨平台部分，服务器事务回归由 POSIX 主机验证。Windows 需要 VS C++ 构建工具、WebView2；Linux 需要 GTK/WebKit2GTK、AppImage、rpm 等依赖。

## 首次平台接入（单独操作）

先只读检查：

```sh
pnpm release:check
```

如果下载目录已就绪，不要为了客户端重新部署平台。准备发布工具：

```sh
pnpm release:server
```

`release:server` 只创建远端目录、上传客户端发布工具，不部署或重启网关。SSH 使用 BatchMode 和已有 known_hosts；先人工核对首次连接指纹，带口令 SSH 私钥需加入 ssh-agent。

只有 `release:check` 不通过且确认网关缺少 `/client-downloads/` 支持时，才需要人工检查现有下载支持改动，在测试环境验收后按平台流程升级一次。客户端发布脚本不会替你升级平台。平台仍有白屏或其他访问问题时，先处理平台问题，不继续发布客户端。

平台自己的部署命令（会构建、备份并重启网关；由你单独执行）：

```sh
cd /Users/wenszou/IdeaProjects/zhishu/zhishu-proxy
bash deploy/rocky10/deploy.sh "client-downloads-$(date +%Y%m%d-%H%M%S)"
```

这个入口自行构建，不要先 `build.sh` 再用同一个 ID 执行 `deploy.sh`。升级后用浏览器验证登录、控制台和 `/download`，不能只看 `/health`。后台模式下当前下载页可能要求先登录。

平台异常时停止客户端发布，使用该次部署输出中的备份 ID：

```sh
ssh -i /Users/wenszou/IdeaProjects/zhishu/zhishu-proxy/veee-rocky-ed25519 root@43.136.64.205 \
  'bash /opt/zhishu-proxy/rocky10/rollback.sh <该次部署输出的备份ID>'
```

平台验收通过后回到客户端仓库，执行 `pnpm release:server` 和 `pnpm release:check`，再按开头两步发布。日常客户端发布无需再次部署网关。

## 本机配置与地址

| 项目 | 地址/位置 |
| --- | --- |
| 平台与下载页 | `http://43.136.64.205/download` |
| 安装包根地址 | `http://43.136.64.205/client-downloads` |
| 客户端自动更新清单 | `http://43.136.64.205/client-downloads/latest.json` |
| 网页读取的下载清单 | `http://43.136.64.205/client-release.json` |
| SSH 服务器 | `root@43.136.64.205` |
| 服务器发布目录 | `/opt/zhishu-proxy/data/public/client-downloads` |
| 平台清单覆盖文件 | `/opt/zhishu-proxy/data/public/client-release.json` |
| 本机发布配置 | `release.publish.local.json` |
| 本机应用配置 | `release.config.local.json` |
| 本机更新签名私钥 | `~/.tauri/zhishu-ccs.key` |

下载使用服务器默认 HTTP 端口，不需要额外域名、HTTPS 证书、GitHub 仓库或单独开放下载端口。服务器只需要现有平台、SSH 和 Python 3；日常客户端发布不重启网关，也不构建网关镜像。

HTTP 不加密网络流量，登录密码仍有明文传输风险。更新包始终由独立私钥签名，客户端先验签再安装；这不能替代 HTTPS 对登录和网络传输的保护。以后有域名时按本文末尾迁移。

## Windows / Linux 打包与汇总

在对应系统上检出相同源码、配置相同的生产地址和原签名密钥，使用项目指定 pnpm 版本安装依赖。然后在该系统的终端运行：

```sh
# Windows x64
pnpm release:build 4.0.21 --target windows-x64
# Windows ARM64
pnpm release:build 4.0.21 --target windows-arm64
# Linux x64
pnpm release:build 4.0.21 --target linux-x64
# Linux ARM64
pnpm release:build 4.0.21 --target linux-arm64
```

Mac 不会生成 Windows/Linux 原生安装包；这两类入口已实现，真实打包待对应机器验证。Windows 需要 C++ 构建工具和 WebView2，Linux 需要 GTK/WebKit2GTK 等依赖。

多平台统一发布：把各机器的 `release/v4.0.21/<目标>/` 目录完整复制到发布机（包括 `build.json`），然后执行：

```sh
pnpm release:import 4.0.21 /你复制过来的目录/windows-x64
pnpm release:import 4.0.21 /你复制过来的目录/linux-x64
pnpm release:publish 4.0.21
```

同一下载源一旦上线过多种平台，以后每次都要把这些平台的产物收齐再发布，脚本会阻止漏发已有平台。

## 配置和其他命令

| 文件/命令 | 用途 |
| --- | --- |
| `release.publish.local.json` | 服务器、SSH 私钥路径、下载地址、远端静态目录 |
| `release.config.local.json` | 客户端生产 API/Web 地址、更新地址、公钥与 HTTP 开关 |
| `~/.tauri/zhishu-ccs.key`、`.pub` | 本机更新身份；备份原件，不上传私钥、不随意换密钥 |
| `pnpm release:help` | 显示命令用途 |
| `pnpm test:release` | 本地发布回归，不连接生产服务器 |

## 打包具体做什么

`pnpm release:build [版本号] [--target 目标]`：

1. 校验平台地址、HTTP 开关、稳定更新地址和专用签名密钥。
2. 同步 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 的版本。原文件备份在 `release/.source-backups/`；Cargo 构建自动更新 Cargo.lock。缺少新版应用内摘要时创建 `src/whats-new/<版本>.json`，默认 `items: []`，不会编造功能说明。
3. 运行 TypeScript 和发布脚本回归，调用原有正式 Tauri 构建。
4. 输出安装包、更新包和 `.sig`，直接在构建目录验签后记录 `build.json`，包含生产配置指纹、文件大小和 SHA-256；验签无需额外复制安装包。

macOS 未配置 `APPLE_SIGNING_IDENTITY` 时，脚本自动做 ad-hoc 签名，再重新压缩并签署更新包；配置了 Apple 证书时保留正式签名。

构建会保留旧 bundle 为 `bundle.previous-时间戳`，避免把旧安装包混进新版本。成功后可自行清理这些旧构建目录。

| 目标 | 在哪种机器执行 | 默认产物 |
| --- | --- | --- |
| `macos` | macOS | universal DMG、ZIP、签名 `.tar.gz` 更新包 |
| `windows-x64` | Windows（VS C++ 构建工具、WebView2） | x64 MSI 及签名 |
| `windows-arm64` | Windows（安装 ARM64 C++ 工具链/对应 Rust target） | ARM64 MSI 及签名 |
| `linux-x64` | x64 Linux（GTK/WebKit2GTK、AppImage、rpm 工具） | AppImage、DEB、RPM 及签名 |
| `linux-arm64` | ARM64 Linux（同上） | ARM64 AppImage、DEB、RPM 及签名 |

不指定目标时根据本机系统和架构选择，macOS 始终选择 universal。Windows/Linux 安装依赖可参考已有 `.github/workflows/release.yml` 和 Tauri [构建前置条件](https://tauri.app/start/prerequisites/)。

macOS 命令不会把 Windows、Linux 模拟成已完成。现有 GitHub Actions 可以提供对应构建机，但两个仓库当前 origin 指向上游，脚本不推送上游、不创建上游 Release。需要云端构建时，先配置自己的仓库及 CI 密钥。

macOS updater 签名与 Apple Developer ID 签名是两件事。没有 Apple Developer ID/公证配置时，产物可用于内部测试，首次下载可能被 Gatekeeper 拦截；不能声称已经 Apple 公证。正式面向公众外发时按 Tauri [macOS 签名文档](https://tauri.app/distribute/sign/macos/)设置证书和公证凭据后重新打包。

原有 `pnpm build:unsigned`、`pnpm build:editable` 继续用于内测，但关闭了自动更新，不能通过此正式发布流程上线。已安装这两种内测包的机器，首次需要手动安装一次本流程的签名正式包。

## 发布具体做什么

`pnpm release:publish [版本号] [--dry-run]`：

1. 校验构建记录、生产地址与公钥一致性、所有文件 SHA-256 和更新签名。
2. 汇总到 `release/v<版本>/publish/`，自动生成网页 `client-release.json` 和 Tauri `latest.json`，两者格式不同，不用手写。
3. 先探测网关下载目录，旧网关返回 SPA 页面时会在上传前失败；通过后上传到服务器 `.staging/`；服务器重新校验全部文件，随后以不可变版本目录保存。只上传安装包和公开元数据，不上传源码、SSH/更新私钥或本机配置。
4. 从公网逐个检查安装包可下载且大小正确；这一步失败时保留旧版清单。
5. 使用发布锁，检查未降版本、未漏已支持平台；原子切换 `current`，替换平台下载清单。
6. 从公网核对两份更新/下载清单和平台清单与本地完全一致，成功后打印下载页地址。

缓存中的 `publish/` 用于失败重试，因此同一批文件重试不会刷新发布时间或覆盖不同安装包。如果打包产物变了但尚未上线，先把原 `publish/` 和 `publish-inputs.json` 移到别处再汇总。已经上线就必须增加版本号。

服务器目录：

截至 2026-10-25，线上 `current` 指向 `versions/v4.0.19`。下方 `v4.0.20` 是目录结构示例；实际线上版本以 `current` 和 `pnpm release:check` 为准。

```text
/opt/zhishu-proxy/data/public/
  client-release.json                 # 网页下载清单
  client-downloads/
    tools/server.py                   # 只通过 SSH 执行，不对外开放
    .staging/                         # 上传暂存，不对外开放
    .publish.lock
    current -> versions/v4.0.20        # 两份公共清单使用同一发布快照
    versions/v4.0.20/                  # 不可变安装包、签名、两份清单和校验记录
```

网关仅开放两份公共清单与版本目录中的安装包，不开放目录列表、工具、staging、任意路径或越界符号链接。安装包支持 HEAD/断点范围请求；版本文件长期缓存，最新清单不缓存。

## 常见失败

| 提示 | 处理 |
| --- | --- |
| SSH host key verification failed | 人工核对服务器指纹后加入 known_hosts，不关闭校验 |
| Permission denied / BatchMode | 检查 `sshKey`、SSH 用户、私钥权限与 ssh-agent |
| 缺少更新私钥 / 公钥不匹配 | 找回原密钥与 `.pub`，不随意更换 |
| 同一版本已打包 / 不可覆盖 | 新代码使用新版本号；未完成的本地残留先移走 |
| 线上 URL 是网页 HTML | 网关未包含 `/client-downloads/` 支持，先部署一次新版网关 |
| 缺少以前已发布的平台 | 在对应系统打包并 import 后再发布 |
| 公网验证失败 | 检查平台 HTTP 服务、文件挂载/权限和路由；旧清单保持不变，修好后重试 |
| 页面显示暂未发布，但更新清单已有版本 | 检查 `/client-release.json` 的 `files` 是否为空；用原产物重新执行 `pnpm release:publish <版本>`，无需重新打包 |
| 自动更新提示未配置 | 当前是 unsigned/editable 老包，手动安装一次新的签名正式包 |

清理 `.staging/` 时先确认没有正在上传的发布，只清理确认失败的目录。版本目录不自动删除，以免破坏下载和仍在进行中的更新。不能通过重新发布旧版本给客户端自动降级；修复问题后发布更高补丁版本。

## 迁移到 HTTPS

先配置域名和反向代理证书，保留旧 HTTP 更新入口可用。修改本机发布配置的 `downloadBaseUrl`，关闭 `allowInsecureHttp`；修改应用配置的平台 API/Web 地址并关闭 `platformAllowInsecureHttp`，再执行 `release:init` 同步更新地址与开关。保留同一套签名密钥，发布更高版本。

在所有旧客户端迁移前保留旧 HTTP 更新入口，并让它返回指向新 HTTPS 安装包的有效签名清单。新包会内置 HTTPS 更新地址。网页切换 HTTPS 后拒绝 HTTP 安装包链接，必须一起切换清单地址。

## 验证边界

发布回归：`pnpm test:release`。真实上线还要在目标系统验证安装、平台登录、工具接入和从旧签名版本升级到新签名版本。脚本验签和公网文件验证不代替桌面安装/重启验证，也不代替真实模型请求和计费验收。具体本次执行结果记录在平台仓库 `docs/zhishu/客户端发布与接入.md`。

macOS 产物可用 `lipo -info` 检查双架构、`codesign --verify --deep --strict` 检查应用签名、`hdiutil verify` 检查 DMG。各目标系统全新安装还需验证协议注册、2FA 和自动同步。平台开启人机验证时，当前密码登录会明确拒绝；浏览器授权仍是后续需求。

更新配置依据 Tauri [官方 updater 文档](https://tauri.app/plugin/updater/)：HTTP 使用明确的传输开关，签名验证始终保留。

<details>
<summary>底层构建参数与 GitHub/R2 配置（可选）</summary>

## 本地生产配置

手动配置或 CI 使用时，复制 `release.config.example.json` 为 `release.config.local.json`：

| 配置项                      | 默认值与要求                                                | 覆盖环境变量                          |
| --------------------------- | ----------------------------------------------------------- | ------------------------------------- |
| `platformApiUrl`            | 所有构建必填；仅 origin，不含 `/api/v1`、账号密码或查询参数 | `VITE_PLATFORM_API_URL`               |
| `platformWebUrl`            | 所有构建必填；仅 origin                                     | `VITE_PLATFORM_WEB_URL`               |
| `platformAllowInsecureHttp` | 默认 `false`；临时 HTTP 平台需明确开启                      | `ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP` |
| `updateUrl`                 | 正式包必填；自己的稳定更新清单地址                          | `ZHISHU_UPDATE_URL`                   |
| `updatePublicKey`           | 正式包必填；原更新身份 `.pub` 文件的完整 base64 内容        | `ZHISHU_UPDATE_PUBLIC_KEY`            |
| `updateAllowInsecureHttp`   | 默认 `false`；HTTP 更新源需单独开启，仍校验签名             | `ZHISHU_UPDATE_ALLOW_INSECURE_HTTP`   |

环境变量覆盖本地 JSON。私钥通过 `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 或 CI Secret 提供，不写入应用配置。

## 打包命令

底层构建与内测入口如下；日常发布仍使用文档开头的 `release:build` 和 `release:publish`。

| 命令                  | 服务器地址                 | 自动更新 / updater 产物  | 用途                 |
| --------------------- | -------------------------- | ------------------------ | -------------------- |
| `pnpm build`          | 登录页可修改，配置为默认值 | 开启，需要更新签名密钥   | 底层正式构建         |
| `pnpm build:unsigned` | 登录页可修改，配置为默认值 | 关闭，无需更新配置或密钥 | 无更新签名的内测包   |
| `pnpm build:editable` | 登录页可修改，配置为默认值 | 关闭，无需更新配置或密钥 | 可切换服务器的内测包 |

三个模式均校验平台 API/Web 地址，并把配置地址作为登录页默认值；用户可以修改服务器地址。构建脚本生成忽略文件 `src/.env.production.local` 与 `src-tauri/tauri.release.local.conf.json`，再调用 Tauri。未配置更新的开发包跳过自动检查，手动检查会提示未配置。

辅助入口：`pnpm release:prepare` 仅校验和生成配置；`pnpm test:release` 运行完整发布回归；`pnpm test:release-config` 仅运行配置和网页下载清单测试；`pnpm build:renderer --mode development` 仅构建前端。

包装脚本接受 `--unsigned`、`--editable`，移除 pnpm 分隔符 `--`，其他参数传给 `tauri build`。禁止额外传入 `-c` / `--config` 覆盖发布策略。

| Tauri 参数          | 默认行为与说明                                                                                      |
| ------------------- | --------------------------------------------------------------------------------------------------- |
| `--target <triple>` | 当前机器架构；支持 `universal-apple-darwin`、`x86_64-pc-windows-msvc`、`aarch64-pc-windows-msvc` 等 |
| `--bundles <列表>`  | 默认读取 `bundle.targets: "all"`；可指定 `app,dmg`、`msi`、`appimage,deb,rpm`                       |
| `--debug`           | 默认 release；debug 仅用于排错                                                                      |
| `--no-bundle`       | 仅编译可执行文件                                                                                    |
| `--features <列表>` | 启用 Cargo features                                                                                 |
| `--no-sign`         | 跳过平台代码签名，与 updater 签名分开                                                               |
| `--ci`              | 非交互模式                                                                                          |

完整参数以 `pnpm tauri build --help` 为准。示例：

```sh
# macOS universal 内测包；需要预先安装两个 Rust target
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm build:editable -- --target universal-apple-darwin --bundles app
# 在 Windows 构建 ARM64 内测包；x64 使用 x86_64-pc-windows-msvc
pnpm build:unsigned -- --target aarch64-pc-windows-msvc --bundles msi
# 在 Linux 构建内测包
pnpm build:unsigned -- --bundles appimage,deb,rpm
```

底层 Tauri 产物位于 `src-tauri/target/[<target>/]release/bundle/`，设置 `CARGO_TARGET_DIR` 时以该目录为准。正式 CLI 整理后的产物位于 `release/v<版本>/<目标>/`。macOS 无 Finder 环境下，日常发布 CLI 已使用 `hdiutil` 封装 DMG，无需另跑一套手工打包步骤。

## GitHub Actions 与签名

`release:init` 已初始化的机器沿用原密钥。手工配置新发布身份时只生成一次：

```sh
pnpm tauri signer generate -w ~/.tauri/zhishu-ccs.key
```

配置 Repository Variables（macOS 合并打包作业也会读取，不要只放 Environment Variables）：

- `ZHISHU_PLATFORM_API_URL`、`ZHISHU_PLATFORM_WEB_URL`、`ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP`
- `ZHISHU_UPDATE_URL`、`ZHISHU_UPDATE_PUBLIC_KEY`

更新签名使用 Secret `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`；Apple 证书、公证及 Windows 代码签名沿用既有发布流程。CI 的安装包整理、签名、公证与重试仍由原工作流负责。

`ZHISHU_UPDATE_URL` 必须允许终端用户匿名访问，例如自己公开仓库的 `https://github.com/<组织>/<仓库>/releases/latest/download/latest.json`，或自己的 HTTPS 对象存储地址。私有仓库的产物需另发到公开可下载的服务。公钥、私钥和更新源必须属于自己的发布身份。

启用 R2 镜像需配置 `ZHISHU_R2_BUCKET`、`ZHISHU_R2_PUBLIC_BASE_URL` 和既有 R2 Secrets。镜像会清理所配置桶的旧版本，须使用专属客户端发布桶；配置不齐时不上传。

## 平台下载清单

单独生成网页下载清单的兼容入口继续保留：

```sh
node scripts/generate-download-manifest.mjs <安装包目录> <版本tag> <自己的下载根地址> <输出文件> <真实发布时间>
```

安装包位于 `<下载根地址>/<版本tag>/<文件名>`。GitHub Release 和 R2 镜像都会附带 `client-release.json`，平台将它提供为 `/client-release.json`；可部署到平台 `frontend/public/` 后构建，或由网关代理。SHA-256 和大小由真实文件计算。网页 `client-release.json` 与 updater `latest.json` 格式不同。

</details>
