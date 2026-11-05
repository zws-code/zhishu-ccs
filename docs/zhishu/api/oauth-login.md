# 智枢平台 OAuth2 授权登录：接入契约与已知问题

更新：2026-11-04。适用：`zhishu-ccs`（智枢客户端）、`dsh`（DeepSeek Harness 插件）。
网关侧实现：`zhishu-proxy` 仓库 `backend/internal/zhishu/clientoauth/`、`backend/internal/zhishu/clientapi/handler.go`、`frontend/src/zhishu/views/OAuthAuthorizeView.vue`。
网关版本：提交 `840c66206`（含 P0 CSP 修复） + 仓库内未提交的 app 深链改动（`app_url`）。

## 0. 当前状态

- 网关已实现授权服务器、公开同意页、授权码 + PKCE（S256）与 loopback 回跳；后端与前端测试通过。
- 真实浏览器端到端登录尚未验收。
- **P0 已修复（网关提交 `840c66206`）：仅在 `/oauth/authorize` 页面把 loopback 来源加入 `frame-src`，成功页的隐藏 iframe 现在默认可用；其余页面 CSP 不变。** 见第 6 节 P0。
- 本仓库 `src-tauri/src/zhishu/services/platform_oauth.rs` 的公开客户端与回环实现与本契约一致（`client_id=zhishu-ccs`）。

## 1. 客户端 ID

| 客户端 | client_id | 说明 |
| --- | --- | --- |
| 智枢客户端 | `zhishu-ccs` | 当前实现正确使用该 ID |
| DeepSeek Harness | `dsh` | 网关已注册；dsh 插件目前仍以 `zhishu-ccs` 授权，见 P1（不影响 zhishu-ccs） |

每个客户端必须使用**自己的** `client_id`。网关把授权码绑定到发起授权的 client_id，换 token 时必须提交同一个 ID。

## 2. 配置发现

`GET /api/zhishu/v1/client/config`（公开，`Cache-Control: no-store`）新增字段：

```json
{
  "login_methods": ["password", "oauth"],
  "oauth_authorize_url": "https://<gateway>/oauth/authorize",
  "oauth_client_id": "zhishu-ccs",
  "oauth_scopes": ["client"],
  "oauth_clients": [
    { "client_id": "zhishu-ccs", "client_name": "智枢客户端" },
    { "client_id": "dsh", "client_name": "DeepSeek Harness" }
  ]
}
```

- 智枢客户端仅保留 OAuth2。只有 `login_methods` 包含 `oauth`、`oauth_authorize_url` 和 `oauth_client_id` 非空时允许发起授权；缺失字段或配置失败时禁用登录并提示重试/更换服务器，不回退密码登录。
- `oauth_client_id` 是单客户端时代的旧字段，固定为 `zhishu-ccs`；客户端应使用自己的固定 ID，或从 `oauth_clients` 中按应用 ID 选择。
- 平台地址仍以配置/用户填写为准；`oauth_authorize_url` 由网关 `FrontendURL` 拼出。

## 3. 登录流程（Authorization Code + PKCE，RFC 6749/7636/8252）

1. 客户端生成 `state`（随机、至少 128 bit）与 `code_verifier`（43–128 字符，`[A-Za-z0-9-._~]`），计算 `code_challenge = BASE64URL(SHA256(verifier))`。
2. 在本机启动临时 loopback 监听（推荐 `http://127.0.0.1:<随机端口>/<自己的路径>`），只接受 GET，路径不得带 query/fragment。
3. 用系统浏览器或新标签页打开：

```text
{oauth_authorize_url}?response_type=code
  &client_id=<自己的 client_id>
  &redirect_uri=http://127.0.0.1:<port>/<path>
  &code_challenge=<S256>
  &code_challenge_method=S256
  &state=<state>
  &scope=client
```

4. 用户在平台网页完成登录（账号密码、2FA、已有第三方登录、人机验证都在网页完成）并确认授权。
5. 浏览器回到 loopback：成功 `?code=...&state=...`；拒绝 `?error=access_denied&state=...`。
6. 客户端校验 `state` 后立即调用 token 端点换令牌（授权码 120 秒有效、一次性）。
7. 令牌与密码登录完全相同：可用于 `/api/zhishu/v1/client/account`、`/access`、`/usage/summary`；刷新用 `/api/zhishu/v1/client/auth/refresh`，登出用 `/client/auth/logout`。

## 4. 接口

### 4.1 `GET /api/zhishu/v1/oauth/authorize/context`（公开）

参数：`client_id`、`redirect_uri`、`code_challenge`、`code_challenge_method=S256`、`state`、`scope`。
返回：`{ client_id, client_name, scopes, redirect_uri }`。供同意页展示和早期校验；客户端一般不需要调用。

### 4.2 `POST /api/zhishu/v1/oauth/authorize`（平台同意页内部使用）

需要 `Authorization: Bearer <平台 JWT>`，请求体包含上述参数加 `decision: "approve" | "deny"`，返回 `{ redirect_url, app_url? }`：`redirect_url` 是 loopback 回跳地址；`app_url` 是客户端注册的深链（`zhishu-ccswitch://v1/oauth`、`dsh://auth/callback`），用于尝试直接拉起应用。
客户端不需要调用；同意页由平台负责。

### 4.3 `POST /api/zhishu/v1/oauth/token`（公开）

接受 `application/json` 或 `application/x-www-form-urlencoded`：

```text
grant_type=authorization_code
client_id=<自己的 client_id>
code=<授权码>
redirect_uri=<与授权时完全一致>
code_verifier=<PKCE verifier>
```

成功（面板封装，HTTP 200）：

```json
{ "code": 0, "data": { "access_token": "...", "refresh_token": "...", "expires_in": 86400, "token_type": "Bearer", "scope": "client" } }
```

失败：HTTP 400/401/403/429，响应为 `{ code, message, reason }`：

| reason | 含义 |
| --- | --- |
| `CLIENT_OAUTH_INVALID_REQUEST` | 参数缺失/非法（redirect_uri、challenge、state、scope） |
| `CLIENT_OAUTH_INVALID_CLIENT` | 未知 client_id |
| `CLIENT_OAUTH_INVALID_GRANT` | 授权码无效/过期/已用、redirect_uri 或 client_id 不匹配、verifier 不匹配、账号不可用 |
| `CLIENT_OAUTH_UNSUPPORTED_GRANT_TYPE` | grant_type 不是 authorization_code |
| `CLIENT_OAUTH_FORBIDDEN` | 平台处于后台模式且用户不是管理员 |

### 4.4 redirect_uri 规则

只接受：`http`、host 为 `127.0.0.1` / `localhost` / `[::1]`、显式端口、路径以 `/` 开头；不得包含 userinfo、query 或 fragment。token 交换时与授权时逐字比对。

### 4.5 其他约束

- 授权码 120 秒有效、一次性（Redis 存储，key 为 code 的 SHA-256），绑定 client_id、redirect_uri、challenge、用户。
- `scope` 只支持 `client`（可省略，默认 `client`）。
- token 端点不记录请求体；客户端同样不得记录 code、verifier、token。

## 5. 客户端实现要求

- 回环监听必须单次有效：收到一次回调后关闭；`state` 不匹配、路径不符的请求不应结束流程。
- 拒绝授权（`error=access_denied`）按用户取消处理，不视为凭证错误。
- 等待超时建议 ≥ 5 分钟（平台侧要求用户完成登录/2FA/验证码）；授权码只在用户点击授权后生成，等待期间不会过期。
- 换 token 不要盲目重试 5xx：授权码是一次性的，第一次请求可能已在服务端消费。
- 若平台开启 `session_binding`，令牌会按 token 交换请求的 IP/UA 绑定；客户端换 UA、切网络或经代理可能使会话失效。平台默认关闭该开关。
- 刷新与登出沿用现有接口；不要把 access token 当模型 API Key 使用。

## 6. 已知问题（2026-11-02 网关复核）

### P0：隐藏 iframe 回传被默认 CSP 拦截（网关侧，影响两个客户端）——已修复

- 现象：平台同意页授权成功后不跳转，改用隐藏 iframe 请求 loopback 地址（`frontend/src/zhishu/composables/useOAuthAuthorize.ts` 的 `deliverToLoopback`）。
- 原因：网关默认 CSP 的 `frame-src` 不带 loopback，浏览器拒绝加载该 iframe。
- 修复（2026-11-02，提交 `840c66206`）：`/oauth/authorize`（含末尾斜杠）的响应额外加入 `http://127.0.0.1:*`、`http://localhost:*`、`http://[::1]:*`；其他页面仍用默认策略，`backend/internal/server/middleware/security_headers_test.go` 覆盖这两类路径。
- 注意：修复随网关版本生效，旧网关需升级；若线上由反向代理覆盖 CSP，也要确认其未删除 `frame-src` 中的 loopback 来源。
- 剩余风险：个别浏览器（如 Safari）对 loopback 混合内容更严格；若遇到，可改用 app 深链或顶层跳转。
- 可用下面的命令确认线上是否已包含 loopback：

```bash
curl -sI https://<gateway>/oauth/authorize | grep -i content-security-policy
```

### P1：dsh 目前以 `zhishu-ccs` 身份授权（dsh 插件侧）

- 插件只读 `oauth_client_id`（固定为 `zhishu-ccs`），且兜底常量也是 `zhishu-ccs`，从不读取 `oauth_clients`。
- 后果：dsh 登录时同意页显示“智枢客户端”，授权码绑定 `zhishu-ccs`。当前能登录成功（两端一致），但客户端身份错误；后续按客户端做权限、审计或回调限制时会直接失效。
- 修复：dsh 插件在授权与换 token 时使用 `client_id=dsh`。网关无需改动；zhishu-ccs 不受影响。

### P2：app 深链只作为唤醒信号（zhishu-ccs 已处理）

授权响应新增 `app_url`，成功页会先尝试深链、再投递 loopback。`zhishu-ccs` 现在识别 `zhishu-ccswitch://v1/oauth`（兼容 legacy `ccswitch://`）：只聚焦主窗口并继续等待 loopback，不会再把唤醒链接当成导入请求弹出解析错误。授权码仍只从 loopback 接收；dsh 仍未处理 `dsh://auth/callback`。建议 dsh 实现前不要把深链作为自动动作，至少先投递 loopback。

### P2：插件文档仍声明不支持网页授权

dsh 仓库的 `DSH登录插件接入.md` 第 1 节与第 9 节仍写着不支持网页授权登录，与插件现状不一致。

### P3：平台成功页与实际投递脱节

成功页在授权成功后立即显示“登录成功”，但页面无法确认 iframe 是否到达客户端；“打开应用”重试仍是同一机制，且授权码 120 秒后过期。CSP 修复后默认部署可正常投递；客户端未启动时仍只能靠重试或重新登录。

### P3：重复点击“打开应用”

两个客户端收到回调后都会关闭 loopback 监听；重复点击通常只会得到连接拒绝或已消费的 code。按钮只应在首次投递失败（客户端未启动）时使用。

### P3：`oauth_client_id` 与 `oauth_clients` 并存

前者是旧字段，固定 `zhishu-ccs`。客户端不要依赖它判断自己的身份；网关后续可能只保留 `oauth_clients`。

## 7. 客户端验收清单

- [ ] `/config` 返回 `oauth`，授权页能打开并按自己的 `client_id` 展示名称。
- [ ] 未登录时先登录再回到同意页；登录方式（密码/2FA/第三方/验证码）不受影响。
- [ ] 同意后回环收到 `code` 且 `state` 匹配；拒绝时收到 `error=access_denied`。
- [ ] token 交换成功，`/account`、`/access` 正常，刷新与登出工作。
- [ ] 错误路径：错误 verifier、错误 redirect_uri、过期码、重复使用、后台模式。
- [ ] 平台默认 CSP 下的真实浏览器链路（`/oauth/authorize` 已放开 loopback frame-src）。

## 8. Windows / Edge 授权后等待的专项审查（2026-11-04）

### 场景与结论边界

用户报告 Windows 使用 Edge 完成平台授权后客户端持续转圈。本次检查当前 `0.0.7` 源码、前后端调用和测试。随后用户在 Windows 虚拟机复现，提供 IPv6 CSP 警告及两条回调请求标红无响应的观察（见下文）；本次智能体未直接操作该虚拟机，仍缺 loopback 请求的具体 `net::ERR_*` / blocked reason 与 Edge 版本。以下区分代码确定缺陷与待现场验证的原因，不能宣称已解决该用户的真实故障。

### 高优先级：隐藏 iframe 投递可能被 Edge LNA 阻止（未修改网关）

本文件记录的网关投递机制使用隐藏 iframe 请求 loopback；客户端把深链只作为唤醒信号，即使网页向深链附加 code/state，当前客户端也不消费这些参数来完成 OAuth。平台网页授权成功不代表授权码已经送达客户端。

微软[官方 LNA 说明](https://learn.microsoft.com/en-us/deployedge/ms-edge-local-network-access)明确：Edge 143 起默认启用 Local Network Access 限制，覆盖子框架导航、fetch 与子资源请求；当前不覆盖顶层页面导航。请求权限需要安全上下文，HTTP 页面查询该权限始终为 denied。CSP 放行 `frame-src` 不能替代 LNA 权限。

排查必须检查现场 Edge 版本、授权网页实际 HTTPS/HTTP 来源、站点本地网络权限、DevTools 对 loopback 请求的拦截原因，以及网关/反向代理最终 CSP；不能只凭网页显示“登录成功”判断投递成功。跨域 iframe 场景下，仅允许站点权限也可能不够。

网关后续应优先提供标准的 **顶层导航到服务端生成的完整 `redirect_url`**，或明确的用户点击回到客户端入口，避免仅依赖隐藏 iframe。保留 state、PKCE 与原始 redirect_uri 校验。不要让用户全局关闭浏览器安全限制；本仓库的客户端接收修复无法绕过浏览器侧未发送的请求。

### 已修复：客户端回调接收与日志

修改均位于自有模块，侵入等级 **L0**，没有新增上游补丁、依赖、IPC 或存储格式：

- [`platform_oauth.rs`](../../../src-tauri/src/zhishu/services/platform_oauth.rs)：旧代码只 `read` 一次即解析请求，TCP 分段可能把有效回调误判并继续等待。现在在 8 KiB 上限内读完整 HTTP 请求头，复用已有 `httparse` 解析器。
- 同一文件：旧代码串行接收和读取连接，空预连接最多阻塞后续请求 10 秒，读取及响应期间不能及时响应取消/总超时，响应写入本身也没有超时。现在最多并发 16 个连接，每连接读取和响应共用 10 秒期限，取消与 300 秒总期限独立生效；结束时取消剩余连接。state 不匹配、拒绝授权与 PKCE 行为保持原契约。
- 同一文件：回环页面改为“授权已接收，正在验证登录”，避免在兑换令牌前宣称已登录。等待超时明确提示未收到回调及浏览器拦截可能。
- [`platform_auth.rs`](../../../src-tauri/src/zhishu/services/platform_auth.rs) 与 OAuth 模块：新增 `[platform-oauth]` 阶段日志，覆盖操作锁等待、存储初始化、配置读取、浏览器启动、有效回调、令牌兑换、账户验证、旧会话清理、新会话保存及完成/受控错误码。日志不含授权地址、state、授权码、verifier 或 token。
- [`platform_http_log.rs`](../../../src-tauri/src/zhishu/services/platform_http_log.rs)：发现 OAuth token 请求体的字符串 `code` 未被通用脱敏器识别；现在只在该端点把授权码加入请求/响应已知秘密列表，保留业务响应的数字 `code`。不修改实际发送内容。

### 已确认但本次未修复的相邻问题

1. **跨阶段取消不完整**：取消 receiver 只存活于等待回调阶段。令牌交换期间取消可能返回成功，后端仍继续登录；完成阶段又持有认证锁，取消需等登录完成。前端会丢弃迟到结果，但不能阻止后端已经发布/持久化的会话。等待操作锁或注册通道前的取消同样可能丢失。完整修复需要覆盖整次尝试的取消状态、身份代际以及会话发布屏障，相关现有测试只覆盖前端丢弃结果。
2. **系统凭据库无界等待**：会话存储等待不可取消的同步 `spawn_blocking` 任务；若 keyring/文件系统挂起，登录 IPC、认证锁与操作锁可能持续占用。没有证据证明该用户命中了 Windows 凭据库问题。不能只加 Promise 超时或丢弃阻塞任务：迟到的 create/touch 仍可能写回会话指针，导致取消/退出后可恢复。需要单独设计提交屏障与慢凭据库测试。
3. **旧会话清理不可随意降级**：现有清理已允许“会话指针失效”或“旧凭据删除”任一成功。真正失败时两条阻断旧账户恢复的途径都不可靠；不能简单改警告后继续发布新账户。

### 用户虚拟机复现与线上响应头核对（2026-11-04 18:16 后）

- 用户报告：Windows 虚拟机中客户端等待授权，取消后能很快恢复按钮；授权页控制台提示 `frame-src` 中 `http://[::1]:*` 无效并被忽略；自定义协议深链与 IPv4 loopback 回调在 Network 均标红、未显示响应。现场提供的 code/state 不在本文保存，也未被智能体重放。
- 只读实测：对 `http://43.136.64.205/oauth/authorize` 发 HEAD，HTTP 200，返回单条 CSP，`frame-src` 同时包含 `http://127.0.0.1:*`、`http://localhost:*` 和被浏览器报告无效的 IPv6 项。IPv6 项警告应修正，但浏览器忽略该项不等于忽略整个指令，不能据此认定 IPv4 回调被 CSP 阻止。HEAD 结果仍不能代替现场完整响应头、iframe 属性和浏览器 blocked reason。
- 自定义协议没有普通 HTTP 响应，Network 标红不能单独证明应用唤醒失败；当前客户端 `is_oauth_wakeup` 只识别路径并聚焦窗口，不消费 code/state，所以深链也不是现有授权码接收的备用通道。
- 用户对照结果（18:36 反馈）：把失败的完整 HTTP loopback 回调地址粘到浏览器地址栏后，客户端成功登录。这证明该轮 Windows 回环监听、有效回调处理及后续登录链路可用；故障聚焦授权网页的内嵌投递路径，不能再把本次复现归因于监听创建失败或系统凭据库持续挂起。此项是用户实测报告，智能体未重放授权码；仍缺 blocked reason，不能把具体浏览器限制直接定为 LNA 或上述 IPv6 CSP 警告。
- 修复方向：在网关授权页以顶层导航打开服务端返回的完整 `redirect_url`，复用既有 state / PKCE / redirect_uri 校验，保留明确的用户点击回到客户端入口；不以深链唤醒成功作为授权码送达确认。当前工作区未找到网关的 `useOAuthAuthorize.ts`，本次没有修改或部署网关，客户端接收端的修复不能替代该投递路径改造。

### 定位办法与验证记录

新版本日志最后停在 `opening browser; loopback listener ready`，且没有 `validated callback received`：优先查网页向本机的投递和浏览器限制。停在 `exchanging authorization code` 或 `validating account`：查对应平台 HTTP 请求及受控错误码（单次请求超时 20 秒）。停在 `clearing previous persisted session` 或 `saving new persisted session`：检查系统凭据库与应用数据目录。深链把窗口拉起不足以证明回环已到达。

已执行：平台后端相关回归 **117 项**通过，包含认证/会话、OAuth 回调、HTTP 日志、平台目录/同步/退出清理（OAuth 子集 12 项，含新增 TCP 分段、空预连接、读取中取消、总超时、超限请求 5 项；另新增授权码脱敏测试 1 项）；前端登录及接口回归 **40 项**通过；`cargo fmt --check --manifest-path src-tauri/Cargo.toml` 通过。后端回归使用临时 `CC_SWITCH_TEST_HOME`、模拟服务器和内存凭据库。

本智能体未执行：Windows + Edge 实机操作、用户现场直接复现、真实系统凭据库挂起/恢复、网关代码修复及部署、完整工程 Clippy/全部测试/发布构建。用户已报告 Windows 虚拟机失败及地址栏顶层导航成功，范围见上文；不等同于改造后网关的正式验收。回退可撤销本次自有模块修改，无数据库迁移或凭据迁移；前述待修项在回退前后均存在。
