# 客户端专用 API 契约与实现映射

> 服务端：Zhishu Proxy，接口版本 v1。文档日期：2026-10-31。
> 2026-10-31 客户端实施：源码已切换专用接口，旧平台调用和 Key 生命周期逻辑已删除。L0 自有业务 + L1 注册/翻译 + L2 共享元数据扩展，见 [ADR-0004](../adr/ADR-0004-client-api-only.md) 和 [验证记录](../history/validation.md)。以下保留服务端接入契约，真实平台验收另行记录。
> 本文的 HTTP/DTO 契约用于解释客户端实现；服务端内部事务、管理后台行为及线上部署需在服务端仓库单独核验，不能由客户端源码推定。

## 1. 职责与必须停止的旧行为

平台统一负责用户分组授权、专用 Key 整理/创建/删除、订阅及额度状态、授权模型目录。一个用户的一个分组只有一个专用 Key，多个客户端、设备和 Agent 共用。

**CCS 从平台获取专用 API Key 的唯一入口是 `POST /api/zhishu/v1/client/access`。**

客户端必须停止以下旧行为（本次源码已移除）：

- 查询旧 `GET /api/v1/keys`、Key 详情或按命名分页查找来获取平台凭证。
- 调用旧 `POST/PUT/DELETE /api/v1/keys[/:id]` 创建、修改、替换或清理平台 Key。
- 根据分组前缀、订阅到期、网络失败、模型请求 401/403 决定创建或删除 Key。
- 使用旧 `/api/v1/client/credentials` 或 `/api/zhishu/v1/client/credentials` 作为兼容回退。
- 新接口不可用时恢复任何上述旧逻辑。

客户端请求器使用封闭的方法/路径白名单，拒绝旧面板和 credentials 路由，不提供旧接口回退。旧客户端和服务端部署状态须分别核验。

客户端继续负责自身的本地快照、状态展示、重试、Provider 管理、工具能力适配和配置文件写入。删除本地 Provider、退出某个客户端不能触发删除共享平台 Key。

## 2. 接口清单与调用顺序

前缀：`/api/zhishu/v1/client`。

| 方法与路径 | 认证 | 用途 |
|---|---|---|
| GET /config | 无 | 平台名称、登录配置、验证码状态和网页入口 |
| POST /auth/login | 无 | 邮箱密码登录 |
| POST /auth/login/2fa | TOTP 临时凭证 | 完成二次验证 |
| POST /auth/refresh | refresh token | 换取并轮换登录令牌 |
| POST /auth/logout | refresh token | 撤销当前刷新会话 |
| GET /account | 用户 access token | 账户信息、状态、余额 |
| POST /access | 用户 access token | 整理并获取平台基础数据与专用 Key |
| GET /usage/summary | 用户 access token | 只读额度快照，不返回 Key 明文 |

请求及响应使用 JSON。成功外壳统一为：

```json
{"code":0,"message":"success","data":{}}
```

登录后的接口使用 `Authorization: Bearer <access_token>`。用户身份来自登录令牌；不接受客户端指定其他用户或分组来扩权。模型 API Key 与登录令牌用途不同，不得混用。

流程：

1. 读取 config，登录；若要求 TOTP，再完成 /auth/login/2fa。
2. 调用 POST /access，校验当前平台/用户身份，原子保存完整本地快照。
3. 客户端读取快照，按各工具规则同步 Provider 与配置。
4. 展示刷新通过 /account 和 /usage/summary 完成；不重复整理 Key 或应用工具配置。

HTTP 契约要求这些接口返回 `Cache-Control: no-store`，实际响应头需在服务端或联调时核验。令牌与 Key 不进入普通前端查询缓存、诊断日志或错误信息。

### 登录与续期示例

```http
POST /api/zhishu/v1/client/auth/login
Content-Type: application/json

{"email":"demo@example.invalid","password":"example-password"}
```

登录复用平台原有认证规则与防滥用限制。正常成功 data 包含 `access_token`、`refresh_token`、`expires_in`（秒）、`token_type` 和 `user`；兼容降级见下文。需要 TOTP 时包含 `requires_2fa: true`、`temp_token` 等原登录响应字段，继续请求：

```json
{"temp_token":"example-temporary-token","totp_code":"123456"}
```

/auth/refresh 和 /auth/logout 请求体均使用：

```json
{"refresh_token":"example-refresh-token"}
```

刷新成功后保存新 refresh token，后续不要继续使用旧值。退出尝试撤销对应刷新会话，不删除多个客户端共用的模型 Key；具体兼容语义见下文。

/config 返回 `schema_version`、`site_name`、`api_base_url`、`login_methods`、`totp_enabled`、注册和找回密码开关与网页入口，以及验证码启用信息。首版客户端实现邮箱密码与 TOTP；平台启用验证码时仍须满足现有登录校验，不能通过客户端接口绕过。浏览器授权与验证码交互不属于本次 CCS 实施范围。


### /config、账户与认证字段补充

/config 成功时的 data 示例：

```json
{
  "schema_version": 1,
  "site_name": "示例平台",
  "api_base_url": "https://api.example.invalid",
  "login_methods": ["password"],
  "totp_enabled": true,
  "registration_enabled": true,
  "password_reset_enabled": true,
  "register_url": "https://www.example.invalid/register",
  "password_reset_url": "https://www.example.invalid/forgot-password",
  "subscriptions_url": "https://www.example.invalid/subscriptions",
  "turnstile_enabled": false,
  "turnstile_site_key": "",
  "tencent_captcha_enabled": false,
  "tencent_captcha_app_id": "",
  "aliyun_captcha_enabled": false
}
```

网页地址来自平台配置；返回相对路径时，相对于当前登录平台 origin 解析。开关为 false 时不应展示相应入口。服务端契约可接受 `turnstile_token`，或 `tencent_captcha_ticket` 与 `tencent_captcha_randstr` 配对字段；当前 CCS 尚未实现验证码交互，启用这些验证时明确拒绝密码登录，不绕过校验。

TOTP 中间响应的 data 示例：

```json
{"requires_2fa":true,"temp_token":"example-temporary-token","user_email_masked":"d***@example.invalid"}
```

登录成功的 `user` 沿用平台现有用户 DTO；账户刷新使用精简的 /account 契约。现有登录实现遇到刷新令牌生成失败时可只返回 `access_token`、`token_type` 和 `user`，因此 `refresh_token`、`expires_in` 是可选字段；缺少刷新令牌时客户端必须标记该会话不能自动续期，不能伪造或复用上一次账号的刷新令牌。

/auth/refresh 成功的 data：

```json
{"access_token":"example-access-token","refresh_token":"example-new-refresh-token","expires_in":3600,"token_type":"Bearer"}
```

/auth/logout 成功的 data 为 `{"message":"Logged out successfully"}`。该接口沿用现有登出语义：撤销所提交刷新令牌为尽力执行，空请求也可成功；客户端退出时必须提交本会话 refresh token 并清除本地登录态。成功响应不表示已有 access token 立即失效，它仍可能存活至到期；不撤销其他客户端会话，也不删除共享 API Key。

GET /account 的完整成功示例：

```json
{"code":0,"message":"success","data":{"id":7,"email":"demo@example.invalid","username":"demo","status":"active","balance_usd":10}}
```

GET /usage/summary 在当前没有符合条件的分组时：

```json
{"code":0,"message":"success","data":{"schema_version":1,"synced_at":"2026-10-31T08:00:00Z","account":{"id":7,"email":"demo@example.invalid","username":"demo","status":"active","balance_usd":10},"groups":[]}}
```

有分组时使用第 3 节的分组结构，但 `credential.key`、`connection`、`catalog_status`、`catalog_error` 不出现，`models` 为 null。客户端合并额度刷新时，不得用这些缺省值覆盖 /access 保存的凭证、连接和模型数据。

## 3. /access 响应契约

请求体可省略或使用 `{}`；不提供客户端创建 Key 的名称、额度、过期时间、用户 ID 或分组 ID 参数。整理可能产生数据库写入，因此必须使用 POST。

下例全部账号、域名和凭证为虚构数据：

```json
{
  "code": 0,
  "message": "success",
  "data": {
    "schema_version": 1,
    "synced_at": "2026-10-31T08:00:00Z",
    "account": {
      "id": 7,
      "email": "demo@example.invalid",
      "username": "demo",
      "status": "active",
      "balance_usd": 10
    },
    "groups": [{
      "id": 42,
      "name": "智枢router-ccs-demo",
      "platform": "openai",
      "status": "active",
      "subscription_type": "standard",
      "is_exclusive": false,
      "credential": {
        "id": 123,
        "name": "zhishu-client-42",
        "key": "sk-example-not-a-real-credential",
        "status": "active",
        "client_managed": true,
        "expires_at": null,
        "quota_usd": 0,
        "used_usd": 0
      },
      "quota": {"available": true, "reasons": []},
      "subscriptions": [],
      "connection": {
        "base_url": "https://api.example.invalid",
        "endpoints": {
          "messages": "/v1/messages",
          "responses": "/v1/responses",
          "chat_completions": "/v1/chat/completions",
          "gemini": "/v1beta/models"
        }
      },
      "models": [{"id":"example-model","display_name":"Example Model","context_window":128000}],
      "catalog_status": "ok"
    }]
  }
}
```

字段约定：

| 字段 | 含义 |
|---|---|
| schema_version | 当前为 1 |
| synced_at | 服务端生成快照的 RFC3339 时间；CCS 本地摘要可转换为 Unix 毫秒 |
| account | 当前 JWT 用户；不包含密码、登录令牌或管理员备注 |
| groups | 当前接入范围内的分组；含已有专用 Key 但订阅过期/额度不足的分组 |
| credential | 专用 Key 的身份和状态；不存在时为 null |
| credential.key | 仅 /access 下发；不得从 ID、名称推导或寻找替代 Key |
| credential.expires_at | Key 自身到期时间，null 表示未设置 |
| credential.quota_usd | Key 自身额度，0 表示未设置独立上限 |
| credential.used_usd | Key 自身配额计数，不是完整历史账单 |
| quota | 当前额度与凭证状态快照，不能仅根据 credential 存在判断可调用 |
| subscriptions | 该分组现存订阅关联，包含到期/暂停记录；非订阅分组为空数组 |
| connection.base_url | 配置的网关地址；为空时使用当前已登录平台的 origin |
| connection.endpoints | 网关协议路径目录；并不代表每个模型都支持所有协议 |
| models | 成功时为数组（允许空）；目录失败或只读额度查询时为 null |
| catalog_status | /access 中为 ok 或 failed |
| catalog_error | 目录失败时为 MODEL_CATALOG_UNAVAILABLE |

模型能力可包含 `description`、`context_window`、`max_output_tokens`、`supported_endpoint_types` / `supported_endpoints`、`supports_tools` / `supports_function_calling`、`supports_streaming`、`input_modalities`、`supported_reasoning_levels`、`default_reasoning_level`。未提供的字段表示未知，不能填入猜测的固定窗口、思考档位或工具能力。以服务端返回的模型 ID 为授权范围，不用其他模型列表扩大它。

### 额度标识与订阅

`quota.available` 为本次额度检查结果，不是上游可达性或调用成功承诺；`quota.reasons` 为原因数组：

| 原因 | 含义 |
|---|---|
| key_missing | 当前没有专用 Key；只读查询不会补建 |
| key_expired | Key 自身已过期 |
| key_quota_exhausted | Key 自身额度耗尽 |
| key_disabled | Key 非活跃 |
| key_rate_limit_exhausted | Key 的 5 小时、日或 7 日额度窗口耗尽 |
| balance_insufficient | 按量分组对应账户余额不足 |
| subscription_expired | 没有有效订阅 |
| subscription_suspended | 订阅暂停且没有其他有效订阅 |
| subscription_quota_exhausted | 有效订阅的周期额度耗尽 |

订阅对象包含 `id`、`group_id`、`status`、`starts_at`、`expires_at`、`active` 及 `daily/weekly/monthly`。各窗口均包含：

```json
{"limit_usd":10,"used_usd":4,"remaining_usd":6,"resets_at":"2026-11-01T00:00:00Z"}
```

未设置窗口限额时 `limit_usd` 和 `remaining_usd` 为 null；窗口未激活时重置时间可为 null。不要据此推断过期订阅仍可无限使用。金额单位为 USD，具体日期和窗口语义遵循服务端平台规则。

账号余额刷新返回与 account 同形状的数据。GET /usage/summary 使用与快照相同的账户、分组、Key 元信息、额度和订阅结构，但不含 `credential.key`、连接配置和模型目录；不创建、删除 Key。它不能作为取得 Key 明文的替代接口。

## 4. Key 生命周期边界

客户端只消费成功 `/access` 响应中的授权分组、凭证、模型与额度；分组前缀、Key 申请/撤销、到期及续订规则属于服务端责任。客户端不从 ID 或名称寻找替代 Key，不因网络失败或额度不足自行创建凭证。

成功空集合与获取失败分别处理；目录失败保留既有供应商。删除本地 Provider 或退出登录不撤销共享模型 Key。具体本地协调见 [功能说明](../features/platform-managed-models.md)。

## 5. 失败语义与本地两阶段衔接

HTTP 失败使用平台原有错误外壳：

```json
{"code":403,"message":"client-managed API keys are read-only","reason":"CLIENT_API_KEY_READ_ONLY"}
```

- 登录缺失、失效、过期或用户禁用：按认证失败处理；不得把它转为创建模型 Key。
- 429：平台登录或创建频率等限制；客户端自行决定重试。
- 分组、用户授权、订阅、关联查询或 Key 整理失败：客户端收到整次 /access 失败时保留本机快照，不应用空数据；服务端事务回滚须由服务端验证。
- 模型目录单独失败：/access 可成功返回分组和凭证，同时该组 `catalog_status=failed`、`models=null`；保留失败状态，不将目录误判为空目录删除。
- 额度不可用：同步仍可成功，使用 quota 标识展示；与网络/同步失败分别处理。
- 请求取消或响应丢失时，服务端事务可能已经提交；再次 /access 复用关联，客户端不得自行创建补偿 Key。
- 任何上述失败都不得回退到旧后台 Key 整理接口。

CCS 修改位置与映射：

| 当前能力 | 客户端改造 |
|---|---|
| discover_group_catalogs | 已通过 /access 聚合请求与 DTO 映射 |
| platform_sync_catalog | 保留 IPC；获取新快照并私密原子保存，不写工具配置 |
| platform_get_catalog | 仍只读本机白名单摘要，不包含明文 Key/JWT |
| platform_get_catalog_key | 保留显式查看与平台、用户、分组归属校验 |
| platform_sync_providers | 保持读取本地快照，工具协议适配与 Provider/配置更新归客户端 |
| 账户刷新 | GET /api/zhishu/v1/client/account |
| 额度刷新 | GET /api/zhishu/v1/client/usage/summary |

本地 `keyAvailable` 目前表示快照中存在凭证明文，不能直接改成额度可用；新增额度状态字段承接 `quota`。按 `baseUrl + userId` 隔离数据，拒绝登录切换后的迟到响应。订阅过期在新快照中仍有分组和 Key；不再根据客户端旧前缀规则过滤服务端已授权的专属分组。


模型与 CCS 摘要字段的映射要求：

| 服务端字段 | CCS 本地摘要/适配输入 | 缺省处理 |
|---|---|---|
| models[].id | id | 必填；无 ID 的条目视为格式错误 |
| display_name | name | 可回落为 id |
| description | description | 保持缺失 |
| context_window / max_output_tokens | contextWindow / maxOutputTokens | 保持未知，不按名称猜测 |
| supported_endpoint_types / supported_endpoints | endpoints | 缺失表示未知；不能从 connection.endpoints 推导模型能力 |
| supports_tools / supports_function_calling | supportsTools | 保留 true / false / 未知三态 |
| supports_streaming | supportsStreaming | 保留 true / false / 未知三态 |
| input_modalities | inputModalities | 无声明时保持未知 |
| supported_reasoning_levels | reasoningLevels | 只保存明确返回的档位 |
| default_reasoning_level | defaultReasoningLevel | 缺失不自行设定 |
| credential.id / credential.name | keyId / keyName | 保持服务端身份，不按名称检索其他 Key |
| credential.key | 私密快照；摘要仅给 keyAvailable | 禁止进入普通前端摘要 |
| catalog_status / catalog_error | failed / error | failed 不等同于 Key 缺失或额度不足 |
| quota / subscriptions | 新增额度及订阅状态字段 | 与 keyAvailable 分开保存和展示 |

工具/流式能力的适配输入保留可空值；工具必须使用默认值时由 Adapter 显式决定，不写回平台已声明能力。本机目录为版本 2，HTTP schema_version 为 1，两者不是同一版本号。旧单文件 v2 仅按账号归属迁移，v1 不迁移字段或接管凭证，需重新同步；详见 ADR-0005。

## 6. CCS 验收清单（执行情况见验证记录）

- 登录后仅从新 /access 获取平台专用 Key；网络记录不出现旧 Key 列表、详情、创建、修改、删除或旧 credentials 回退。
- 代码中不再执行平台 Key 命名查找、创建、轮换、旧 Key 引用清理及分组授权推断。
- 多客户端同步同一账号同组取得同一 Key；客户端不提交设备 ID 来申请另一枚 Key。
- 基础同步只写本地快照；工具配置同步不发平台请求、不刷新额度、不创建或删除平台 Key。
- Key/订阅过期、暂停和额度耗尽正确展示；续订继续使用同一 Key。
- 平台快照失败、单组模型失败和成功空列表有不同处理；保留其他账号、自建 Provider 和有效本地数据。
- 新接口不可用时有失败提示与重试路径，不回退旧接口。
- 新快照的模型能力完整映射，未知参数保留未知；完成目标工具的真实配置及模型调用验收。

## 7. 当前源码入口

| 职责 | 文件 |
| --- | --- |
| HTTP 白名单、登录、续期、账户、access 和 usage 请求 | `src-tauri/src/zhishu/services/platform_auth.rs` |
| 响应 DTO、版本与模型字段校验 | `src-tauri/src/zhishu/services/client_api.rs` |
| 账号快照 v2 与旧文件迁移 | `src-tauri/src/zhishu/services/platform_models/base_data.rs` |
| 本地协调与格式适配 | `src-tauri/src/zhishu/services/platform_models/`、`src-tauri/src/zhishu/integrations/ccswitch/platform_models.rs` |
| 前端 IPC 与类型 | `src/zhishu/api/platformAuth.ts`、`platformModels.ts`、`platformUsage.ts` |
| 导出与实际注册 | `src-tauri/src/zhishu/commands/mod.rs`、`src-tauri/src/lib.rs` |
