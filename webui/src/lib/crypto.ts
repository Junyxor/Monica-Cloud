/**
 * Client-side cryptography for Monica Cloud protocol v1 (docs/protocol.zh-CN.md §1).
 *
 * Everything here runs in the browser (or Node) with WebCrypto only. The server
 * never sees the master password, masterKey, wrapKey, userKey or item keys.
 * This file has no imports so Node can run it directly for end-to-end tests.
 */

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const subtle = globalThis.crypto.subtle;

type Bytes = Uint8Array<ArrayBuffer>;

export const KDF_PBKDF2 = 0;
export const ENC_VERSION = "1";
export const BLOB_MAGIC = "MCB1";
export const BLOB_CHUNK_SIZE = 64 * 1024;
const GCM_IV = 12;
const GCM_TAG = 16;

// ---------------------------------------------------------------------------
// Encoding helpers

export function b64encode(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

export function b64decode(value: string): Bytes {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export function b64urlEncode(bytes: Uint8Array): string {
  return b64encode(bytes).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function b64urlDecode(value: string): Bytes {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/");
  return b64decode(padded + "=".repeat((4 - (padded.length % 4)) % 4));
}

export function randomBytes(length: number): Bytes {
  const bytes = new Uint8Array(length);
  // getRandomValues is limited to 65536 bytes per call.
  for (let i = 0; i < length; i += 65536) {
    globalThis.crypto.getRandomValues(bytes.subarray(i, Math.min(i + 65536, length)));
  }
  return bytes;
}

export function utf8(value: string): Bytes {
  return encoder.encode(value) as Bytes;
}

// ---------------------------------------------------------------------------
// KDF (§1.1)

async function pbkdf2(password: Bytes, salt: Bytes, iterations: number): Promise<Bytes> {
  const key = await subtle.importKey("raw", password, "PBKDF2", false, ["deriveBits"]);
  const bits = await subtle.deriveBits({ name: "PBKDF2", hash: "SHA-256", salt, iterations }, key, 256);
  return new Uint8Array(bits);
}

async function hkdf(ikm: Bytes, info: string): Promise<Bytes> {
  const key = await subtle.importKey("raw", ikm, "HKDF", false, ["deriveBits"]);
  const bits = await subtle.deriveBits(
    { name: "HKDF", hash: "SHA-256", salt: new Uint8Array(0), info: utf8(info) },
    key,
    256
  );
  return new Uint8Array(bits);
}

export function normalizeEmail(email: string): string {
  return email.trim().toLowerCase();
}

export interface MasterKeys {
  /** Sent to the server in place of the password. */
  authHash: string;
  /** Unwraps the account userKey. */
  wrapKey: CryptoKey;
}

export async function deriveMasterKeys(email: string, masterPassword: string, iterations: number): Promise<MasterKeys> {
  const password = utf8(masterPassword.normalize("NFC"));
  const masterKey = await pbkdf2(password, utf8(normalizeEmail(email)), iterations);
  const authHash = b64encode(await pbkdf2(masterKey, password, 1));
  const wrapKey = await importAesKey(await hkdf(masterKey, "monica-cloud/v1/wrap"));
  masterKey.fill(0);
  return { authHash, wrapKey };
}

// ---------------------------------------------------------------------------
// EncString (§1.4)

export async function importAesKey(raw: Bytes): Promise<CryptoKey> {
  if (raw.length !== 32) throw new Error("AES-256 keys must be 32 bytes");
  return subtle.importKey("raw", raw, "AES-GCM", false, ["encrypt", "decrypt"]);
}

export async function encryptBytes(key: CryptoKey, plaintext: Bytes): Promise<string> {
  const iv = randomBytes(GCM_IV);
  const ciphertext = new Uint8Array(await subtle.encrypt({ name: "AES-GCM", iv }, key, plaintext));
  return `${ENC_VERSION}.${b64encode(iv)}.${b64encode(ciphertext)}`;
}

export async function decryptBytes(key: CryptoKey, encString: string): Promise<Bytes> {
  const parts = encString.split(".");
  if (parts.length !== 3 || parts[0] !== ENC_VERSION) {
    throw new Error("Unsupported encryption format; update Monica Cloud WebUI");
  }
  const iv = b64decode(parts[1]);
  const ciphertext = b64decode(parts[2]);
  return new Uint8Array(await subtle.decrypt({ name: "AES-GCM", iv }, key, ciphertext));
}

export async function encryptString(key: CryptoKey, value: string): Promise<string> {
  return encryptBytes(key, utf8(value));
}

export async function decryptString(key: CryptoKey, encString: string): Promise<string> {
  return decoder.decode(await decryptBytes(key, encString));
}

export async function encryptJson(key: CryptoKey, value: unknown): Promise<string> {
  return encryptString(key, JSON.stringify(value));
}

export async function decryptJson<T>(key: CryptoKey, encString: string): Promise<T> {
  return JSON.parse(await decryptString(key, encString)) as T;
}

// ---------------------------------------------------------------------------
// Account key (§1.2) and item keys (§1.3)

export interface UserKey {
  raw: Bytes;
  key: CryptoKey;
}

export async function generateUserKey(): Promise<UserKey> {
  const raw = randomBytes(32);
  return { raw, key: await importAesKey(raw) };
}

export async function unwrapUserKey(wrapKey: CryptoKey, protectedUserKey: string): Promise<UserKey> {
  let raw: Bytes;
  try {
    raw = await decryptBytes(wrapKey, protectedUserKey);
  } catch {
    throw new Error("Master password is incorrect or the account key is damaged");
  }
  return { raw, key: await importAesKey(raw) };
}

export interface EncryptedItem {
  key: string;
  data: string;
}

/** Encrypt an item payload. Pass the existing item key to keep attachments readable. */
export async function encryptItem(userKey: CryptoKey, payload: unknown, itemKeyRaw?: Bytes): Promise<EncryptedItem & { itemKeyRaw: Bytes }> {
  const raw = itemKeyRaw ?? randomBytes(32);
  const itemKey = await importAesKey(raw);
  return { key: await encryptBytes(userKey, raw), data: await encryptJson(itemKey, payload), itemKeyRaw: raw };
}

export async function decryptItem<T>(userKey: CryptoKey, item: EncryptedItem): Promise<{ payload: T; itemKeyRaw: Bytes; itemKey: CryptoKey }> {
  const itemKeyRaw = await decryptBytes(userKey, item.key);
  const itemKey = await importAesKey(itemKeyRaw);
  return { payload: await decryptJson<T>(itemKey, item.data), itemKeyRaw, itemKey };
}

// ---------------------------------------------------------------------------
// Chunked binary encryption (§1.5)

function chunkAad(index: number, final: boolean): Bytes {
  const aad = new Uint8Array(9);
  new DataView(aad.buffer).setBigUint64(0, BigInt(index));
  aad[8] = final ? 1 : 0;
  return aad;
}

export function encryptedBlobSize(plainLength: number, chunkSize = BLOB_CHUNK_SIZE): number {
  const chunks = Math.max(1, Math.ceil(plainLength / chunkSize));
  return 8 + chunks * (GCM_IV + GCM_TAG) + plainLength;
}

export async function encryptBlob(key: CryptoKey, plaintext: Bytes, chunkSize = BLOB_CHUNK_SIZE): Promise<Bytes> {
  const out = new Uint8Array(encryptedBlobSize(plaintext.length, chunkSize));
  out.set(utf8(BLOB_MAGIC), 0);
  new DataView(out.buffer).setUint32(4, chunkSize);
  const chunks = Math.max(1, Math.ceil(plaintext.length / chunkSize));
  let offset = 8;
  for (let i = 0; i < chunks; i++) {
    const plain = plaintext.subarray(i * chunkSize, Math.min((i + 1) * chunkSize, plaintext.length));
    const iv = randomBytes(GCM_IV);
    const sealed = new Uint8Array(
      await subtle.encrypt({ name: "AES-GCM", iv, additionalData: chunkAad(i, i === chunks - 1) }, key, plain)
    );
    out.set(iv, offset);
    out.set(sealed, offset + GCM_IV);
    offset += GCM_IV + sealed.length;
  }
  return out;
}

export async function decryptBlob(key: CryptoKey, blob: Bytes): Promise<Bytes> {
  if (blob.length < 8 || decoder.decode(blob.subarray(0, 4)) !== BLOB_MAGIC) {
    throw new Error("Not a Monica Cloud encrypted file");
  }
  const chunkSize = new DataView(blob.buffer, blob.byteOffset, blob.byteLength).getUint32(4);
  if (chunkSize < 1024 || chunkSize > 16 * 1024 * 1024) throw new Error("Invalid chunk size");
  const sealedChunk = GCM_IV + chunkSize + GCM_TAG;
  const parts: Bytes[] = [];
  let offset = 8;
  let index = 0;
  let total = 0;
  while (offset < blob.length) {
    const end = Math.min(offset + sealedChunk, blob.length);
    const final = end === blob.length;
    const iv = blob.subarray(offset, offset + GCM_IV);
    const sealed = blob.subarray(offset + GCM_IV, end);
    let plain: Bytes;
    try {
      plain = new Uint8Array(await subtle.decrypt({ name: "AES-GCM", iv, additionalData: chunkAad(index, final) }, key, sealed));
    } catch {
      throw new Error("Encrypted file is damaged, truncated or reordered");
    }
    parts.push(plain);
    total += plain.length;
    offset = end;
    index++;
  }
  if (index === 0) throw new Error("Encrypted file is truncated");
  const out = new Uint8Array(total);
  let position = 0;
  for (const part of parts) {
    out.set(part, position);
    position += part.length;
  }
  return out;
}

// ---------------------------------------------------------------------------
// Send (§1.6)

export interface SendKeys {
  sendKey: Bytes;
  contentKey: CryptoKey;
}

export async function deriveSendKeys(sendKey: Bytes = randomBytes(16)): Promise<SendKeys> {
  return { sendKey, contentKey: await importAesKey(await hkdf(sendKey, "monica-cloud/v1/send")) };
}

export async function sendPasswordHash(password: string, sendKey: Bytes): Promise<string> {
  return b64encode(await pbkdf2(utf8(password), sendKey, 100_000));
}
