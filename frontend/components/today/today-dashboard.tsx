import Link from "next/link";
import type { ReactNode } from "react";

import type { Plan, PlanStep } from "@/lib/api";

const RECENT_COMPLETED_LIMIT = 5;

type PlanWithNextAction = {
  plan: Plan;
  nextAction: PlanStep;
};

export type RecentlyCompletedStep = {
  plan: Plan;
  step: PlanStep;
};

export type TodayDashboardData = {
  primaryAction?: PlanWithNextAction;
  otherReadyPlans: PlanWithNextAction[];
  waitingPlans: Plan[];
  recentlyCompleted: RecentlyCompletedStep[];
};

export type TodayPlansState =
  | { status: "loading" }
  | { status: "error" }
  | { status: "ready"; plans: Plan[] };

export type ArchivedPlansState =
  | { status: "loading" }
  | { status: "error" }
  | { status: "ready"; plans: Plan[] };

export function todayDashboardData(plans: Plan[]): TodayDashboardData {
  const readyPlans = plans.flatMap((plan) => {
    if (plan.status !== "ready") return [];
    const nextAction = plan.steps.find(
      (step) => step.status === "ready" && step.isNextAction,
    );
    return nextAction ? [{ plan, nextAction }] : [];
  });
  const [primaryAction, ...otherReadyPlans] = readyPlans;
  const recentlyCompleted = plans
    .flatMap((plan) =>
      plan.steps
        .filter((step) => step.status === "complete")
        .map((step) => ({ plan, step })),
    )
    .sort(
      (left, right) =>
        Date.parse(right.step.updatedAt) - Date.parse(left.step.updatedAt) ||
        right.plan.id.localeCompare(left.plan.id) ||
        right.step.position - left.step.position,
    )
    .slice(0, RECENT_COMPLETED_LIMIT);

  return {
    primaryAction,
    otherReadyPlans,
    waitingPlans: plans.filter((plan) => plan.status === "waiting"),
    recentlyCompleted,
  };
}

export function ArchivedPlans({
  onRestore,
  onRetry,
  restoreError,
  restoringPlanId,
  state,
}: {
  onRestore(planId: string): void;
  onRetry(): void;
  restoreError: string | null;
  restoringPlanId: string | null;
  state: ArchivedPlansState;
}) {
  return (
    <details className="today-archived-plans">
      <summary>
        Archived Plans
        {state.status === "ready" ? ` (${state.plans.length})` : ""}
      </summary>
      <div className="today-archived-content">
        {restoreError ? (
          <p className="today-archive-error" role="alert">
            {restoreError}
          </p>
        ) : null}
        {state.status === "loading" ? (
          <p aria-busy="true" className="dashboard-notice">
            Loading archived Plans…
          </p>
        ) : null}
        {state.status === "error" ? (
          <div className="dashboard-notice dashboard-error" role="alert">
            <p>We could not load archived Plans. Please try again.</p>
            <button
              className="button button-ghost"
              onClick={onRetry}
              type="button"
            >
              Retry
            </button>
          </div>
        ) : null}
        {state.status === "ready" && state.plans.length === 0 ? (
          <p className="dashboard-notice">No archived Plans.</p>
        ) : null}
        {state.status === "ready" && state.plans.length > 0 ? (
          <ul className="today-archive-list">
            {state.plans.map((plan) => (
              <li key={plan.id}>
                <div>
                  <strong>{plan.summary}</strong>
                  <span>{plan.status}</span>
                </div>
                <button
                  className="button button-small button-ghost"
                  disabled={restoringPlanId !== null}
                  onClick={() => onRestore(plan.id)}
                  type="button"
                >
                  {restoringPlanId === plan.id ? "Restoring…" : "Restore"}
                </button>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </details>
  );
}

export function TodayDashboard({
  onRetry,
  state,
}: {
  onRetry(): void;
  state: TodayPlansState;
}) {
  if (state.status === "loading") {
    return (
      <section aria-busy="true" className="today-dashboard">
        <p className="dashboard-notice">Loading your private Plans…</p>
      </section>
    );
  }

  if (state.status === "error") {
    return (
      <section className="today-dashboard">
        <div className="dashboard-notice dashboard-error" role="alert">
          <p>We could not load your Plans. Please try again.</p>
          <button
            className="button button-ghost"
            onClick={onRetry}
            type="button"
          >
            Retry
          </button>
        </div>
      </section>
    );
  }

  if (state.plans.length === 0) {
    return (
      <section className="today-dashboard">
        <article className="next-action-live next-action-unavailable">
          <p className="workspace-empty-kicker">Next action</p>
          <h2>Your next action will appear here.</h2>
          <p>
            Capture something, review the suggestions, and generate a Plan when
            you are ready.
          </p>
        </article>
      </section>
    );
  }

  const data = todayDashboardData(state.plans);
  return (
    <section className="today-dashboard" aria-label="Today’s Plans">
      {data.primaryAction ? (
        <article className="next-action-live">
          <p className="workspace-empty-kicker">Next action</p>
          <h2>{data.primaryAction.nextAction.title}</h2>
          <p>{data.primaryAction.nextAction.rationale}</p>
          {data.primaryAction.nextAction.dueOn ? (
            <p className="today-next-action-meta">
              Due {data.primaryAction.nextAction.dueOn}
            </p>
          ) : null}
          <Link
            className="text-link"
            href={`/plans/${data.primaryAction.plan.id}`}
          >
            Open Plan <span aria-hidden="true">→</span>
          </Link>
        </article>
      ) : (
        <article className="next-action-live next-action-unavailable">
          <p className="workspace-empty-kicker">Next action</p>
          <h2>No step is ready right now.</h2>
          <p>
            Review your Waiting Plans when the response or detail you need
            arrives.
          </p>
        </article>
      )}

      {data.otherReadyPlans.length > 0 ? (
        <DashboardSection heading="Other active Plans">
          <ul className="today-plan-list">
            {data.otherReadyPlans.map(({ plan, nextAction }) => (
              <li key={plan.id}>
                <Link href={`/plans/${plan.id}`}>
                  <span>{plan.summary}</span>
                  <strong>{nextAction.title}</strong>
                </Link>
              </li>
            ))}
          </ul>
        </DashboardSection>
      ) : null}

      {data.waitingPlans.length > 0 ? (
        <DashboardSection heading="Waiting">
          <ul className="today-plan-list">
            {data.waitingPlans.map((plan) => (
              <li key={plan.id}>
                <Link href={`/plans/${plan.id}`}>
                  <span>{plan.summary}</span>
                  <strong>
                    {plan.steps
                      .filter((step) => step.status === "waiting")
                      .map((step) => step.waitingOn)
                      .filter((detail): detail is string => Boolean(detail))
                      .join(" · ")}
                  </strong>
                </Link>
              </li>
            ))}
          </ul>
        </DashboardSection>
      ) : null}

      {data.recentlyCompleted.length > 0 ? (
        <DashboardSection heading="Recently completed">
          <ul className="today-plan-list">
            {data.recentlyCompleted.map(({ plan, step }) => (
              <li key={step.id}>
                <Link href={`/plans/${plan.id}`}>
                  <span>{plan.summary}</span>
                  <strong>{step.title}</strong>
                </Link>
              </li>
            ))}
          </ul>
        </DashboardSection>
      ) : null}
    </section>
  );
}

function DashboardSection({
  children,
  heading,
}: {
  children: ReactNode;
  heading: string;
}) {
  return (
    <section className="today-dashboard-section">
      <h2>{heading}</h2>
      {children}
    </section>
  );
}
