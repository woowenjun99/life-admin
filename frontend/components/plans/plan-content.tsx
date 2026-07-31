"use client";

import Link from "next/link";
import { type FormEvent, useCallback, useEffect, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import { fetchPlan, type Plan, type PlanStep, updatePlanStep } from "@/lib/api";
import { PlanStepControls } from "./plan-step-controls";

export function PlanContent({ planId }: { planId: string }) {
  const { user } = useAuth();
  const [plan, setPlan] = useState<Plan | null>(null);
  const [error, setError] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [pendingStepId, setPendingStepId] = useState<string | null>(null);
  const [waitingStepId, setWaitingStepId] = useState<string | null>(null);
  const [waitingOn, setWaitingOn] = useState("");
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

  const updateStep = useCallback(
    async (
      step: PlanStep,
      status: PlanStep["status"],
      waitingOnDetail: string | null,
    ) => {
      if (!user) return;
      setPendingStepId(step.id);
      setActionError(null);
      try {
        setPlan(
          await updatePlanStep(user, planId, step.id, {
            status,
            waitingOn: waitingOnDetail,
          }),
        );
        setWaitingStepId(null);
        setWaitingOn("");
      } catch {
        setActionError("We could not update this Plan step. Please try again.");
      } finally {
        setPendingStepId(null);
      }
    },
    [planId, user],
  );

  const startWaiting = (step: PlanStep) => {
    setActionError(null);
    setWaitingStepId(step.id);
    setWaitingOn(step.waitingOn ?? "");
  };

  const submitWaiting = (event: FormEvent<HTMLFormElement>, step: PlanStep) => {
    event.preventDefault();
    const detail = waitingOn.trim();
    if (!detail) {
      setActionError("Add what this step is waiting on before saving it.");
      return;
    }
    void updateStep(step, "waiting", detail);
  };

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
  const isSaving = pendingStepId !== null;
  return (
    <section className="workspace-panel plan-panel">
      <div className="review-heading">
        <div>
          <p className="workspace-empty-kicker">Your approved Plan</p>
          <h1>One clear next action.</h1>
        </div>
        <Link className="text-link" href="/today">
          Back to Today <span aria-hidden="true">←</span>
        </Link>
      </div>
      <p className="plan-summary">{plan.summary}</p>
      {nextAction ? (
        <article className="next-action-live">
          <p className="workspace-empty-kicker">Next action</p>
          <h2>{nextAction.title}</h2>
          <p>{nextAction.rationale}</p>
        </article>
      ) : (
        <article className="next-action-live next-action-unavailable">
          <p className="workspace-empty-kicker">Next action</p>
          <h2>
            {plan.status === "complete"
              ? "This Plan is complete."
              : "No step is ready right now."}
          </h2>
          <p>
            {plan.status === "complete"
              ? "You completed every step in this Plan."
              : "Review the Waiting steps when the information or response you need arrives."}
          </p>
        </article>
      )}
      {actionError ? (
        <p className="workspace-error plan-action-error" role="alert">
          {actionError}
        </p>
      ) : null}
      <ol className="plan-steps-live">
        {plan.steps.map((step) => (
          <li
            aria-busy={pendingStepId === step.id}
            className={
              step.status === "waiting"
                ? "is-waiting"
                : step.status === "complete"
                  ? "is-complete"
                  : ""
            }
            key={step.id}
          >
            <div className="plan-step-copy">
              <p className="plan-step-title">{step.title}</p>
              <p>{step.rationale}</p>
              {step.dueOn ? (
                <p className="plan-step-meta">Due {step.dueOn}</p>
              ) : null}
              {step.waitingOn ? (
                <p className="plan-step-meta">Waiting on {step.waitingOn}</p>
              ) : null}
            </div>
            <PlanStepControls
              isSaving={isSaving}
              onCancelWaiting={() => {
                setActionError(null);
                setWaitingStepId(null);
                setWaitingOn("");
              }}
              onMakeReady={(current) => void updateStep(current, "ready", null)}
              onMarkComplete={(current) =>
                void updateStep(current, "complete", null)
              }
              onStartWaiting={startWaiting}
              onSubmitWaiting={submitWaiting}
              onWaitingOnChange={setWaitingOn}
              step={step}
              waitingOn={waitingOn}
              waitingOpen={waitingStepId === step.id}
            />
          </li>
        ))}
      </ol>
      <p className="review-safety">
        This Plan is a guide for you. Updating its steps never sends messages,
        creates events, or takes action outside Life Inbox.
      </p>
    </section>
  );
}
