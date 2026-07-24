# Monica Cloud

Self-hosted server and WebUI for Monica Pass.

This repository currently contains the WebUI prototype only. The backend service has not been implemented yet.

## Repository Layout

```text
.
├── webui/   Vue 3 + Vite WebUI
└── server/  Backend placeholder
```

## WebUI

```bash
cd webui
npm install
npm run dev
```

The development server listens on `http://127.0.0.1:5173/`.

Build the WebUI with:

```bash
cd webui
npm run build
```

## Backend Status

The server API is not written yet. The WebUI currently uses mock data from `webui/src/lib/api.ts`.

Planned backend responsibilities:

- Account login and session management
- Device registration and approval
- Encrypted backup upload and download
- MDBX sync endpoints
- Send management
- Organization, collection, member, and policy management
- Diagnostics and settings APIs

The service should store encrypted client data only. It must not decrypt Monica vault contents.

## License

AGPL-3.0. See `LICENSE`.
