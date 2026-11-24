# ADR-0018：在 v4.0.33 重建分支上合并 v4.0.40，并为上游 WorkBuddy 加共存补丁

- 日期：2026-11-24
- 状态：已在 `reorg/upstream-4.0.40` 实施，本机自动验证逐提交通过；`ccs-dev`、`reorg/upstream-4.0.33` 与所有 tag 未改动，不构成主线替换或发布批准
- 等级：L3（WorkBuddy 原生配置归属、设置目录覆盖、平台同步迁移）
- 起点：`reorg/upstream-4.0.33` / `617b92497`；官方目标 v4.0.40 / 提交 `f19a9d4f4f08bda03106a7be87c77cbcdf55b019`
- 编号：主线已有 ADR-0015（v4.0.37）、ADR-0016（v4.0.40），本分支原 ADR-0015（v4.0.33 重建）改为 [ADR-0017](ADR-0017-merge-4.0.33-upstream-workbuddy.md)，本记录为 ADR-0018

## 背景

用户决定继续使用 v4.0.33 线性重建分支，而不是改用主线。主线 `ccs-dev` 在 2026-11-21、2026-11-24 分别合并了 v4.0.37、v4.0.40，期间没有新的二开功能提交，但保留了二开 WorkBuddy 适配；本分支要求 WorkBuddy 只用上游实现。

原样使用上游 WorkBuddy 时与原版 CC Switch 共用 `~/.workbuddy/models.json` 和同一个归属字段 `ccSwitchProviderId`：两个产品会把对方写入的条目当成自己的去替换或删除。另外，旧版智枢（0.0.x，二开适配器）写入的平台条目没有归属标记，上游会把它们当作用户条目永久保留，并拒绝再写入同名模型。

## 决策

1. 新分支 `reorg/upstream-4.0.40` 从 `reorg/upstream-4.0.33` 出发，`git merge v4.0.40` 生成一个 merge 提交，不 rebase、不改写前 8 个提交。非 WorkBuddy 的冲突与自动合并结果对齐主线 3231ad64f 的取舍（schema 27、AppSpec 兼容导出、代理/Skills/测试拆分、Grok 账号、更新 IPC 兼容、德语、保留上游删除的 `claude.yml` 与 `ui/tabs` 等）；WorkBuddy 只用上游实现，merge 提交中上游 `workbuddy_config.rs` 与 v4.0.40 字节一致。
2. 单独的 `fix(workbuddy)` 提交加入共存补丁，两处改法与主线相同：
   - 上游 `workbuddy_config.rs` 的 `OWNER_FIELD` 改为 `zhishuCcsProviderId`，原版 `ccSwitchProviderId` 条目不再被智枢接管；
   - `data_dir()` 依次读设置页目录覆盖（`settings::get_override_dir(&AppType::WorkBuddy)`，随之恢复 `workbuddy_config_dir` 设置、目录设置项与文案）、`WORKBUDDY_CONFIG_DIR`、默认 `~/.workbuddy`。
3. 同一提交在 `src-tauri/src/zhishu/integrations/ccswitch/workbuddy_migration.rs` 加一次性认领：平台同步 WorkBuddy 时，没有任何归属字段、且 `url`（忽略 `/chat/completions` 后缀和末尾斜杠）与 `apiKey` 都和当前账号某个平台供应商一致的条目，补上 `zhishuCcsProviderId`。只改归属字段，不增删条目；随后的上游写入按归属整体替换（同名重复条目由此收敛为一条）。用户自建条目和原版 `ccSwitchProviderId` 条目一律不动；认领失败只记警告，不阻塞同步。参考树与 4.0.33 分支用过 `ccSwitchProviderId`，但从未发布，不做迁移。

## 备选方案

| 方案 | 结论 |
| --- | --- |
| 直接切到主线 | 用户已决定继续用重建分支；主线保留二开 WorkBuddy |
| 保持上游原样 `ccSwitchProviderId` | 与原版 CC Switch 共用同一文件时互相覆盖，违反共存要求 |
| 把认领放进上游 `workbuddy_config.rs` | 增加上游补丁面；放在 `src-tauri/src/zhishu/` 即可在同步前完成 |
| 认领所有无标记条目 | 会吞掉用户自建条目；只按当前账号平台凭据匹配 |

## 风险

- 认领依赖旧条目的 `url`/`apiKey` 与当前或历史平台供应商一致；Key 已轮换且数据库中也没有旧行时，旧条目会作为用户条目保留，需用户手动清理。
- 上游仍禁止同一模型 ID 属于两张卡：用户自建或原版 CC Switch 条目与平台模型同名时，启用会被拒绝。
- 设置页目录覆盖与 `WORKBUDDY_CONFIG_DIR` 同时存在时以设置页为准。

## 验证与回退

merge、fix、docs 三个提交均执行 8 项全量检查与 `pnpm test:release`、`pnpm test:release-cos`，见 [验证记录](../history/validation.md)。新增回归：旧未标记条目、用户条目、原版条目、已归属条目、重复模型 ID 的认领单测，以及一次经平台同步与启用的端到端用例。未验证真实 WorkBuddy 桌面端、真实平台账号与双产品实机共存。回退：不采用新分支即可；若实际运行过，智枢写入的条目带 `zhishuCcsProviderId`，回退到上游原样实现后会被视为用户条目，需要手动清理。
