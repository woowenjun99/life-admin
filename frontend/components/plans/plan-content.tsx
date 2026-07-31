"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import { fetchPlan, type Plan } from "@/lib/api";

export function PlanContent({ planId }: { planId: string }) {
  const { user } = useAuth();
  const [plan, setPlan] = useState<Plan | null>(null);
  const [error, setError] = useState(false);
  const load = useCallback(async () => {
    if (!user) return;
    try {
      setPlan(await fetchPlan(user, planId));
      setError(false);
    } catch {
      setError(true);
    }
  }, [planId, user]);
  useEffect(() => {
    void load();
  }, [load]);

  if (!plan && !error)
    return (
      <section aria-busy="true" className="workspace-panel plan-panel">
        <p>Opening your Plan…</p>
      </section>
    );
  if (error || !plan)
    return (
      <section className="workspace-panel plan-panel">
        <div className="workspace-error" role="alert">
          <p>We could not load this private Plan.</p>
          <button
            className="button button-ghost"
            onClick={() => void load()}
            type="button"
          >
            Retry
          </button>
        </div>
      </section>
    );
  const nextAction = plan.steps.find((step) => step.isNextAction);
  return (
    <section className="workspace-panel plan-panel">
      <div className="review-heading">
        <div>
          <p className="workspace-empty-kicker">Your approved Plan</p>
          <h1>One clear next action.</h1>
        </div>
        <Link className="text-link" href="/today">
          Back to Inbox <span aria-hidden="true">←</span>
        </Link>
      </div>
      <p className="plan-summary">{plan.summary}</p>
      {nextAction ? (
        <article className="next-action-live">
          <p className="workspace-empty-kicker">Next action</p>
          <h2>{nextAction.title}</h2>
          <p>{nextAction.rationale}</p>
        </article>
      ) : null}
      <ol className="plan-steps-live">
        {plan.steps.map((step) => (
          <li
            className={step.status === "waiting" ? "is-waiting" : ""}
            key={step.id}
          >
            <div>
              <p className="plan-step-title">{step.title}</p>
              <p>{step.rationale}</p>
              {step.dueOn ? (
                <p className="plan-step-meta">Due {step.dueOn}</p>
              ) : null}
              {step.waitingOn ? (
                <p className="plan-step-meta">Waiting on {step.waitingOn}</p>
              ) : null}
            </div>
            <span>
              {step.status === "waiting"
                ? "Waiting"
                : step.isNextAction
                  ? "Next action"
                  : "Ready"}
            </span>
          </li>
        ))}
      </ol>
      <p className="review-safety">
        This Plan is a guide for you. Completing or waiting on steps is the next
        increment.
      </p>
    </section>
  );
}
