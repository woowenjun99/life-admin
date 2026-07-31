import type { InboxListState } from "@/components/inbox/inbox-list";

import type { ArchivedPlansState, TodayPlansState } from "./today-dashboard";

export function isBrandNewWorkspace({
  archivedPlans,
  inbox,
  plans,
}: {
  archivedPlans: ArchivedPlansState;
  inbox: InboxListState;
  plans: TodayPlansState;
}): boolean {
  return (
    plans.status === "ready" &&
    plans.plans.length === 0 &&
    archivedPlans.status === "ready" &&
    archivedPlans.plans.length === 0 &&
    inbox.status === "ready" &&
    inbox.items.length === 0
  );
}
