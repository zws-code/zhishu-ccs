# ADR-0015：合并上游 v4.0.33，改用上游 WorkBuddy，并在 v4.0.33 上线性重建历史

- 日期：2026-11-17
- 状态：已在新分支 `reorg/upstream-4.0.33` 实施，本机自动验证逐提交通过；`ccs-dev` 未改动，不构成合并或发布批准
- 等级：L3（WorkBuddy 原生配置、Provider 分发、schema 迁移、平台同步适配、历史重建）
- 产品起点：`ccs-dev` / `56cba5065`，0.0.11（上游 v4.0.25 + 自有提交 + v4.0.26/28/29 合并）
- 官方目标：v4.0.33；tag 对象 `ee5b0d3968f42380dfd46fcac5abe11ffbb88562`，提交 `86e23b6331b9ce2cee314d1ebb5b186da0de645a`

## 背景

上游 v4.0.31 起原生支持 WorkBuddy，与二开 WorkBuddy 使用同一应用 ID `workbuddy`、同一文件 `~/.workbuddy/models.json`，但归属模型不同（上游在条目上写 `ccSwitchProviderId`，二开按 url + apiKey 识别）。两套实现同时存在会让同一文件被两种规则改写，枚举与分发也会重复。另外，既有 `ccs-dev` 历史是上游 v4.0.25 之上按文件拆分的整理结果，中间提交从未单独构建，且功能提交与上游修改有混合。

## 决策

1. 删除二开 WorkBuddy 的全部代码、接线和测试（包括历史中），采用上游 WorkBuddy 实现，上游 WorkBuddy 文件保持与 v4.0.33 字节一致。
2. 保留平台供应商同步对 `workbuddy` 的支持：平台适配器生成上游格式的供应商配置（共享 `vendor` / `url` / `apiKey`，`url` 为完整的 `/chat/completions` 地址，模型条目不含 Key），写入、删除、启用全部经上游 `ProviderService` → `workbuddy_config`。两个平台白名单仍包含 `workbuddy`。
3. 以 v4.0.33 为唯一基点线性重建自有历史：上游修改（身份、发布、核心补丁/共存）与各功能（配置历史、平台登录、平台目录、简易模式）分属不同提交，每个提交都能构建并通过全部检查；最终代码树等于在 `ccs-dev` 上合并 v4.0.33 得到的参考树。

## 备选方案

| 方案 | 结论 |
| --- | --- |
| 保留二开 WorkBuddy，屏蔽上游实现 | 需要长期修改上游 WorkBuddy 的十余个模块，违背“上游最小补丁”，且两套归属规则会互相破坏 `models.json` |
| 两套并存（不同应用 ID） | 同一桌面应用出现两个入口、共用一个文件，用户无法判断由谁管理 |
| 在 `ccs-dev` 上直接合并 v4.0.33 | 可用作参考树，但会把二开 WorkBuddy 保留在历史中，也无法让每个提交单独构建 |
| 按文件整理（沿用 2026-11-03 做法） | 上游文件中的功能钩子（配置历史包装、平台元数据等）会混进“上游修改”提交，且中间提交无法通过检查 |

## 影响与风险

- 设置页不再提供 WorkBuddy 目录覆盖，只认 `WORKBUDDY_CONFIG_DIR` 与 `~/.workbuddy`。
- “是否安装”改为与上游应用页一致的数据目录检测，不再识别仅装了 macOS 应用包但从未运行的情况。
- 二开写入且没有 `ccSwitchProviderId` 的旧条目会被视为用户条目保留；同名模型可能与新写入冲突（上游拒绝同一模型属于两张卡）。需真实环境复核并在发布说明中提示。
- 平台 WorkBuddy 的 `url` 从 `<root>/v1` 改为 `<root>/v1/chat/completions`，与上游预设一致；旧托管行在下次同步时自动刷新。
- schema 26 由上游引入（`enabled_workbuddy`），旧版本无法读取升级后的数据库。
- 平台登录提交（中间状态）不含目录同步与账号清理：这是拆分产物，只在历史中存在，最终树行为不变。

## 验证与回退

逐提交执行 frozen install、renderer 构建、类型、格式、前端全量、Rust fmt/Clippy 与隔离 `CC_SWITCH_TEST_HOME` 下全量 Rust 测试；最终分支与参考树除 `docs/`、`AGENTS.md` 外 `git diff --exit-code` 无差异。结果见 [验证记录](../history/validation.md)，冲突与映射见 [同步报告](../upstream-sync/2026-11-17.md) 与 [提交整理记录](../history/commit-reorganization.md)。

未验证：真实 WorkBuddy 桌面端、真实平台账号、真实 keyring、应用启动、打包与升级。回退方式是不采用新分支（`ccs-dev` 与所有 tag 未改动）。
