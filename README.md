# Full-stack starter

This repository contains two independent applications:

- `backend/` — an Axum API with PostgreSQL via SQLx and Firebase Admin Auth.
- `frontend/` — a Next.js App Router application with Firebase Web SDK, T3 Env, and Biome.

## Prerequisites

- Rust (the project uses the Rust 2024 edition)
- Bun 1.3 or later
- Docker Desktop for the local PostgreSQL container
- A Firebase project for browser configuration and Firebase Admin credentials

## Local development

Install the frontend and backend tooling once, then create local configuration:

```sh
cd backend && bun install
cd ../frontend && bun install
cd ..
cp backend/.env.example backend/.env
cp frontend/.env.example frontend/.env.local
```

Fill in the Firebase project and Web App values, then start the full local
stack with one command:

```sh
./scripts/start-local.sh
```

The launcher starts PostgreSQL on `5432`, Firebase Auth Emulator on `9099`
(with its UI on `4000`), Axum on `3001`, and Next.js on `3000`. It fails before
starting if another process owns one of those ports, so it does not interrupt
unrelated local services. PostgreSQL is first stopped only through this
project's Compose service. The launcher stops only the Firebase, backend, and
frontend child processes it creates when you press Ctrl-C.

**The local launcher resets the PostgreSQL `public` schema on every start. All
local application tables and data are deleted.** Firebase Auth Emulator users
are also fresh unless you explicitly configure emulator import/export data.

Open a `psql` session with:

```sh
docker compose exec postgres psql -U app -d app
```

## Firebase credentials

The Rust service initializes a Firebase Admin Auth client from the complete
service-account JSON supplied in the server-only
`FIREBASE_SERVICE_ACCOUNT_JSON` environment variable. Store that value in a
secret manager; do not write the JSON to a file or commit it. The backend also
requires `FIREBASE_PROJECT_ID`, which must match the service account's project.

For the Firebase Auth Emulator, set
`FIREBASE_AUTH_EMULATOR_HOST=127.0.0.1:9099`; no service-account key is needed
for emulator access.

The web SDK accepts only `NEXT_PUBLIC_FIREBASE_*` configuration values. Do not
put a service-account credential or another server secret in `frontend/.env.local`.

## API proxy

Browser code calls relative URLs such as `fetch("/api/ready")`. The Next.js
route handler proxies those requests to `BACKEND_INTERNAL_URL`, a server-only
variable. Local development uses `http://127.0.0.1:3001`; a deployed frontend
should use the backend's private service URL, such as `http://backend:3001`.
`BACKEND_INTERNAL_URL` is never sent to the browser.

## Backend container

Build the backend image from the repository root:

```sh
docker build --file backend/Dockerfile --tag full-stack-backend backend
```

Provide `DATABASE_URL`, `FIREBASE_PROJECT_ID`, and
`FIREBASE_SERVICE_ACCOUNT_JSON` through the container platform's secret and
environment configuration. The image listens on port `3001`; keep it on a
private network and set the frontend's `BACKEND_INTERNAL_URL` to its internal
service URL.
