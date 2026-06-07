# Rust WebShell

A web-based terminal built with Rust + Axum, providing secure browser access to a server shell via WebSocket and xterm.js.

## Tech Stack

### Backend
- **Axum** — Async web framework
- **sqlx** — PostgreSQL async driver
- **bcrypt** — Password hashing (auto-salted)
- **JWT** — Authentication (HS256, 24h expiry)
- **libc** — Low-level PTY/fork for shell spawning

### Frontend
- **Vue 3** — Composition API with `<script setup>`
- **Vite 6** — Build tool
- **TypeScript** — Type safety
- **Pinia 3** — State management
- **Vue Router 4** — Client-side routing
- **xterm.js 5** — Terminal emulator
- **Axios** — HTTP client

## Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `DATABASE_URL` | PostgreSQL connection string | Yes |
| `JWT_SECRET` | JWT signing secret | Yes |
| `PORT` | Server port | No (default: 3000) |

## Getting Started

### Prerequisites
- Rust (latest stable)
- Node.js 20+
- PostgreSQL

### Backend

```bash
# Build
cargo build --release

# Run
./target/release/rust-webshell
```

### Frontend

```bash
cd frontend

# Install dependencies
npm install

# Development (with hot reload, proxies to backend at :3000)
npm run dev

# Production build (outputs to ../static/)
npm run build
```

### Deploy

```bash
./deploy.sh
```

Builds the backend, installs the binary as a systemd service, and copies static files to `/usr/local/share/webshell/static`.

## API Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/login` | No | Login `{username, password}` → `{token, username}` |
| POST | `/api/change-password` | No | Change password `{username, old_password, new_password}` |
| POST | `/api/upload` | Bearer JWT | Multipart file upload → `/root/uploads/` |
| GET | `/api/ws/shell?token=xxx` | JWT query param | WebSocket PTY shell |

> **Note:** Registration is disabled. Users must be added directly in the database.

## Project Structure

```
├── src/                    # Rust backend
│   ├── main.rs             # Entry point, routes, static file serving
│   ├── config.rs           # Environment config
│   ├── db.rs               # PostgreSQL pool + migrations
│   ├── auth.rs             # JWT creation/verification
│   ├── handlers.rs         # HTTP handlers (login, upload, etc.)
│   └── ws.rs               # WebSocket + PTY shell handler
├── frontend/               # Vue 3 frontend
│   ├── src/
│   │   ├── api/            # Axios API client
│   │   ├── components/     # Terminal, VirtualKeyboard, dialogs
│   │   ├── router/         # Vue Router config
│   │   ├── stores/         # Pinia auth store
│   │   └── views/          # Login, Shell pages
│   └── vite.config.ts      # Build config (outputs to ../static/)
├── static/                 # Build output (served by backend)
├── tests/                  # Integration tests
├── deploy.sh               # Deployment script
└── .env                    # Environment variables (not committed)
```

## Architecture

The backend binds to `127.0.0.1:3000` (localhost only) and is designed to sit behind a reverse proxy (e.g. nginx) that terminates TLS at `/webshell/`. The frontend WebSocket URL auto-selects `ws:` or `wss:` based on the page protocol.

The PTY shell uses raw `libc` calls (`openpty`, `fork`, `setsid`, `execlp`) to spawn `/bin/bash --login` as root. Terminal resize is supported via JSON messages `{ "cols": N, "rows": N }`.
