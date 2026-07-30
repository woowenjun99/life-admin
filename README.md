# Life Inbox

**Life Inbox turns life clutter into one clear next action.** It is a private
personal life-admin workspace for collecting the notes, reminders, and loose
ends that are easy to capture but hard to organise.

## What it is for

Life admin often arrives as fragments: a quick note about a passport renewal,
a document to read, a date to remember, or something that is waiting on another
person. Those fragments tend to stay spread across notes, messages, and memory,
which makes it difficult to decide what deserves attention now.

Life Inbox is designed to help people:

- **Capture first, organise later** — save an item as soon as it occurs instead
  of deciding where it belongs.
- **Turn ambiguity into a practical next step** — review a capture, shape it
  into a concise plan, and surface one recommended next action.
- **Keep the person in control** — suggestions and plans require review and
  approval; the product does not send messages, buy things, or make external
  changes on the user's behalf.
- **Keep personal information private** — each workspace is tied to an
  authenticated Firebase user, and application data is intended to be scoped to
  that owner.

The intended flow is:

```text
Capture → Review → Generate plan → Approve → Complete next action
```

## Illustrative examples

These examples show the intended reviewed workflow. They are not claims about
features that are already automated.

| Capture | What the person reviews | Example next action |
| --- | --- | --- |
| “My passport expires in October; check whether I need to renew before the December trip.” | The expiry date, trip date, and whether any requirement is uncertain. | Find the official renewal requirements and note the application deadline. |
| “The school sent a form about the museum trip.” | The child, return date, cost, and any missing attachment. | Read the form and add the return deadline to the plan. |
| “The washing machine is making a loud noise.” | Whether this is urgent, any warranty details, and possible repair options. | Find the model number and request a repair quote. |

Today, the implementation supports private authentication, authenticated text
capture, and private PDF/JPEG/PNG capture. Review, extraction, planning, file
reads/downloads, and step completion are the intended later stages illustrated
above.

## Research and market-discovery starting point

The following desk research informs the problem framing. It does **not** yet
validate demand for Life Inbox, its pricing, or its effectiveness; user
interviews, competitive research, and usability testing are still needed.

- The [U.S. Bureau of Labor Statistics' 2024 American Time Use Survey](https://www.bls.gov/news.release/archives/atus_06262025.htm)
  reported that 80% of people spent time on household activities on an average
  day, averaging about two hours. Its definition includes household-management
  and organisational activities, such as paperwork and planning a party.
- Allison Daminger's peer-reviewed study, [*The Cognitive Dimension of
  Household Labor*](https://doi.org/10.1177/0003122419859007), uses 70
  interviews with 35 couples to describe cognitive household work as
  anticipating needs, identifying options, making decisions, and monitoring
  progress. It is qualitative research, so it explains the problem rather than
  measuring the size of this product's market.

The current implementation provides the private Firebase-authenticated
workspace, text capture, and one-file private capture backed by PostgreSQL
metadata and Firebase Storage. Every supported upload is type/size checked and
scanned before it is stored. AI-assisted extraction, reviewed plans, Inbox
listing, and file reads/downloads are still planned; they are not represented
as completed product capabilities here.

## Technical overview

This repository contains two independent applications:

- `backend/` — an Axum API with PostgreSQL via SQLx and Firebase Admin Auth.
- `frontend/` — a Next.js App Router application with Firebase Web SDK, T3 Env, and Biome.

## Prerequisites

- Rust (the project uses the Rust 2024 edition)
- Bun 1.3 or later
- Docker Desktop for the local PostgreSQL and ClamAV containers
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

Set `FIREBASE_STORAGE_BUCKET` in `backend/.env` as well as the Firebase Auth
project value. For local emulation the bucket name is an identifier only; the
launcher points the server at the Storage Emulator.

The launcher starts PostgreSQL on `5432`, project-owned ClamAV on loopback port
`3310` by default, Firebase Auth Emulator on `9099`, Firebase Storage Emulator on `9199`
(with its UI on `4000`), Axum on `3001`, and Next.js on `3000`. It fails before
starting if another process owns one of those ports, so it does not interrupt
unrelated local services. PostgreSQL and ClamAV are first stopped only through
this project's Compose services. The launcher stops only the Firebase,
backend, and frontend child processes it creates when you press Ctrl-C; the
ClamAV signature volume remains available for the next start.

If port `3310` is in use, choose another loopback port with
`CLAMAV_HOST_PORT=3330 ./scripts/start-local.sh`. The launcher passes the
matching `CLAMAV_ADDRESS` to the local backend.

**The local launcher resets the PostgreSQL `public` schema on every start. All
local application tables and data are deleted.** Firebase Auth Emulator users
are also fresh unless you explicitly configure emulator import/export data.

Open a `psql` session with:

```sh
docker compose exec postgres psql -U app -d app
```

## Data schema manual smoke

The backend applies its embedded SQLx migrations before it starts serving
requests. After starting the local stack, verify the schema and migration
history without exposing personal capture contents:

```sh
docker compose exec postgres psql -U app -d app -c '\dt'
docker compose exec postgres psql -U app -d app -c 'SELECT version FROM _sqlx_migrations ORDER BY version'
```

With a Firebase Auth Emulator ID token, create a text capture through the
same API the browser will use:

```sh
curl --request POST http://127.0.0.1:3001/api/v1/inbox-items \
  --header "Authorization: Bearer $FIREBASE_ID_TOKEN" \
  --header 'Content-Type: application/json' \
  --data '{"text":"Renew passport"}'
```

The response contains metadata only. Inspect only the safe metadata columns:

```sh
docker compose exec postgres psql -U app -d app -c \
  'SELECT owner_uid, source_type, status, created_at, updated_at FROM inbox_items'
```

To confirm the status and source-content constraints, this block succeeds only
when PostgreSQL rejects both invalid inserts:

```sql
DO $$
BEGIN
  BEGIN
    INSERT INTO inbox_items (owner_uid, source_type, original_text, status)
    VALUES ('manual-smoke-owner', 'text', 'invalid status', 'invalid');
    RAISE EXCEPTION 'expected inbox_items status constraint';
  EXCEPTION WHEN check_violation THEN
    NULL;
  END;
  BEGIN
    INSERT INTO inbox_items (owner_uid, source_type, status)
    VALUES ('manual-smoke-owner', 'text', 'captured');
    RAISE EXCEPTION 'expected inbox_items source-content constraint';
  EXCEPTION WHEN check_violation THEN
    NULL;
  END;
END;
$$;
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

## Private file capture

`POST /api/v1/inbox-items/files` accepts exactly one multipart field named
`file`. It accepts one PDF, JPEG, or PNG of up to 10 MiB, verifies its declared
MIME type against its magic bytes, rejects unsafe display filenames, scans it
with ClamAV, and only then writes the object and its Inbox metadata. Object
keys are generated server-side and are never returned to the browser. This
increment has no file listing, preview, download, or direct browser Storage
access.

Set `FIREBASE_STORAGE_BUCKET` to the production bucket name. The service
account must have bucket-level object create and delete permission, while the
checked-in [Storage rules](storage.rules) deny all browser reads and writes.
Keep `FIREBASE_SERVICE_ACCOUNT_JSON` server-only; it is used by the backend
Storage JSON API client in production, not by the Web SDK.

For local work, the launcher sets
`FIREBASE_STORAGE_EMULATOR_HOST=127.0.0.1:9199`. The Firebase Storage Emulator
supports the object insert/delete operations used by this capture path; see the
[Firebase Storage Emulator documentation](https://firebase.google.com/docs/emulator-suite/connect_storage).

The backend image bundles ClamAV. It refreshes signatures before starting
`clamd`, keeps `clamd` bound to loopback, waits for its `zPING` health reply,
then starts Axum as the non-root `app` user. Persist `/var/lib/clamav` in
production so signature downloads survive restarts, budget memory for the
signature database plus concurrent 10 MiB scans, and never expose the scanner
port publicly. The scanner uses the framed `zINSTREAM` protocol described in
the [ClamD protocol documentation](https://docs.clamav.net/manual/Usage/ClamdProtocol.html).

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

Provide `DATABASE_URL`, `FIREBASE_PROJECT_ID`, `FIREBASE_STORAGE_BUCKET`, and
`FIREBASE_SERVICE_ACCOUNT_JSON` through the container platform's secret and
environment configuration. Mount persistent storage at `/var/lib/clamav` and
do not publish the scanner port. The image listens on port `3001`; keep it on a
private network and set the frontend's `BACKEND_INTERNAL_URL` to its internal
service URL.
