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
and PDF/JPEG/PNG capture, text/PDF extraction into editable suggestions, an
explicit plan-generation confirmation, and editable Plans. A person can edit a
Plan summary and its ordered steps while keeping completed work final; each
save is revision-checked. Each Plan also has one private discussion where the
assistant can answer questions or propose a revision, which the person must
explicitly apply. No conversation or Plan update takes an external action.
The earliest ready step is shown as the Next action. Image captures remain
private and saved, but are not AI-extracted.

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
workspace, text and private-file capture backed by PostgreSQL metadata and
Firebase Storage, automatic text/PDF extraction, editable review, and Plans
with owner-scoped step-status updates. Every supported upload is type/size
checked and stored privately. Image extraction, plan listing, and
archive/delete controls are intentionally not included.

## Technical overview

This repository contains two independent applications:

- `backend/` — an Axum API with PostgreSQL via SQLx, Firebase Admin Auth, and Firebase Cloud Messaging.
- `frontend/` — a Next.js App Router PWA with Firebase Web SDK, T3 Env, and Biome.

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

Set `FIREBASE_STORAGE_BUCKET` in `backend/.env` as well as the Firebase Auth
project value. For local emulation the bucket name is an identifier only; the
launcher points the server at the Storage Emulator.

To enable extraction and plan generation locally, set `OPENAI_API_KEY` in
`backend/.env`. The key is read only by Axum; it must never appear in a browser
environment file. `OPENAI_BASE_URL` defaults to `https://api.openai.com/v1`,
`OPENAI_MODEL` defaults to `gpt-5.6-terra`, and `OPENAI_API_MODE` defaults to
`responses`. That default uses the
[Responses API](https://developers.openai.com/api/docs/guides/migrate-to-responses),
strict structured outputs, and the temporary PDF Files API flow.

For DeepSeek text-note extraction and plan generation, use its Chat
Completions compatibility API:

```dotenv
OPENAI_API_MODE=chat_completions
OPENAI_BASE_URL=https://api.deepseek.com
OPENAI_MODEL=deepseek-v4-pro
```

Chat Completions mode validates JSON output in the backend, but does not send
PDFs to the provider because it does not rely on a provider Files API. A PDF
is still saved privately and reported as unsupported for extraction. Without a
key, captures are still saved and text/PDF extraction reports a retryable
provider state instead of discarding the capture.

Before switching an existing deployment to Chat Completions mode, run it with
the prior Responses-mode provider settings until any queued provider-file
deletions have drained. The server refuses to start an incompatible mode while
that cleanup queue is non-empty, so private PDFs are not left at the old
provider.

The launcher starts PostgreSQL on `5432`, Firebase Auth Emulator on `9099`,
Firebase Storage Emulator on `9199` (with its UI on `4000`), Axum on `3001`,
and Next.js on `3000`. It fails before starting if another process owns one of
those ports, so it does not interrupt unrelated local services. PostgreSQL is
first stopped only through this project's Compose service. The launcher stops
only the Firebase, backend, and frontend child processes it creates when you
press Ctrl-C.

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

## Private web sessions

After Firebase sign-in, the browser exchanges its Firebase ID token through
the same-origin API proxy for a five-day `life_inbox_session` cookie. The
cookie is `HttpOnly`, `SameSite=Lax`, host-only, and scoped to `/`; it is also
`Secure` outside the Firebase Auth Emulator. The Next.js `proxy.ts` verifies
this cookie through the backend before serving Today, Plan, or review routes.
The backend remains the authority for API access and continues to verify the
browser's Firebase bearer token on every private API request.

The session cookie is cleared through `DELETE /api/v1/auth/session` before
the browser signs out of Firebase. Firebase App Check is not enabled in this
project yet.

## Private file capture

`POST /api/v1/inbox-items/files` accepts exactly one multipart field named
`file`. It accepts one PDF, JPEG, or PNG of up to 10 MiB, verifies its declared
MIME type against its magic bytes, rejects unsafe display filenames, and then
writes the object and its Inbox metadata. Object keys are generated server-side
and are never returned to the browser. There is no direct browser Storage
access. This increment does not include malware scanning; type validation is
not a substitute for content safety screening.

## AI data handling and review

When `OPENAI_API_KEY` is configured, saved text captures are sent to the
configured AI provider solely to produce structured, evidence-backed draft
suggestions. The default provider is OpenAI and uses Responses mode; changing
`OPENAI_BASE_URL` changes where those captures are sent. JPEG and PNG captures
are never sent to the AI provider in this increment. In Responses mode, PDFs
use the temporary Files API flow; the backend deletes the provider file after
extraction and queues server-side cleanup if that deletion is temporarily
unavailable. In Chat Completions mode, PDFs remain private but are not sent to
the provider.

The configured AI provider receives untrusted capture data, never authorization
to act. The model is instructed to preserve missing details as questions and to
ignore instructions embedded in a capture. Before a plan is generated, people
can edit or remove every suggestion. Planning receives only that reviewed list,
and the product does not take external actions. Provider file identifiers,
Storage object keys, and credentials are never returned in API responses.

For a PDF, the review screen reads the file through an authenticated,
owner-scoped backend stream; it does not expose a Firebase Storage URL.

Set `FIREBASE_STORAGE_BUCKET` to the production bucket name. The service
account must have bucket-level object create and delete permission, while the
checked-in [Storage rules](storage.rules) deny all browser reads and writes.
Keep `FIREBASE_SERVICE_ACCOUNT_JSON` server-only; it is used by the backend
Storage JSON API client in production, not by the Web SDK.

For local work, the launcher sets
`FIREBASE_STORAGE_EMULATOR_HOST=127.0.0.1:9199`. The Firebase Storage Emulator
supports the object insert/delete operations used by this capture path; see the
[Firebase Storage Emulator documentation](https://firebase.google.com/docs/emulator-suite/connect_storage).

The backend image starts Axum as the non-root `app` user.

The web SDK accepts only `NEXT_PUBLIC_FIREBASE_*` configuration values. Do not
put a service-account credential or another server secret in `frontend/.env.local`.

## PWA and Firebase Cloud Messaging

Life Inbox is installable from a supported browser: its manifest opens the
private Today workspace and its root service worker receives Firebase Cloud
Messaging (FCM) data messages. On a signed-in workspace, select **Turn on
alerts** and grant the browser permission. The app stores the resulting FCM
Firebase Installation ID owner-scoped; the identifier is sent only to the
authenticated backend and is never included in a URL or application response.

To enable browser delivery in a deployed environment, enable Firebase Cloud
Messaging for the Firebase project and add the public Web Push certificate key
from Firebase Console to the frontend environment:

```dotenv
NEXT_PUBLIC_FIREBASE_VAPID_KEY=your-public-web-push-certificate-key
```

`FIREBASE_SERVICE_ACCOUNT_JSON` stays server-only. The backend uses that same
service account to call FCM; do not create a browser-accessible server key.
FCM has no local emulator, so local development remains installable but alert
delivery is disabled when no service account is configured.

Successful suggestion extraction and Plan generation each enqueue a generic
FCM alert. The backend also checks active, non-complete Plan steps every minute
and alerts once on their `due_on` date (UTC); a transient delivery failure is
retried after five minutes. Lock-screen messages intentionally omit personal
capture and Plan content.

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

Provide `DATABASE_URL`, `FIREBASE_PROJECT_ID`, `FIREBASE_STORAGE_BUCKET`,
`FIREBASE_SERVICE_ACCOUNT_JSON`, and (when AI is enabled) `OPENAI_API_KEY`
through the container platform's secret and environment configuration. The
image listens on port `3001`; keep it on a private network and set the
frontend's `BACKEND_INTERNAL_URL` to its internal service URL.
