# 简易模式（应用开关）

> 2026-11-09：新增的外壳布局模式。零后端改动、零新 IPC；前端检查已执行，真实登录后的开关接入与 Windows/Linux 自绘窗口尚未实机验收（见文末）。

## 概述与入口

- 内容区右下角「简易模式 / 高级模式」按钮互切，偏好存 localStorage `cc-switch-shell-mode`（`src/lib/shellMode.ts`，仿 `interfacePrefs` 惯例），不进后端设置、不随云同步。
- 简易模式侧栏依次为「应用」、平台基础数据、MCP、Skills、提示词、会话、授权、用量统计和设置。各应用的供应商页从「应用」页点击应用名称进入。页面组件复用现有实现。
- `platformApps` 视图只在简易模式有效；`normalizeViewForMode`（`src/lib/navigation.ts`）在启动恢复与模式切换时把越界视图归一回供应商页。

## 「应用」页（`src/zhishu/models/components/PlatformAppsPage.tsx`）

- 列表 = 平台同步白名单 ∩ 可见应用（与「接入到工具」向导同源）；每行 = 应用图标与名称（点击进入该应用的供应商页）+ 副标题 + 行内分组/接入方式选择（模式应用，预填推荐默认值）+ 切换开关。
- 应用名下显示状态标签：绿色勾「已接入」、灰色插头「未接入」、琥珀色下载图标「未安装」。已接入时，标签旁显示当前生效的平台供应商和模型，或提示「未启用平台模型」；未安装时开关禁用。
- 开关初始状态 = 该应用存在当前登录账号的平台托管供应商（`meta.platformManaged` 的 origin/userId 匹配，见 `src/zhishu/models/platformManaged.ts`）。
- 开 = `provisionPlatformApp`（`src/zhishu/models/platformProvision.ts`），与「接入到工具」向导确认按钮同一套逻辑：模式应用按行内所选直连/路由/聚合应用，累加式加入全部平台供应商；成功后按应用刷新供应商与代理缓存并更新托盘。分组目录为空时模式应用开关禁用并提示先同步平台基础数据。
- 关 = 复用 `usePlatformProviderDeletion`（`platform_delete_providers`），只删当前账号在该应用的平台托管行，无确认弹窗；自建项、其他账号项、配置修改记录和恢复备份不受影响。
- 目录只读（`platform_get_catalog`）：页面不发平台请求、不创建/撤销 Key、不刷新额度，也不写 `platformUsage`/`platformCatalog` 缓存。

## 简易模式下的屏蔽范围

- 应用页页头（`AppPageHeader`）隐藏，替换为 `SimpleAppHeader`（`AppPageHeader.tsx` 新增导出：仅 52px 拖动区 + Windows/Linux 自绘窗口按钮），添加供应商、搜索、平台同步按钮和 ⋯ 菜单不再可见。
- 供应商列表只渲染当前账号平台托管行；没有托管行时显示指向「应用」页的提示，不渲染列表（避免「从配置导入」等管理入口）。
- 全局页（MCP、Skills、提示词、平台基础数据、会话、授权、用量统计）与设置页保持原样，页头照常。

## 与既有逻辑的关系

- `platformProvision.ts` 的单应用接入实现从 `src/zhishu/onboarding/PlatformOnboarding.tsx` 机械抽取，向导行为不变（回归测试未改断言）；分组数据读取 `fetchPlatformGroupData` 与默认分组解析 `pickPlatformGroupId` 为向导和应用页共用，安装检测共用 `usePlatformAppInstalls`。
- `isAccountManaged` / `listAccountManagedProviders` 是新代码的统一判定；既有调用点（向导、批量同步、用量页脚等）的内联写法保持原样，未做批量替换。

## 测试与未验收

- 已执行：`pnpm typecheck`、`pnpm format:check`（改动文件已格式化）、`pnpm test:unit`；新增 `tests/zhishu/components/PlatformAppsPage.test.tsx`（初始态、未安装禁用、开=向导逻辑、关=仅删托管行、推荐默认值）与 `tests/zhishu/lib/navigation.test.ts` 用例；`tests/zhishu/components/PlatformOnboarding.test.tsx` 回归通过。
- 未验收：真实登录后逐模式（直连/路由/聚合/累加）开关接入并实际启动受影响 CLI；关开后确认自建供应商与原生文件保留；Windows/Linux 自绘窗口按钮与拖动；简易模式偏好跨重启恢复的实机操作。
