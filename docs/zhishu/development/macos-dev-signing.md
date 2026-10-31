# macOS 开发构建与钥匙串授权

## 为什么重新编译后又要求钥匙串密码

`tauri dev` 通过 `cargo run` 启动调试二进制，不经过 Tauri 的 bundle 签名流程。默认 ad-hoc 签名的 `cdhash` 随编译产物变化。

原来的 `Zhishu CC Switch Dev` 自签名证书只能固定 designated requirement，不能提供 Apple Team ID。2026-10-26 本机排查确认：即使旧条目已允许该自签名身份，钥匙串的 partition list 仍绑定旧 `cdhash`；系统日志出现 `ACL partition mismatch`，每次重编译又要求授权。因此仅创建自签名证书、将它设为信任或对旧程序点击“始终允许”，都不足以解决这个问题。

开发启动现在使用 Apple 签发的代码签名证书，让钥匙串识别稳定的团队身份。平台会话和“记住密码”继续使用系统钥匙串。

## 本机准备与启动

需要一张有效且包含私钥的 Apple 代码签名证书，优先使用 Apple Development。已有证书可直接使用；没有时通过 Xcode 的 Apple 账户与证书管理创建或导入开发证书。

检查可用身份：

```sh
security find-identity -v -p codesigning
```

使用正常的开发入口：

```sh
pnpm dev
```

`pnpm dev` 在 macOS 上自动使用开发签名逻辑。开发隔离范围见 [隔离开发](dev-isolation.md)。直接运行 `pnpm tauri dev` 会绕过该签名流程。

启动器按以下顺序选择有效身份：Apple Development、Mac Developer、Developer ID Application、Apple Distribution、3rd Party Mac Developer Application。同一优先级有多个身份时会停止并列出指纹，需明确指定；也可主动指定完整名称或 SHA-1 指纹：

```sh
MACOS_DEV_SIGNING_IDENTITY='<证书的完整 SHA-1 指纹>' pnpm dev
```

`scripts/zhishu/dev-macos.sh` 将选中的指纹传给 Cargo target runner。每次编译启动前，`scripts/zhishu/macos-dev-sign.mjs` 对二进制签名，并检查 Apple 信任链、Team ID 和应用标识；缺少证书、签名或校验失败都会停止启动。`MACOS_DEV_SIGNING_IDENTIFIER` 可覆盖默认应用标识 `com.zhishu.ccswitch.desktop`，日常开发应保持同一标识和签名团队。

修改启动脚本或签名身份后，需要退出当前开发进程，重新运行 `pnpm dev`；已有 watcher 仍携带旧的签名环境变量。

## 首次切换时的授权

- `codesign` 首次使用证书私钥时，系统可能请求一次授权。
- 旧的会话或记住密码条目此前授权给另一种签名身份，切换后每个旧条目可能还需一次“始终允许”。后续保持相同团队与应用标识的重编译可复用授权。
- 系统钥匙串本身被锁定、证书失效或主动切换签名身份，仍可能需要系统认证；本流程不会修改钥匙串锁定策略。

无需删除已保存的登录信息，不修改全局钥匙串 ACL，也不把密码或 refresh token 改存明文文件。

这是本机开发签名；安装包签名、Apple 公证与 updater 更新签名仍按 [打包发布文档](../releases/README.md) 单独配置。

## 回归验证

普通回归不读取或修改真实钥匙串：

```sh
pnpm test:dev-signing
pnpm test:dev-isolation
```

在已配置证书的 macOS 上，可显式执行真实系统验证：

```sh
ZHISHU_CCS_RUN_KEYCHAIN_SMOKE=1 pnpm test:dev-signing
```

该测试编译并签名两个内容不同的临时程序，确认 `cdhash` 不同且 designated requirement 相同；第一版创建随机命名的测试钥匙串条目，第二版在禁止系统交互的情况下读取，最后删除条目。测试仅使用固定的无敏感信息占位值，不读取实际平台凭据；2026-10-26 已在本机通过。它验证跨编译的钥匙串访问，不替代旧凭据首次授权或整个应用的 GUI 验收。
