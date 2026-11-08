# ADR-0013：合并上游 v4.0.28

- 日期：2026-11-09
- 状态：已实施，本机自动验证通过；不构成发布批准
- 等级：L3（Codex 路由写入形态、会话历史迁移、代理转发与 IPC 契约扩展）
- 产品起点：`a04f940632901c6e59ba22becadb0482df92de71`，0.0.9
- 上游基线：v4.0.26 / `f621d4216189ef06bcc6acd3588c84937b9794e1`
- 目标：官方 v4.0.28；tag 对象 `9bd119930cc4fc56b46415e88215d20eeab96726`，提交 `8656ebd91f3eba631837126e29df71e8353b7e43`（即 upstream/main HEAD）；已联网 fetch 并通过 `git ls-remote upstream` 核验 tag 与 peeled commit。

## 目标与选择

吸收上游 63 个提交（214 个文件）：Codex 第三方路由在有 ChatGPT 登录时改写为内置 openai 形态、统一会话历史每次启动后台迁入（state DB 计数去重）、切换切分提示、官方额度保护默认开启、Codex 暂停官方登录设置、会话正文 FTS5 本地搜索、界面密度/缩放、软链接配置写穿、工具安装队列进度、Skills 布局与哈希修复、代理兼容修复（DeepSeek/Kimi 工具调用历史、跨厂商故障转移模型映射、剥离 ChatGPT 身份头、压缩重试）、定价种子 savepoint 批处理、TokenPlus/Genesis 预设、agy Argon 周额度等；同时保留智枢身份、OAuth2 会话、两阶段平台同步、WorkBuddy、DSH、配置历史与 ADR-0009 共存隔离。

整文件采用上游会覆盖智枢补丁；逐提交挑选会遗漏相互依赖并丢失完整上游祖先；继续沿用既定方式：从当前产品提交建立 `sync/upstream-4.0.28` 三方 merge，保留双方祖先，按调用链逐块适配。启动注册、路由写入、会话分桶、代理转发与语言包都是固定入口，纯新增模块无法替代内部调用，继续复用上游 Provider、DAO、IPC、路径函数及受控原子写入。

实施文件范围：上游 `App.tsx`/现有 UI 与五语言包（L1/L2）、Tauri/依赖/命令注册（L1），Codex 路由与历史迁移、代理、会话索引、settings 及相关 services（L3），对应前后端测试；准确清单以合并提交相对第一父的 diff 为准。新增本 ADR 与 2026-11-09 同步报告，更新基线、补丁登记、验证记录和索引（L0）。不新增平行核心实现。

## 关键语义决策

### Codex 第三方路由的新写入形态（上游 905c9148）

Codex 0.167 起线程记录的 provider id 与当前不一致即只读，官方路由（内置 openai）与第三方路由（自定义表）来回切换会把会话历史劈成两组。上游改为：统一会话历史关闭、写入后盘上有 ChatGPT 登录、且行里没有内置 openai 表达不了的客户端设置时，第三方路由不再写自定义表，而是与官方路由同形——不写 `model_provider`，顶层 `openai_base_url` 指向本地代理，会话全部落在 openai 桶；代理负责换上第三方 Key、剥掉 ChatGPT 身份。

本分支合并后该逻辑经由既有常量 `ROUTE_ID = crate::zhishu::identity::CODEX_ROUTE` 生效：无登录、统一历史开启或行内含客户端设置时仍写智枢 `zhishu` 表（ADR-0009 投影不变，不写上游 `custom` 表）；有登录时写内置 openai 形态，`zhishu` 表保留为休眠形态（占位 Key）。共存边界不变：本分支任何路径都不写 `custom` 表。

### 统一会话历史的每次启动迁入（上游 2c5e5d2c、6a0acf07、97e6d82a）

勾选「迁入既有官方会话」后不只跑一次：之后每次启动也把留在 openai 桶的会话迁入当前统一桶。为免每次启动全扫会话目录，先查 state DB 计数，没有 openai 线程直接跳过；state DB 不可读时退回目录标记。本分支保留多桶支持：标记按当前目标桶（`custom` 或 `zhishu`）分别匹配，`unified_history_target` 决定迁移目标，`migrate_*_to` 系列继续携带目标桶参数。state DB 不可读的兜底分支同时要求标记命中当前路由，避免换桶后误判已迁移。

### 其他上游行为变化

- `disable_response_storage` 全链移除（5f2f527d）：预设、模板、通用表单、深链、默认行不再写入，投影不再透出；保留在 `CODEX_FLOOR_TOP`，下次写入会清掉旧行里的该键。金标 `deeplink-codex` 快照随之更新。
- `model_catalog_json` 对绝对配置目录写绝对指针（13af7fc9）；派生目录文件名仍为本分支 `zhishu-ccs-model-catalog.json`（`CATALOG_FILENAME` 指向 `zhishu::identity::CODEX_CATALOG`），相对/UNC 目录保留裸文件名。
- 配置写入经软链接写穿（92ca821e）：`commit_staged` 先按调用方逻辑路径做配置历史留痕（应用归属与还原入口都认它），再解析软链接替换目标文件，链接本身保留。
- 切换结果契约增量扩展：`switch_provider` 返回在 `{ warnings, route? }` 上新增可选 `codexHistorySplit`，不改变既有字段形状。
- 界面密度/缩放、会话正文搜索（独立 `session-index.db`）、工具安装进度等新设置走既有 settings 序列化；数据库 schema 保持 24，无迁移。

## 数据、安全与兼容

保留智枢 0.0.9、独立数据目录、配置归属、代理占位符、Stack/Profile 身份、首写备份和 pending 恢复。平台 OAuth2、Key 生命周期、账号快照和两阶段同步契约不变；会话切分提示与统一迁移只在 Codex 本地文件上操作，不触碰平台目录或 Key。代理新增的「剥离 ChatGPT 身份头」只作用于未托管上游，托管路由行为由既有契约描述。官方额度保护默认开启的一次性迁移只改内存与随后的 settings 落盘，不改数据库。

本轮未升级数据库 schema；会话索引是独立 SQLite 文件，默认关闭。上游源命令变 async 或新增 IPC 时同步 facade、注册与测试，IPC 名称、参数、错误与返回形状按上游新增核对，未改动既有形状。

产品历史以当前 HEAD 为准，不重写历史，不处理 `origin/ccs-dev` 既有分歧（本轮起点相对该缓存领先 97、落后 26），不推送或发布。既有未跟踪 `.agents/`、`.dsh/`、`.omp/`、`.opencode/`、`.pi/`、`.zcode/`、`openspec/` 不纳入提交或清理。

## 验证及回退

执行 frozen install、TypeScript、Prettier、前端全量测试与 renderer；Rust fmt、locked/offline check、Clippy 和隔离全量库/集成回归，重点覆盖 Codex 路由契约、会话迁移、代理转发、金标、MCP、平台认证/目录/审计与共存。Rust 测试使用临时 `CC_SWITCH_TEST_HOME` 及已有配置覆盖，不改变智能体 HOME/CODEX_HOME；发布只跑本机模拟回归，不启动桌面应用或真实工具。

Windows/Linux、系统 keyring、真实 OAuth2、CLI 请求、安装升级及共存实机验收不以本机测试替代。结果及冲突见 [同步报告](../upstream-sync/2026-11-09.md)。失败可回到原产品提交 `a04f9406`；合并后通过 revert merge 第一父线回退代码。若以后实际运行并写入数据，须先备份并评估新路由形态/迁移行为，Git 回退不代替数据恢复。
