# Monica Server WebUI

Original WebUI prototype for Monica's self-hosted backup service. It is a Vue 3 + Vite app using `@m3e/web` Material 3 Expressive Web Components.

Vue uses the `m3e-*` components directly as custom elements, so there is no wrapper layer, router, or state library in this prototype.

## Structure

- `src/lib/modules.ts` registers feature modules. Add, remove, or disable a WebUI section there.
- `src/i18n/en.ts` and `src/i18n/zh-CN.ts` keep translations as independent files.
- `src/lib/theme.ts` owns light, dark, auto, and palette presets.
- `src/components/` contains only reusable UI pieces with clear payoff: table, project detail, and appearance settings.

## Run

```bash
npm install
npm run dev
```

The dev server listens on:

```text
http://127.0.0.1:5173/
```

## Build

```bash
npm run build
```

The production bundle is emitted to `dist/`.

## Backend Status

The current UI uses mock data from `src/lib/api.ts`. Replace `loadServerSnapshot()` with real API calls when the backend is ready.

Reserved API names:

```text
POST /api/v1/auth/register
POST /api/v1/auth/login
POST /api/v1/devices
GET  /api/v1/vaults
POST /api/v1/vaults/{vaultId}/backups
GET  /api/v1/vaults/{vaultId}/backups/{backupId}/download
POST /api/v1/vaults/{vaultId}/sync/bundles
```

The server should store encrypted Monica JSON, MDBX files, and future sync bundles without decrypting vault contents. The WebUI manages accounts, devices, backups, audit events, retention policy, and endpoint settings.
