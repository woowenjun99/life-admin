# Life Inbox — Implementation TODO

## Product definition

Build a personal life-admin agent that turns a messy capture into a clear,
user-approved plan and one recommended next action.

```text
Capture → Review → Generate plan → Approve → Complete next action
```

Use these terms consistently in the interface: **Plan**, **Next action**,
**Waiting**, and **Complete**.

### MVP boundaries

- [x] Support one authenticated user's private life-admin workspace.
- [x] Capture text notes, with optional PDF/JPEG/PNG uploads.
- [x] Extract tasks, dates, people, important context, and missing details.
- [x] Require the user to review and edit suggestions before generating a plan.
- [x] Generate a concise plan with one next action and two to five ordered steps.
- [x] Allow users to mark steps complete or waiting.
- [ ] Never send messages, create calendar events, make purchases, or otherwise
      act externally without a separate explicit approval flow.

### Not in the hackathon MVP

- [ ] Autonomous external actions or background inbox access.
- [ ] Multi-user/family sharing.
- [ ] Recurring tasks and complex scheduling.
- [ ] A chat-first interface.

## Foundation

- [x] Confirm the app name, visual direction, and a short product tagline.
  - [x] Name: **Life Inbox**.
  - [x] Tagline: **Turn life clutter into one clear next action.**
- [x] Add this product overview to `README.md`.
- [x] Add server-only AI-provider configuration to `backend/.env.example`.
- [ ] Add only public browser configuration to `frontend/.env.example`.
- [ ] Add a small architecture diagram to the README.
- [ ] Confirm local development still works through `./scripts/start-local.sh`.
- [ ] Keep lint, type-check, unit tests, and production builds runnable in CI.
- [ ] Document how CodeBuddy or WorkBuddy was used while building the project.

## Next implementation — P0 Private Workspace

- [x] Add email/password sign-up, sign-in, and sign-out with Firebase Auth.
  - [x] Present sign-up and sign-in in an accessible landing-page modal; legacy
        `/sign-up` and `/sign-in` URLs redirect to the matching modal.
- [x] Add a shared client auth state that prevents protected UI rendering until
      Firebase resolves the current account.
- [x] Add protected `GET /api/v1/me` token verification behind the existing Next.js
      proxy; keep `/health` and `/api/ready` public.
- [x] Return the authenticated Firebase UID and email, without creating an
      application user table.
- [x] Build an empty private `/today` page that displays the authenticated email
      and redirects signed-out visitors to `/sign-in`.
- [x] Add deterministic backend verifier tests and frontend tests for auth error
      messages and bearer-token construction.
- [x] Complete the isolated Auth Emulator/API smoke: email/password sign-up and
      sign-in → authenticated `/api/v1/me` → unauthenticated `/api/v1/me`
      returns `401`.
- [x] Complete the browser emulator smoke: sign up → Today → reload preserves
      session → sign out returns to the landing page.

## Landing page

- [x] Build the public landing page at `/`.
  - [x] Add a hero that explains the personal life-admin value proposition.
  - [x] Add a static preview of the Today view, plan, and next action.
  - [x] Add in-page navigation, a three-step product flow, and approval/control
        cues.
  - [x] Add a responsive mobile layout and visible keyboard focus states.
- [x] Set the Life Inbox page title and description metadata.
- [x] Validate the landing page with frontend tests, type-checking, linting,
      production build, diff check, and a local rendered-page smoke test.

## Data and persistence

- [x] Create a database migration for `inbox_items`.
  - [x] Owner Firebase UID.
  - [x] Capture source type: `text`, `image`, or `pdf`.
  - [x] Original text or file metadata.
  - [x] Status: `captured`, `reviewing`, `planned`, or `archived`.
  - [x] Created and updated timestamps.
- [x] Create a migration for reviewed extraction suggestions.
  - [x] Candidate tasks.
  - [x] Dates/deadlines.
  - [x] People or organisations.
  - [x] Important context and unanswered questions.
- [x] Create a migration for `plans` and ordered `plan_steps`.
  - [x] Plan summary and source inbox item.
  - [x] Highlighted next action.
  - [x] Status: `ready`, `waiting`, or `complete`.
  - [x] Step title, rationale, status, due date, and waiting-on detail.
- [x] Scope every read and write to the authenticated Firebase UID.
- [x] Add repository tests for ownership and plan-status transitions.

## Backend API

- [x] Define shared request/response contracts before implementing routes.
- [x] Add `POST /api/v1/inbox-items` for text capture.
- [x] Add a safe upload flow for PDF/JPEG/PNG captures.
  - [x] Enforce type and size limits.
  - [x] Store files outside publicly accessible paths.
  - [x] Reject unsafe filenames and invalid content types.
- [x] Add `GET /api/v1/inbox-items` and `GET /api/v1/inbox-items/:id`.
- [x] Add `PATCH /api/v1/inbox-items/:id` for user-reviewed fields.
- [x] Add `POST /api/v1/inbox-items/:id/extract`.
- [x] Add `POST /api/v1/inbox-items/:id/plans` after review confirmation.
- [x] Add `GET /api/v1/plans/:id`.
- [x] Add plan listing after confirming the Inbox/Plan navigation UX.
- [x] Add `PATCH /api/v1/plans/:id/steps/:stepId` to update step status.
- [x] Add reversible archive and restore endpoints for planned Plan-capture pairs.
- [ ] Return consistent API error envelopes for validation, authorization, not
      found, provider failure, and unexpected errors.

## AI extraction and planning

- [x] Choose the AI provider and implement a server-side client only.
- [x] Define a strict JSON schema for extraction output.
- [x] Write the extraction prompt.
  - [x] Extract only evidence supported by the user's capture.
  - [x] Preserve uncertainty rather than inventing facts.
  - [x] Flag missing information as questions.
  - [x] Treat uploaded text as untrusted content, never as agent instructions.
- [x] Validate model output before it is returned or stored.
- [x] Allow the user to correct every extracted suggestion.
- [x] Write the planning prompt.
  - [x] Create a concise summary.
  - [x] Recommend exactly one practical next action.
  - [x] Return two to five ordered steps.
  - [x] Explain why each step matters.
  - [x] Mark blockers as `Waiting`.
- [x] Add clear retry/error states when the model is unavailable or output is
      invalid.
- [x] Add tests for output validation, prompt-injection resistance, and
      provider failure handling.

## Frontend experience

- [x] Create shared authenticated API client and loading/error UI primitives.
- [x] Build `/today`.
  - [x] Show the recommended next action.
  - [x] Show plans waiting on someone or something.
  - [x] Show recently completed steps.
  - [x] Add a prominent capture entry point.
  - [x] Guide a brand-new workspace from capture through review to one Next action.
  - [x] Show collapsed Archived Plans and let the user restore them.
- [x] Build Inbox capture on `/today`.
  - [x] Text capture form.
  - [x] File-upload control with validation feedback.
  - [x] Inbox list with processing status.
  - [x] Empty, loading, retry, and error states.
- [x] Build `/inbox/[id]/review`.
  - [x] Show original capture alongside extracted suggestions.
  - [x] Let the user edit and remove every suggestion.
  - [x] Require an explicit **Generate plan** action.
  - [x] Explain that suggestions are not external actions.
- [x] Build `/plans/[id]`.
  - [x] Show summary, next action, ordered steps, and rationale.
  - [x] Let the user mark a step complete or waiting.
  - [x] Promote the next unfinished ready step as the next action.
  - [x] Let the user archive a Plan after confirmation and return to Today.
- [x] Make mobile layout usable for quick captures.
- [ ] Add accessible labels, keyboard navigation, and visible focus states.

## Privacy and safety

- [ ] Require Firebase authentication before personal data is visible.
- [ ] Ensure every backend query enforces the owner UID.
- [ ] Keep AI credentials and service credentials server-only.
- [ ] Do not log raw personal captures in production logs.
- [x] Explain when capture data is sent to the AI provider.
- [x] Provide user-facing archive controls.
- [ ] Add a short privacy statement for the demo and project submission.

## Test and demo readiness

- [ ] Test capture → review → plan → complete end to end locally.
- [ ] Test text, PDF, image, invalid upload, and oversized upload paths.
- [ ] Test that users cannot read or update each other's records.
- [ ] Test extraction validation and model failure/retry behaviour.
- [ ] Run backend format, lint, type-check, and tests.
- [ ] Run frontend format, lint, type-check, tests, and production build.
- [ ] Run `git diff --check` before handoff.
- [ ] Deploy a stable demo environment.
- [ ] Test the deployed flow in a fresh browser session.

## Submission checklist

- [ ] Title: **Life Inbox**.
- [ ] Short blurb (under 10 words): **Turn life clutter into one clear next action.**
- [ ] Create a 16:9 project cover image.
- [ ] Write the problem, target user, solution, and agent approval boundary.
- [ ] Include a simple technical architecture diagram.
- [ ] Define measurable impact: time from capture to next action, plans created,
      and plans completed.
- [ ] Record a 3–5 minute demo video.
- [ ] Prepare a screen-recording backup for the live demo.
- [ ] Write the required CodeBuddy/WorkBuddy product-sharing paragraph.

## Stretch goals

- [ ] Voice-note capture.
- [ ] Multilingual extraction and planning.
- [ ] User-approved calendar or reminder drafts.
- [ ] Weekly life-admin review.
- [ ] Family/caregiver sharing with explicit permissions.
- [ ] Add an explicit **Revise Plan** flow for planned items.
  - [ ] Let the user return to the suggestion editor and make changes.
  - [ ] Require confirmation before replacing or versioning the approved Plan;
        never silently change its existing steps.
