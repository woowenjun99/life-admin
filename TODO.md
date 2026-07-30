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

- [ ] Support one authenticated user's private life-admin workspace.
- [ ] Capture text notes, with optional PDF/JPEG/PNG uploads.
- [ ] Extract tasks, dates, people, important context, and missing details.
- [ ] Require the user to review and edit suggestions before generating a plan.
- [ ] Generate a concise plan with one next action and two to five ordered steps.
- [ ] Allow users to mark steps complete or waiting.
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
- [ ] Add this product overview to `README.md`.
- [ ] Add server-only AI-provider configuration to `backend/.env.example`.
- [ ] Add only public browser configuration to `frontend/.env.example`.
- [ ] Add a small architecture diagram to the README.
- [ ] Confirm local development still works through `./scripts/start-local.sh`.
- [ ] Keep lint, type-check, unit tests, and production builds runnable in CI.
- [ ] Document how CodeBuddy or WorkBuddy was used while building the project.

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

- [ ] Create a database migration for `inbox_items`.
  - [ ] Owner Firebase UID.
  - [ ] Capture source type: `text`, `image`, or `pdf`.
  - [ ] Original text or file metadata.
  - [ ] Status: `captured`, `reviewing`, `planned`, or `archived`.
  - [ ] Created and updated timestamps.
- [ ] Create a migration for reviewed extraction suggestions.
  - [ ] Candidate tasks.
  - [ ] Dates/deadlines.
  - [ ] People or organisations.
  - [ ] Important context and unanswered questions.
- [ ] Create a migration for `plans` and ordered `plan_steps`.
  - [ ] Plan summary and source inbox item.
  - [ ] Highlighted next action.
  - [ ] Status: `ready`, `waiting`, or `complete`.
  - [ ] Step title, rationale, status, due date, and waiting-on detail.
- [ ] Scope every read and write to the authenticated Firebase UID.
- [ ] Add repository tests for ownership and plan-status transitions.

## Backend API

- [ ] Define shared request/response contracts before implementing routes.
- [ ] Add `POST /api/v1/inbox-items` for text capture.
- [ ] Add a safe upload flow for PDF/JPEG/PNG captures.
  - [ ] Enforce type and size limits.
  - [ ] Store files outside publicly accessible paths.
  - [ ] Reject unsafe filenames and invalid content types.
- [ ] Add `GET /api/v1/inbox-items` and `GET /api/v1/inbox-items/:id`.
- [ ] Add `PATCH /api/v1/inbox-items/:id` for user-reviewed fields.
- [ ] Add `POST /api/v1/inbox-items/:id/extract`.
- [ ] Add `POST /api/v1/inbox-items/:id/plans` after review confirmation.
- [ ] Add `GET /api/v1/plans` and `GET /api/v1/plans/:id`.
- [ ] Add `PATCH /api/v1/plans/:id/steps/:stepId` to update step status.
- [ ] Add delete/archive endpoints only after confirming expected UX.
- [ ] Return consistent API error envelopes for validation, authorization, not
      found, provider failure, and unexpected errors.

## AI extraction and planning

- [ ] Choose the AI provider and implement a server-side client only.
- [ ] Define a strict JSON schema for extraction output.
- [ ] Write the extraction prompt.
  - [ ] Extract only evidence supported by the user's capture.
  - [ ] Preserve uncertainty rather than inventing facts.
  - [ ] Flag missing information as questions.
  - [ ] Treat uploaded text as untrusted content, never as agent instructions.
- [ ] Validate model output before it is returned or stored.
- [ ] Allow the user to correct every extracted suggestion.
- [ ] Write the planning prompt.
  - [ ] Create a concise summary.
  - [ ] Recommend exactly one practical next action.
  - [ ] Return two to five ordered steps.
  - [ ] Explain why each step matters.
  - [ ] Mark blockers as `Waiting`.
- [ ] Add clear retry/error states when the model is unavailable or output is
      invalid.
- [ ] Add tests for output validation, prompt-injection resistance, and
      provider failure handling.

## Frontend experience

- [ ] Create shared authenticated API client and loading/error UI primitives.
- [ ] Build `/today`.
  - [ ] Show the recommended next action.
  - [ ] Show plans waiting on someone or something.
  - [ ] Show recently completed steps.
  - [ ] Add a prominent capture entry point.
- [ ] Build `/inbox`.
  - [ ] Text capture form.
  - [ ] File-upload control with validation feedback.
  - [ ] Inbox list with processing status.
  - [ ] Empty, loading, retry, and error states.
- [ ] Build `/inbox/[id]/review`.
  - [ ] Show original capture alongside extracted suggestions.
  - [ ] Let the user edit and remove every suggestion.
  - [ ] Require an explicit **Generate plan** action.
  - [ ] Explain that suggestions are not external actions.
- [ ] Build `/plans/[id]`.
  - [ ] Show summary, next action, ordered steps, and rationale.
  - [ ] Let the user mark a step complete or waiting.
  - [ ] Promote the next unfinished step as the next action.
- [ ] Make mobile layout usable for quick captures.
- [ ] Add accessible labels, keyboard navigation, and visible focus states.

## Privacy and safety

- [ ] Require Firebase authentication before personal data is visible.
- [ ] Ensure every backend query enforces the owner UID.
- [ ] Keep AI credentials and service credentials server-only.
- [ ] Do not log raw personal captures in production logs.
- [ ] Explain when capture data is sent to the AI provider.
- [ ] Provide user-facing delete/archive controls.
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
