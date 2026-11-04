/**
 * Login state. Tokens survive a reload (sessionStorage, this tab only) but the
 * decrypted account key lives in memory only, so a reload or inactivity locks
 * the vault until the master password is entered again.
 */

import { computed, ref, shallowRef } from "vue";
import { ApiError, MonicaClient, type ServerConfig, type TokenResponse } from "./client";
import type { UserKey } from "./crypto";

export type SessionState = "loading" | "offline" | "auth" | "locked" | "unlocked";

const SESSION_KEY = "monica.session";
const DEVICE_KEY = "monica.deviceId";
const EMAIL_KEY = "monica.lastEmail";
const AUTO_LOCK_MS = 15 * 60 * 1000;

export const client = new MonicaClient("");
export const serverConfig = ref<ServerConfig | null>(null);
export const sessionState = ref<SessionState>("loading");
export const userKey = shallowRef<UserKey | null>(null);
export const account = ref(client.session?.account ?? null);
export const isAdmin = computed(() => account.value?.role === "owner" || account.value?.role === "admin");
export const lastEmail = ref(safeGet(localStorage, EMAIL_KEY) ?? "");

const lockListeners: (() => void)[] = [];

/** Register cleanup (e.g. clearing decrypted vault data) that runs on lock and logout. */
export function onLock(listener: () => void) {
  lockListeners.push(listener);
}

client.onSession = (session) => {
  account.value = session?.account ?? null;
  if (session) safeSet(sessionStorage, SESSION_KEY, JSON.stringify(session));
  else {
    safeRemove(sessionStorage, SESSION_KEY);
    if (sessionState.value === "unlocked" || sessionState.value === "locked") {
      clearKeys();
      sessionState.value = "auth";
    }
  }
};

export async function initSession() {
  try {
    serverConfig.value = await client.config();
  } catch {
    sessionState.value = "offline";
    return;
  }
  const stored = safeGet(sessionStorage, SESSION_KEY);
  if (stored) {
    try {
      client.session = JSON.parse(stored) as TokenResponse;
      account.value = client.session.account;
      sessionState.value = "locked";
      return;
    } catch {
      safeRemove(sessionStorage, SESSION_KEY);
    }
  }
  sessionState.value = "auth";
}

function deviceInfo() {
  let identifier = safeGet(localStorage, DEVICE_KEY);
  if (!identifier) {
    identifier = crypto.randomUUID();
    safeSet(localStorage, DEVICE_KEY, identifier);
  }
  const ua = navigator.userAgent;
  const browser = /Edg\//.test(ua) ? "Edge" : /Firefox\//.test(ua) ? "Firefox" : /Chrome\//.test(ua) ? "Chrome" : /Safari\//.test(ua) ? "Safari" : "Browser";
  const os = /Windows/.test(ua) ? "Windows" : /Android/.test(ua) ? "Android" : /Mac OS X/.test(ua) ? "macOS" : /Linux/.test(ua) ? "Linux" : "";
  return { identifier, name: [browser, os].filter(Boolean).join(" · "), type: "web" as const, appVersion: "webui" };
}

export async function login(email: string, password: string) {
  userKey.value = await client.login(email.trim(), password, deviceInfo());
  lastEmail.value = email.trim();
  safeSet(localStorage, EMAIL_KEY, lastEmail.value);
  sessionState.value = "unlocked";
  touch();
}

export async function register(email: string, name: string, password: string) {
  const iterations = serverConfig.value?.kdfIterations ?? 600_000;
  await client.register(email.trim(), name.trim(), password, iterations);
  serverConfig.value = await client.config();
  await login(email, password);
}

export async function unlock(password: string) {
  userKey.value = await client.unlock(password);
  // Make sure the session is still valid before showing the vault.
  try {
    await client.revisionDate();
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      clearKeys();
      sessionState.value = "auth";
      throw error;
    }
  }
  sessionState.value = "unlocked";
  touch();
}

export function lock() {
  if (sessionState.value !== "unlocked") return;
  clearKeys();
  sessionState.value = "locked";
}

export async function logout() {
  clearKeys();
  try {
    await client.logout();
  } catch {
    // Already invalid on the server; local state is cleared either way.
  }
  sessionState.value = "auth";
}

/** Called after a password change or account deletion: the server already revoked every session. */
export function endSession() {
  clearKeys();
  client.session = null;
  safeRemove(sessionStorage, SESSION_KEY);
  sessionState.value = "auth";
}

function clearKeys() {
  userKey.value?.raw.fill(0);
  userKey.value = null;
  for (const listener of lockListeners) listener();
}

// Auto-lock after inactivity ------------------------------------------------

let lastActivity = Date.now();
function touch() {
  lastActivity = Date.now();
}
for (const event of ["pointerdown", "keydown", "wheel", "touchstart"]) {
  window.addEventListener(event, touch, { passive: true });
}
window.setInterval(() => {
  if (sessionState.value === "unlocked" && Date.now() - lastActivity > AUTO_LOCK_MS) lock();
}, 30_000);

// Storage access can throw in private mode or when site data is blocked.
function safeGet(storage: Storage, key: string) {
  try {
    return storage.getItem(key);
  } catch {
    return null;
  }
}
function safeSet(storage: Storage, key: string, value: string) {
  try {
    storage.setItem(key, value);
  } catch {
    /* ignore */
  }
}
function safeRemove(storage: Storage, key: string) {
  try {
    storage.removeItem(key);
  } catch {
    /* ignore */
  }
}
