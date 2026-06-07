# CLAUDE.md

## Project Overview

Rust web-based terminal (WebShell) with a Rust/Axum backend and Vue 3 frontend. Users authenticate via JWT and interact with a server shell through a browser-based xterm.js terminal over WebSocket.

## Build & Run Commands

```bash
# Backend
cargo build --release          # Build backend
cargo run                      # Run backend (dev)

# Frontend
cd frontend
npm install                    # Install deps
npm run dev                    # Dev server (hot reload, proxies to :3000)
npm run build                  # Production build → ../static/

# Deploy
./deploy.sh                    # Full deploy (build + systemd + static copy)
```

## Architecture

- **Backend**: Axum 0.8 on Tokio, PostgreSQL via sqlx, JWT auth (HS256), PTY via libc
- **Frontend**: Vue 3 + Vite 6 + TypeScript + Pinia 3 + Vue Router 4 + xterm.js 5
- **Build output**: Frontend builds to `static/`, served by Axum as fallback route
- **Reverse proxy**: Backend listens on `127.0.0.1:3000`, expects nginx at `/webshell/`

## Key Files

- `src/main.rs` — Route definitions, server startup, static file serving
- `src/ws.rs` — WebSocket handler, PTY spawning (libc openpty/fork/exec)
- `src/auth.rs` — JWT creation and verification
- `src/handlers.rs` — HTTP handlers (login, change-password, file upload)
- `src/db.rs` — PostgreSQL connection pool, schema migration
- `src/config.rs` — Environment variable config
- `frontend/vite.config.ts` — Vite config (base: `/webshell/`, outDir: `../static`)
- `frontend/src/stores/auth.ts` — Pinia auth store (token/username in localStorage)
- `frontend/src/components/Terminal.vue` — xterm.js WebSocket terminal

## Conventions

- Backend routes are prefixed with `/api/`
- Frontend base path is `/webshell/` (for reverse proxy)
- JWT is passed as `Authorization: Bearer <token>` for HTTP, `?token=<jwt>` for WebSocket
- Registration is disabled; users are added directly in the database
- Environment variables are loaded from `.env` via dotenvy
- Static files are committed as build output in `static/`

## Testing

```bash
cargo test                     # Run tests (integration tests require DB, marked #[ignore])
```

## Dependencies to Watch

- Axum 0.8 — check for breaking changes on upgrade
- sqlx 0.9 — compile-time query verification requires DATABASE_URL at build time
- @xterm/xterm 5 — major version may break API
