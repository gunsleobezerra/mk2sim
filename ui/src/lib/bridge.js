/**
 * Thin transport layer between the UI and the Rust backend.
 *
 * Falls back to the in-browser simulator when `window.__TAURI__` is absent so the
 * replica can be developed and demoed without the native shell.
 */

import { simulator } from "./simulator.js";

const tauri = globalThis.__TAURI__;
const tauriInvoke = tauri?.core?.invoke ?? tauri?.tauri?.invoke;
const tauriListen = tauri?.event?.listen;

export const isTauri = typeof tauriInvoke === "function";

export async function invoke(command, args = {}) {
  if (!isTauri) return simulator.invoke(command, args);
  return tauriInvoke(command, args);
}

/**
 * Subscribe to a backend event. Always resolves to an unsubscribe function.
 */
export async function listen(event, callback) {
  if (!isTauri || typeof tauriListen !== "function") {
    const unsubscribe = simulator.subscribe(callback);
    return () => unsubscribe();
  }
  const unlisten = await tauriListen(event, (message) => callback(message.payload));
  return () => {
    try {
      unlisten();
    } catch {
      /* already disposed */
    }
  };
}

export { simulator };
