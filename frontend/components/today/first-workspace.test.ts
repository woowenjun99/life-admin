import { expect, test } from "bun:test";

import type { InboxListState } from "@/components/inbox/inbox-list";

import { isBrandNewWorkspace } from "./first-workspace";
import type { ArchivedPlansState, TodayPlansState } from "./today-dashboard";

const noActivePlans: TodayPlansState = { status: "ready", plans: [] };
const noArchivedPlans: ArchivedPlansState = { status: "ready", plans: [] };
const noInboxItems: InboxListState = { status: "ready", items: [] };

test("recognizes a fully loaded workspace with no active or archived work", () => {
  expect(
    isBrandNewWorkspace({
      plans: noActivePlans,
      archivedPlans: noArchivedPlans,
      inbox: noInboxItems,
    }),
  ).toBe(true);
});

test("keeps the first-task guide hidden when work exists or any source is unresolved", () => {
  expect(
    isBrandNewWorkspace({
      plans: {
        status: "ready",
        plans: [
          {
            id: "plan-123",
            inboxItemId: "inbox-123",
            summary: "Renew passport",
            status: "ready",
            steps: [],
            createdAt: "2026-07-31T00:00:00Z",
            updatedAt: "2026-07-31T00:00:00Z",
          },
        ],
      },
      archivedPlans: noArchivedPlans,
      inbox: noInboxItems,
    }),
  ).toBe(false);
  expect(
    isBrandNewWorkspace({
      plans: noActivePlans,
      archivedPlans: {
        status: "ready",
        plans: [
          {
            id: "plan-archived",
            inboxItemId: "inbox-archived",
            summary: "File taxes",
            status: "complete",
            steps: [],
            createdAt: "2026-07-31T00:00:00Z",
            updatedAt: "2026-07-31T00:00:00Z",
          },
        ],
      },
      inbox: noInboxItems,
    }),
  ).toBe(false);
  expect(
    isBrandNewWorkspace({
      plans: noActivePlans,
      archivedPlans: noArchivedPlans,
      inbox: {
        status: "ready",
        items: [
          {
            id: "inbox-123",
            sourceType: "text",
            status: "reviewing",
            canRetryExtraction: false,
            createdAt: "2026-07-31T00:00:00Z",
            updatedAt: "2026-07-31T00:00:00Z",
          },
        ],
      },
    }),
  ).toBe(false);
  expect(
    isBrandNewWorkspace({
      plans: { status: "loading" },
      archivedPlans: noArchivedPlans,
      inbox: noInboxItems,
    }),
  ).toBe(false);
  expect(
    isBrandNewWorkspace({
      plans: noActivePlans,
      archivedPlans: { status: "error" },
      inbox: noInboxItems,
    }),
  ).toBe(false);
  expect(
    isBrandNewWorkspace({
      plans: noActivePlans,
      archivedPlans: noArchivedPlans,
      inbox: { status: "loading" },
    }),
  ).toBe(false);
});
