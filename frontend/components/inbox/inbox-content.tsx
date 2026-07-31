"use client";

import type { Plan } from "@/lib/api";

import { CaptureForms } from "./capture-forms";
import { InboxList, type InboxListState } from "./inbox-list";

export function InboxContent({
  firstTask = false,
  inboxState,
  onCaptured,
  onRetry,
  plans = [],
}: {
  firstTask?: boolean;
  inboxState: InboxListState;
  onCaptured(): void;
  onRetry(): void;
  plans?: Plan[];
}) {
  const planSummaries = new Map(
    plans.map((plan) => [plan.inboxItemId, plan.summary]),
  );
  const listState =
    inboxState.status === "ready"
      ? {
          status: "ready" as const,
          items: inboxState.items.map((item) => {
            const planSummary = planSummaries.get(item.id);
            return planSummary ? { ...item, planSummary } : item;
          }),
        }
      : inboxState;

  return (
    <>
      <CaptureForms
        onCaptured={onCaptured}
        variant={firstTask ? "first_task" : "default"}
      />
      {!firstTask ? <InboxList onRetry={onRetry} state={listState} /> : null}
    </>
  );
}
