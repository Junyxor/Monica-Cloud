/**
 * Decrypted, in-memory vault. Fetches incremental sync from the server,
 * decrypts with the account key and exposes reactive lists to the views.
 */

import { reactive } from "vue";
import { ApiError, type AttachmentRecord, type FolderRecord, type ItemRecord, type SendRecord, type SendWrite, sendShareUrl } from "./client";
import * as mc from "./crypto";
import { emptyPayload, type ItemPayload } from "./items";
import { client, onLock, userKey } from "./session";

export interface VaultItem {
  id: string;
  type: number;
  folderId: string | null;
  favorite: boolean;
  trashed: boolean;
  archived: boolean;
  creationDate: string;
  revisionDate: string;
  attachments: AttachmentRecord[];
  payload: ItemPayload;
  itemKeyRaw: Uint8Array<ArrayBuffer> | null;
  /** Set when the item could not be decrypted (e.g. written by a newer client). */
  error?: string;
}

export interface VaultFolder {
  id: string;
  name: string;
}

export interface VaultSend {
  record: SendRecord;
  name: string;
  notes: string;
  text: string;
  fileName: string;
  shareUrl: string;
}

export interface ItemDraft {
  id?: string;
  type: number;
  folderId: string | null;
  favorite: boolean;
  archived: boolean;
  payload: ItemPayload;
}

export class ConflictError extends Error {
  constructor(readonly server: VaultItem) {
    super("conflict");
  }
}

export const vault = reactive({
  items: [] as VaultItem[],
  folders: [] as VaultFolder[],
  sends: [] as VaultSend[],
  revisionDate: "",
  syncing: false,
  lastSyncAt: null as Date | null,
  error: ""
});

onLock(() => {
  vault.items = [];
  vault.folders = [];
  vault.sends = [];
  vault.revisionDate = "";
  vault.lastSyncAt = null;
  vault.error = "";
});

function key(): CryptoKey {
  if (!userKey.value) throw new Error("Vault is locked");
  return userKey.value.key;
}

async function decryptItem(record: ItemRecord): Promise<VaultItem> {
  const base = {
    id: record.id,
    type: record.type,
    folderId: record.folderId,
    favorite: record.favorite,
    trashed: Boolean(record.deletedDate),
    archived: Boolean(record.archivedDate),
    creationDate: record.creationDate,
    revisionDate: record.revisionDate,
    attachments: record.attachments
  };
  try {
    const { payload, itemKeyRaw } = await mc.decryptItem<ItemPayload>(key(), record);
    return { ...base, payload: { ...emptyPayload(), ...payload }, itemKeyRaw };
  } catch (error) {
    return { ...base, payload: { ...emptyPayload(), name: "—" }, itemKeyRaw: null, error: String(error) };
  }
}

async function decryptFolder(record: FolderRecord): Promise<VaultFolder> {
  try {
    return { id: record.id, name: await mc.decryptString(key(), record.name) };
  } catch {
    return { id: record.id, name: "—" };
  }
}

async function decryptSend(record: SendRecord): Promise<VaultSend> {
  const sendKey = await mc.decryptBytes(key(), record.key);
  const { contentKey } = await mc.deriveSendKeys(sendKey);
  const dec = (value: string | null) => (value ? mc.decryptString(contentKey, value) : Promise.resolve(""));
  return {
    record,
    name: await dec(record.name),
    notes: await dec(record.notes),
    text: await dec(record.text),
    fileName: await dec(record.fileName),
    shareUrl: sendShareUrl(location.origin + location.pathname, record.accessId, sendKey)
  };
}

function upsert<T extends { id: string }>(list: T[], value: T) {
  const index = list.findIndex((x) => x.id === value.id);
  if (index >= 0) list.splice(index, 1, value);
  else list.push(value);
}

function sortFolders() {
  vault.folders.sort((a, b) => a.name.localeCompare(b.name));
}

let pending: Promise<void> | null = null;

/** Pull changes since the last sync (or everything on first run). */
export function syncVault(): Promise<void> {
  if (pending) return pending;
  pending = (async () => {
    vault.syncing = true;
    vault.error = "";
    try {
      const sync = await client.sync(vault.revisionDate || undefined);
      const [items, folders, sends] = await Promise.all([
        Promise.all(sync.items.map(decryptItem)),
        Promise.all(sync.folders.map(decryptFolder)),
        Promise.all(sync.sends.map((s) => decryptSend(s).catch(() => null)))
      ]);
      if (sync.full) {
        vault.items = items;
        vault.folders = folders;
        vault.sends = sends.filter((s): s is VaultSend => s !== null);
      } else {
        for (const item of items) upsert(vault.items, item);
        for (const folder of folders) upsert(vault.folders, folder);
        for (const send of sends) if (send) upsertSend(send);
        for (const gone of sync.deleted) {
          if (gone.type === "item") vault.items = vault.items.filter((i) => i.id !== gone.id);
          if (gone.type === "folder") vault.folders = vault.folders.filter((f) => f.id !== gone.id);
          if (gone.type === "send") vault.sends = vault.sends.filter((s) => s.record.id !== gone.id);
        }
      }
      sortFolders();
      vault.revisionDate = sync.revisionDate;
      vault.lastSyncAt = new Date();
    } catch (error) {
      vault.error = error instanceof Error ? error.message : String(error);
      throw error;
    } finally {
      vault.syncing = false;
      pending = null;
    }
  })();
  return pending;
}

/** Cheap check used by the background poll and window focus. */
export async function syncIfChanged() {
  if (!userKey.value || vault.syncing) return;
  const { revisionDate } = await client.revisionDate();
  if (revisionDate !== vault.revisionDate) await syncVault();
}

function upsertSend(send: VaultSend) {
  const index = vault.sends.findIndex((s) => s.record.id === send.record.id);
  if (index >= 0) vault.sends.splice(index, 1, send);
  else vault.sends.unshift(send);
}

/** Record a write we made ourselves without waiting for the next sync. */
async function applyItem(record: ItemRecord) {
  const item = await decryptItem(record);
  upsert(vault.items, item);
  return item;
}

// Items ---------------------------------------------------------------------

export async function saveItem(draft: ItemDraft, base?: VaultItem, force = false): Promise<VaultItem> {
  const encrypted = await mc.encryptItem(key(), draft.payload, base?.itemKeyRaw ?? undefined);
  const body = {
    type: draft.type,
    folderId: draft.folderId,
    favorite: draft.favorite,
    archived: draft.archived,
    key: encrypted.key,
    data: encrypted.data,
    lastKnownRevisionDate: force ? undefined : base?.revisionDate
  };
  try {
    const record = draft.id ? await client.updateItem(draft.id, body) : await client.createItem(body);
    return applyItem(record);
  } catch (error) {
    if (error instanceof ApiError && error.status === 409 && error.current) {
      const server = await applyItem(error.current as ItemRecord);
      throw new ConflictError(server);
    }
    throw error;
  }
}

export async function setFavorite(item: VaultItem, favorite: boolean) {
  return saveItem({ id: item.id, type: item.type, folderId: item.folderId, favorite, archived: item.archived, payload: item.payload }, item);
}

export async function setArchived(item: VaultItem, archived: boolean) {
  return saveItem({ id: item.id, type: item.type, folderId: item.folderId, favorite: item.favorite, archived, payload: item.payload }, item);
}

export async function trashItem(item: VaultItem) {
  return applyItem(await client.trashItem(item.id));
}

export async function restoreItem(item: VaultItem) {
  return applyItem(await client.restoreItem(item.id));
}

export async function deleteItem(item: VaultItem) {
  await client.deleteItem(item.id);
  vault.items = vault.items.filter((i) => i.id !== item.id);
}

export async function emptyTrash() {
  const ids = vault.items.filter((i) => i.trashed).map((i) => i.id);
  for (let i = 0; i < ids.length; i += 1000) await client.bulk("delete", ids.slice(i, i + 1000));
  vault.items = vault.items.filter((i) => !i.trashed);
}

// Attachments ---------------------------------------------------------------

async function itemKey(item: VaultItem) {
  if (!item.itemKeyRaw) throw new Error("Item key unavailable");
  return mc.importAesKey(item.itemKeyRaw);
}

export async function uploadAttachment(item: VaultItem, file: File) {
  const content = new Uint8Array(await file.arrayBuffer());
  return applyItem(await client.uploadAttachment(item.id, await itemKey(item), file.name, content));
}

export async function downloadAttachment(item: VaultItem, attachment: AttachmentRecord) {
  const { fileName, content } = await client.downloadAttachment(item.id, await itemKey(item), attachment);
  saveFile(fileName, content);
}

export async function deleteAttachment(item: VaultItem, attachment: AttachmentRecord) {
  return applyItem(await client.deleteAttachment(item.id, attachment.id));
}

export async function attachmentName(item: VaultItem, attachment: AttachmentRecord) {
  try {
    return await mc.decryptString(await itemKey(item), attachment.fileName);
  } catch {
    return "—";
  }
}

export function saveFile(fileName: string, content: Uint8Array<ArrayBuffer>) {
  const url = URL.createObjectURL(new Blob([content]));
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// Folders -------------------------------------------------------------------

export async function createFolder(name: string) {
  const record = await client.createFolder(await mc.encryptString(key(), name));
  upsert(vault.folders, await decryptFolder(record));
  sortFolders();
  return record.id;
}

export async function renameFolder(id: string, name: string) {
  const record = await client.updateFolder(id, await mc.encryptString(key(), name));
  upsert(vault.folders, await decryptFolder(record));
  sortFolders();
}

export async function deleteFolder(id: string) {
  await client.deleteFolder(id);
  vault.folders = vault.folders.filter((f) => f.id !== id);
  for (const item of vault.items) if (item.folderId === id) item.folderId = null;
}

// Send ----------------------------------------------------------------------

export interface SendDraft {
  name: string;
  notes: string;
  text: string;
  file: File | null;
  password: string;
  maxAccessCount: number | null;
  deletionDays: number;
  expirationDays: number | null;
  hideEmail: boolean;
}

export async function createSend(draft: SendDraft): Promise<VaultSend> {
  const { sendKey, contentKey } = await mc.deriveSendKeys();
  const now = Date.now();
  const body: SendWrite = {
    type: draft.file ? 1 : 0,
    key: await mc.encryptBytes(key(), sendKey),
    name: await mc.encryptString(contentKey, draft.name || draft.file?.name || "Send"),
    notes: draft.notes ? await mc.encryptString(contentKey, draft.notes) : null,
    passwordHash: draft.password ? await mc.sendPasswordHash(draft.password, sendKey) : null,
    maxAccessCount: draft.maxAccessCount,
    deletionDate: new Date(now + draft.deletionDays * 86_400_000).toISOString(),
    expirationDate: draft.expirationDays ? new Date(now + draft.expirationDays * 86_400_000).toISOString() : null,
    hideEmail: draft.hideEmail
  };
  let encrypted: Uint8Array<ArrayBuffer> | null = null;
  if (draft.file) {
    encrypted = await mc.encryptBlob(contentKey, new Uint8Array(await draft.file.arrayBuffer()));
    body.fileName = await mc.encryptString(contentKey, draft.file.name);
    body.size = encrypted.length;
  } else {
    body.text = await mc.encryptString(contentKey, draft.text);
  }
  let record: SendRecord = await client.createSend(body);
  if (encrypted) record = await client.uploadSendFile(record.id, encrypted);
  const send = await decryptSend(record);
  upsertSend(send);
  return send;
}

export async function setSendDisabled(send: VaultSend, disabled: boolean) {
  const r = send.record;
  const record = await client.updateSend(r.id, {
    type: r.type,
    key: r.key,
    name: r.name,
    notes: r.notes,
    text: r.text,
    fileName: r.fileName,
    size: r.size,
    maxAccessCount: r.maxAccessCount,
    expirationDate: r.expirationDate,
    deletionDate: r.deletionDate,
    hideEmail: r.hideEmail,
    disabled
  });
  upsertSend(await decryptSend(record));
}

export async function deleteSend(send: VaultSend) {
  await client.deleteSend(send.record.id);
  vault.sends = vault.sends.filter((s) => s.record.id !== send.record.id);
}
