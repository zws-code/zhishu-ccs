import { useSyncExternalStore } from "react";

/**
 * 外壳布局模式（高级 / 简易）。纯本机界面偏好：像侧栏开合、密度一样记在
 * localStorage，不进后端设置；右下角切换按钮读写。简易模式只改布局，
 * 不触发任何配置读写。
 */

export type ShellMode = "advanced" | "simple";

export const SHELL_MODE_STORAGE_KEY = "cc-switch-shell-mode";
export const DEFAULT_SHELL_MODE: ShellMode = "advanced";

function readShellMode(): ShellMode {
  try {
    const saved = localStorage.getItem(SHELL_MODE_STORAGE_KEY);
    if (saved === "simple" || saved === "advanced") return saved;
  } catch {
    // 读不到就用默认
  }
  return DEFAULT_SHELL_MODE;
}

let mode: ShellMode = readShellMode();
const listeners = new Set<() => void>();

function emit(next: ShellMode) {
  mode = next;
  listeners.forEach((listener) => listener());
}

export function setShellMode(next: ShellMode) {
  try {
    localStorage.setItem(SHELL_MODE_STORAGE_KEY, next);
  } catch {
    // 记不住也不影响这次切换
  }
  emit(next);
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useShellMode(): [ShellMode, (next: ShellMode) => void] {
  const current = useSyncExternalStore(subscribe, () => mode);
  return [current, setShellMode];
}
