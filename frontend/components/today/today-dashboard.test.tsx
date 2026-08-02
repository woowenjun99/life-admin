import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";

import type { Plan, PlanStep } from "@/lib/api";

import {
  ArchivedPlans,
  TodayDashboard,
  todayDashboardData,
} from "./today-dashboard";

function step(
  id: string,
  status: PlanStep["status"],
  updatedAt: string,
  isNextAction = false,
): PlanStep {
  return {
    id,
    position: Number(id.replace(/\D/g, "")) || 0,
    title: `Step ${id}`,
    rationale: `Why step ${id} matters.`,
    status,
    dueOn: undefined,
    waitingOn: status === "waiting" ? `Reply for ${id}` : undefined,
    isNextAction,
    updatedAt,
  };
}

function plan(
  id: string,
  status: Plan["status"],
  updatedAt: string,
  steps: PlanStep[],
): Plan {
  return {
    id,
    inboxItemId: `inbox-${id}`,
    summary: `Plan ${id}`,
    status,
    revision: 1,
    steps,
    createdAt: "2026-07-01T00:00:00Z",
    updatedAt,
  };
}

test("Today promotes the newest ready Plan and keeps other Plans reachable", () => {
  const plans = [
    plan("newer", "ready", "2026-07-31T00:00:00Z", [
      step("newer-ready", "ready", "2026-07-31T00:00:00Z", true),
    ]),
    plan("older", "ready", "2026-07-30T00:00:00Z", [
      step("older-ready", "ready", "2026-07-30T00:00:00Z", true),
    ]),
    plan("waiting", "waiting", "2026-07-29T00:00:00Z", [
      step("waiting-step", "waiting", "2026-07-29T00:00:00Z"),
    ]),
  ];

  const dashboard = todayDashboardData(plans);
  expect(dashboard.primaryAction?.plan.id).toBe("newer");
  expect(dashboard.otherReadyPlans.map(({ plan }) => plan.id)).toEqual([
    "older",
  ]);
  expect(dashboard.waitingPlans.map(({ id }) => id)).toEqual(["waiting"]);

  const markup = renderToStaticMarkup(
    <TodayDashboard
      onRetry={() => undefined}
      state={{ status: "ready", plans }}
    />,
  );
  expect(markup).toContain("Step newer-ready");
  expect(markup).toContain("Other active Plans");
  expect(markup).toContain("From Plan");
  expect(markup).toContain("today-focus-grid has-companion");
  expect(markup).toContain('href="/plans/older"');
  expect(markup).toContain("Reply for waiting-step");
});

test("Today orders recent completed steps, limits them, and exposes recovery states", () => {
  const completed = Array.from({ length: 6 }, (_, index) =>
    step(
      `complete-${index}`,
      "complete",
      `2026-07-${String(20 + index).padStart(2, "0")}T00:00:00Z`,
    ),
  );
  const dashboard = todayDashboardData([
    plan("complete", "complete", "2026-07-31T00:00:00Z", completed),
  ]);

  expect(dashboard.recentlyCompleted).toHaveLength(5);
  expect(dashboard.recentlyCompleted[0]?.step.id).toBe("complete-5");
  expect(dashboard.recentlyCompleted.at(-1)?.step.id).toBe("complete-1");

  const noPlans = renderToStaticMarkup(
    <TodayDashboard
      onRetry={() => undefined}
      state={{ status: "ready", plans: [] }}
    />,
  );
  expect(noPlans).toContain("Your next action will appear here.");

  const error = renderToStaticMarkup(
    <TodayDashboard onRetry={() => undefined} state={{ status: "error" }} />,
  );
  expect(error).toContain("We could not load your Plans.");
  expect(error).toContain("Retry");
});

test("Archived Plans stays collapsed, handles recovery states, and offers Restore without a Plan link", () => {
  const archived = renderToStaticMarkup(
    <ArchivedPlans
      onRestore={() => undefined}
      onRetry={() => undefined}
      restoreError={null}
      restoringPlanId={null}
      state={{
        status: "ready",
        plans: [
          plan("archived", "waiting", "2026-07-31T00:00:00Z", [
            step("archived-step", "waiting", "2026-07-31T00:00:00Z"),
          ]),
        ],
      }}
    />,
  );
  expect(archived).toContain("<details");
  expect(archived).toContain("Archived Plans (1)");
  expect(archived).toContain("Restore");
  expect(archived).not.toContain('href="/plans/archived"');

  const empty = renderToStaticMarkup(
    <ArchivedPlans
      onRestore={() => undefined}
      onRetry={() => undefined}
      restoreError={null}
      restoringPlanId={null}
      state={{ status: "ready", plans: [] }}
    />,
  );
  expect(empty).toContain("No archived Plans.");

  const retry = renderToStaticMarkup(
    <ArchivedPlans
      onRestore={() => undefined}
      onRetry={() => undefined}
      restoreError="We could not restore this Plan. Please try again."
      restoringPlanId="archived"
      state={{ status: "error" }}
    />,
  );
  expect(retry).toContain("We could not load archived Plans.");
  expect(retry).toContain("We could not restore this Plan.");
  expect(retry).toContain("Retry");
});
