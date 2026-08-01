"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import {
  archivePlan,
  fetchPlan,
  type Plan,
  type PlanStep,
  type PlanUpdate,
  updatePlan,
  updatePlanStep,
} from "@/lib/api";
import { restoreFocusAfterDialogClose } from "@/lib/focus-trap";
import { ArchivePlanDialog } from "./archive-plan-dialog";
import { PlanConversation } from "./plan-conversation";
import { PlanEditor } from "./plan-editor";
import { PlanStepControls } from "./plan-step-controls";

export function PlanContent({ planId }: { planId: string }) {
  const { user } = useAuth();
  const router = useRouter();
  const [plan, setPlan] = useState<Plan | null>(null);
  const [error, setError] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [archiveOpen, setArchiveOpen] = useState(false);
  const [archiveError, setArchiveError] = useState<string | null>(null);
  const [isArchiving, setIsArchiving] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isUpdatingPlan, setIsUpdatingPlan] = useState(false);
  const archiveButtonRef = useRef<HTMLButtonElement>(null);
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
            expectedRevision: plan?.revision ?? 0,
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
    [plan?.revision, planId, user],
  );

  const savePlan = useCallback(
    async (update: PlanUpdate) => {
      if (!user) return;
      setIsUpdatingPlan(true);
      setActionError(null);
      try {
        setPlan(await updatePlan(user, planId, update));
        setIsEditing(false);
      } catch (cause) {
        const message =
          cause instanceof Error && "code" in cause && cause.code === "PLAN_REVISION_CONFLICT"
            ? "This Plan changed elsewhere. Reload it before saving your edits."
            : "We could not update this Plan. Please try again.";
        setActionError(message);
        if (message.startsWith("This Plan changed")) void load();
      } finally {
        setIsUpdatingPlan(false);
      }
    },
    [load, planId, user],
  );

  const startWaiting = (step: PlanStep) => {
    setActionError(null);
    setWaitingStepId(step.id);
    setWaitingOn(step.waitingOn ?? "");
  };

  const archive = useCallback(async () => {
    if (!user) return;

    setIsArchiving(true);
    setArchiveError(null);
    try {
      await archivePlan(user, planId);
      router.replace("/today");
    } catch {
      setArchiveError("We could not archive this Plan. Please try again.");
      setIsArchiving(false);
    }
  }, [planId, router, user]);

  const closeArchiveDialog = useCallback(() => {
    if (isArchiving) return;

    setArchiveOpen(false);
    restoreFocusAfterDialogClose(archiveButtonRef.current);
  }, [isArchiving]);

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
  const isSaving = pendingStepId !== null || isUpdatingPlan;
  return (
    <section className="workspace-panel plan-panel">
      <div className="review-heading">
        <div>
          <p className="workspace-empty-kicker">Your approved Plan</p>
          <h1>One clear next action.</h1>
        </div>
        <div className="plan-heading-actions">
          <Link className="text-link" href="/today">
            Back to Today <span aria-hidden="true">←</span>
          </Link>
          <button
            className="button button-small button-ghost plan-archive-button"
            disabled={isSaving}
            onClick={() => setIsEditing(true)}
            type="button"
          >
            Edit Plan
          </button>
          <button
            className="button button-small button-ghost plan-archive-button"
            disabled={isSaving}
            onClick={() => {
              setArchiveError(null);
              setArchiveOpen(true);
            }}
            ref={archiveButtonRef}
            type="button"
          >
            Archive Plan
          </button>
        </div>
      </div>
      {actionError ? (
        <p className="workspace-error plan-action-error" role="alert">
          {actionError}
        </p>
      ) : null}
      {isEditing ? (
        <PlanEditor
          isSaving={isSaving}
          key={`${plan.id}-${plan.revision}`}
          onCancel={() => setIsEditing(false)}
          onSave={(update) => void savePlan(update)}
          plan={plan}
        />
      ) : (
        <>
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
        </>
      )}
      <PlanConversation
        onPlanUpdated={setPlan}
        onReloadPlan={() => void load()}
        plan={plan}
        user={user}
      />
      {archiveOpen ? (
        <ArchivePlanDialog
          error={archiveError}
          isArchiving={isArchiving}
          onClose={closeArchiveDialog}
          onConfirm={() => void archive()}
        />
      ) : null}
    </section>
  );
}
