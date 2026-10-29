// 账户目录键独立于 Hook，供认证 Context 与页面共同使用，避免相互导入和跨账户复用。
export const platformCatalogKeys = {
  account: (baseUrl: string, userId: number) =>
    ["platformCatalog", baseUrl, userId] as const,
};
