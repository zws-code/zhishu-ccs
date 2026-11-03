import { StrictMode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import type { EventCallback, UnlistenFn } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTauriEvent } from "@/hooks/useTauriEvent";

const { listen, reportError } = vi.hoisted(() => ({
  listen: vi.fn<typeof import("@tauri-apps/api/event").listen>(),
  reportError: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({ listen }));
vi.mock("@/lib/frontendLogger", () => ({ reportFrontendError: reportError }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

const flush = () => act(async () => {});
const emit = (callback: EventCallback<unknown>, payload: unknown = null) =>
  act(() => {
    void callback({ event: "test-event", id: 1, payload });
  });

describe("useTauriEvent", () => {
  beforeEach(() => {
    listen.mockReset().mockResolvedValue(vi.fn());
    reportError.mockReset();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("subscribes only once during StrictMode effect replay", async () => {
    const off = vi.fn().mockResolvedValue(undefined);
    listen.mockResolvedValue(off);
    const { unmount } = renderHook(() => useTauriEvent("test-event", vi.fn()), {
      wrapper: StrictMode,
    });

    await flush();
    expect(listen).toHaveBeenCalledTimes(1);
    expect(off).not.toHaveBeenCalled();
    unmount();
    await flush();
    expect(off).toHaveBeenCalledTimes(1);
  });

  it("does not register an effect already disposed before setup starts", async () => {
    const { unmount } = renderHook(() => useTauriEvent("test-event", vi.fn()));
    unmount();
    await flush();
    expect(listen).not.toHaveBeenCalled();
  });

  it("uses the latest handler without resubscribing and ignores events after unmount", async () => {
    const first = vi.fn();
    const next = vi.fn();
    const { rerender, unmount } = renderHook(
      ({ handler }) => useTauriEvent("test-event", handler),
      { initialProps: { handler: first } },
    );
    await flush();
    const callback = listen.mock.calls[0][1];
    emit(callback, "first");
    rerender({ handler: next });
    emit(callback, "next");
    unmount();
    emit(callback, "late");

    expect(first).toHaveBeenCalledTimes(1);
    expect(first).toHaveBeenCalledWith("first");
    expect(next).toHaveBeenCalledTimes(1);
    expect(next).toHaveBeenCalledWith("next");
    expect(listen).toHaveBeenCalledTimes(1);
  });

  it("cleans up a subscription that resolves after unmount", async () => {
    const pending = deferred<UnlistenFn>();
    const off = vi.fn().mockResolvedValue(undefined);
    const handler = vi.fn();
    listen.mockReturnValue(pending.promise);
    const { unmount } = renderHook(() => useTauriEvent("test-event", handler));
    await flush();
    const callback = listen.mock.calls[0][1];
    unmount();
    emit(callback);
    pending.resolve(off);
    await flush();

    expect(off).toHaveBeenCalledTimes(1);
    expect(handler).not.toHaveBeenCalled();
  });

  it("disposes the old event independently when the event name changes", async () => {
    const pending = deferred<UnlistenFn>();
    const oldOff = vi.fn().mockResolvedValue(undefined);
    const newOff = vi.fn().mockResolvedValue(undefined);
    listen.mockReturnValueOnce(pending.promise).mockResolvedValueOnce(newOff);
    const handler = vi.fn();
    const { rerender, unmount } = renderHook(
      ({ name }) => useTauriEvent(name, handler),
      { initialProps: { name: "old-event" } },
    );
    await flush();
    const oldCallback = listen.mock.calls[0][1];
    rerender({ name: "new-event" });
    await flush();
    pending.resolve(oldOff);
    await flush();
    emit(oldCallback, "stale");
    emit(listen.mock.calls[1][1], "current");

    expect(oldOff).toHaveBeenCalledTimes(1);
    expect(newOff).not.toHaveBeenCalled();
    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith("current");
    unmount();
    await flush();
    expect(newOff).toHaveBeenCalledTimes(1);
  });

  it("reports registration failures with the event name", async () => {
    const error = new Error("registration failed");
    listen.mockRejectedValue(error);
    renderHook(() => useTauriEvent("test-event", vi.fn()));
    await flush();
    expect(reportError).toHaveBeenCalledWith(
      "tauri_event_listen:test-event",
      error,
    );
  });

  it.each(["throw", "reject"] as const)(
    "handles a handler that %ss",
    async (failure) => {
      const error = new Error("handler failed");
      const handler = vi.fn(() => {
        if (failure === "throw") throw error;
        return Promise.reject(error);
      });
      renderHook(() => useTauriEvent("test-event", handler));
      await flush();
      emit(listen.mock.calls[0][1]);
      await flush();
      expect(reportError).toHaveBeenCalledWith(
        "tauri_event_handler:test-event",
        error,
      );
    },
  );

  it.each(["throw", "reject"] as const)(
    "handles an unlisten that %ss without retrying unrelated errors",
    async (failure) => {
      const error = new Error("IPC unavailable");
      const off = vi.fn(() => {
        if (failure === "throw") throw error;
        return Promise.reject(error);
      });
      listen.mockResolvedValue(off);
      const { unmount } = renderHook(() =>
        useTauriEvent("test-event", vi.fn()),
      );
      await flush();
      unmount();
      await flush();
      expect(off).toHaveBeenCalledTimes(1);
      expect(reportError).toHaveBeenCalledWith(
        "tauri_event_unlisten:test-event",
        error,
      );
    },
  );

  it("handles rejected cleanup after a late registration", async () => {
    const pending = deferred<UnlistenFn>();
    const error = new Error("unlisten failed");
    const off = vi.fn().mockRejectedValue(error);
    listen.mockReturnValue(pending.promise);
    const { unmount } = renderHook(() => useTauriEvent("test-event", vi.fn()));
    await flush();
    unmount();
    pending.resolve(off);
    await flush();
    expect(reportError).toHaveBeenCalledWith(
      "tauri_event_unlisten:test-event",
      error,
    );
  });

  it("retries the WebView registration race and actually releases the listener", async () => {
    vi.useFakeTimers();
    // Tauri 2.10 resolves listen before its WebView eval has necessarily installed
    // the entry. unregisterListener then reads missingEntry.handlerId and rejects
    // before the backend unlisten command runs.
    const entries: Record<number, { handlerId: number }> = {};
    const callbacks = new Set([42]);
    const backendListeners = new Set([1]);
    const off = vi.fn(async () => {
      callbacks.delete(entries[1].handlerId);
      backendListeners.delete(1);
    });
    const pending = deferred<UnlistenFn>();
    listen.mockReturnValue(pending.promise);
    const handler = vi.fn();
    const { unmount } = renderHook(() => useTauriEvent("test-event", handler));
    await flush();
    unmount();
    setTimeout(() => {
      entries[1] = { handlerId: 42 };
    }, 0);
    pending.resolve(off);
    await flush();
    expect(off).toHaveBeenCalledTimes(1);
    emit(listen.mock.calls[0][1]);
    await act(() => vi.runAllTimersAsync());

    expect(off).toHaveBeenCalledTimes(2);
    expect(callbacks.size).toBe(0);
    expect(backendListeners.size).toBe(0);
    expect(handler).not.toHaveBeenCalled();
    expect(reportError).not.toHaveBeenCalled();
  });

  it("bounds retries and reports a persistent handlerId failure", async () => {
    vi.useFakeTimers();
    const error = new TypeError(
      "Cannot read properties of undefined (reading 'handlerId')",
    );
    const off = vi.fn().mockRejectedValue(error);
    listen.mockResolvedValue(off);
    const { unmount } = renderHook(() => useTauriEvent("test-event", vi.fn()));
    await flush();
    unmount();
    await act(() => vi.runAllTimersAsync());

    expect(off).toHaveBeenCalledTimes(3);
    expect(reportError).toHaveBeenCalledTimes(1);
    expect(reportError).toHaveBeenCalledWith(
      "tauri_event_unlisten:test-event",
      error,
    );
    expect(vi.getTimerCount()).toBe(0);
  });
});
