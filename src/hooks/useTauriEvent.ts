import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { reportFrontendError } from "@/lib/frontendLogger";

async function unsubscribe(eventName: string, off: UnlistenFn): Promise<void> {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      // UnlistenFn 的声明返回 void，但 Tauri 的实现返回 Promise。
      await off();
      return;
    } catch (error) {
      // Tauri 2.10 的 listen IPC 可能先于 WebView 注册脚本完成；此时
      // unregisterListener 读取尚不存在的条目的 handlerId，后端也未注销。
      // 仅对这个竞态让出事件循环后重试，其他错误保留诊断。
      const registrationPending =
        error instanceof TypeError && error.message.includes("handlerId");
      if (!registrationPending || attempt === 2) {
        reportFrontendError(`tauri_event_unlisten:${eventName}`, error);
        return;
      }
      await new Promise<void>((resolve) => setTimeout(resolve, 16));
    }
  }
}

/**
 * 在 useEffect 中监听 Tauri 事件，自动管理异步注册和卸载清理。
 * 避免每次使用时重复编写 active flag + async setup 样板代码。
 */
export function useTauriEvent<P>(
  eventName: string,
  handler: (payload: P) => void | Promise<void>,
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;

    void (async () => {
      // StrictMode 会同步执行 setup → cleanup → setup。等一个微任务，
      // 跳过已清理的那次 setup，避免发起随即又要取消的异步注册。
      await Promise.resolve();
      if (disposed) return;

      try {
        const off = await listen<P>(eventName, async (event) => {
          // 注销完成前或注册迟到时仍可能收到事件。
          if (disposed) return;
          try {
            await handlerRef.current(event.payload);
          } catch (error) {
            reportFrontendError(`tauri_event_handler:${eventName}`, error);
          }
        });
        if (disposed) {
          await unsubscribe(eventName, off);
        } else {
          unlisten = off;
        }
      } catch (error) {
        reportFrontendError(`tauri_event_listen:${eventName}`, error);
      }
    })();

    return () => {
      disposed = true;
      if (unlisten) {
        const off = unlisten;
        unlisten = undefined;
        void unsubscribe(eventName, off);
      }
    };
  }, [eventName]);
}
