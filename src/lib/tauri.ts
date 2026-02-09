import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// SSR-safe check for Tauri environment
// Tauri v2 uses __TAURI_INTERNALS__, v1 used __TAURI__
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// SSR-safe invoke wrapper
export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    console.warn(`Tauri invoke '${cmd}' called outside Tauri context`);
    throw new Error('Not in Tauri context');
  }
  return invoke<T>(cmd, args);
}

// SSR-safe listen wrapper
export async function tauriListen<T>(
  event: string,
  handler: (payload: T) => void
): Promise<UnlistenFn> {
  if (!isTauri()) {
    return () => {};
  }
  return listen<T>(event, (e) => handler(e.payload));
}
