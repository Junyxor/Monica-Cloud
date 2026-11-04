# Monica Cloud WebUI

Vue 3 + Vite web vault built with [`@m3e/web`](https://github.com/matraic/m3e) Material 3
Expressive components. It decrypts the vault in the browser with WebCrypto; the server only sees
ciphertext.

## Structure

```text
src/
├── lib/
│   ├── crypto.ts     protocol §1: PBKDF2/HKDF, EncString, chunked file encryption, Send keys
│   ├── client.ts     typed API client (also used by scripts/e2e.ts in Node)
│   ├── session.ts    login / unlock / lock / auto-lock; tokens in sessionStorage, keys in memory only
│   ├── vault.ts      decrypted reactive vault, incremental sync, item/folder/attachment/Send actions
│   ├── items.ts      item types, payload schema, field descriptors, password generator
│   ├── totp.ts       RFC 6238 codes
│   ├── router.ts     hash routes (#/vault/…, #/send, #/send/{accessId}/{key}, #/admin/…)
│   ├── ui.ts         toasts, confirm dialog, clipboard with auto-clear
│   └── theme.ts      palettes and light/dark
├── views/            Auth, Unlock, AppShell, Vault, Send, Receive, Devices, Activity, Settings, Admin*
├── components/       ItemDetail, ItemEditor, ItemIcon, TotpCode, PasswordInput, dialogs
└── i18n/             en.ts, zh-CN.ts (zh-CN is type-checked against en)
```

The detail view and the editor are both generated from `typeFields` in `lib/items.ts`, so a new
field only needs a descriptor there plus its two translations.

## Develop

```bash
npm install
npm run dev          # http://127.0.0.1:5173, proxies /api to MONICA_API (default http://127.0.0.1:8000)
npm run typecheck
npm run build        # dist/, served by the server via WEB_VAULT_FOLDER
npm run e2e          # protocol test against MONICA_URL (default http://127.0.0.1:8000)
```

Fonts (Roboto, Material Symbols) are bundled locally; the app makes no third-party requests,
which keeps the strict Content-Security-Policy sent by the server intact.
