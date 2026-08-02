import type { FormEvent } from "react";

import type { PlanStep } from "@/lib/api";

export function planStepStatusLabel(step: PlanStep): string {
  if (step.status === "complete") return "Complete";
  if (step.status === "waiting") return "Waiting";
  return step.isNextAction ? "Next action" : "Ready";
}

type PlanStepControlsProps = {
  step: PlanStep;
  isSaving: boolean;
  waitingOpen: boolean;
  waitingOn: string;
  onMakeReady: (step: PlanStep) => void;
  onMarkComplete: (step: PlanStep) => void;
  onStartWaiting: (step: PlanStep) => void;
  onWaitingOnChange: (value: string) => void;
  onSubmitWaiting: (event: FormEvent<HTMLFormElement>, step: PlanStep) => void;
  onCancelWaiting: () => void;
};

export function PlanStepControls({
  step,
  isSaving,
  waitingOpen,
  waitingOn,
  onMakeReady,
  onMarkComplete,
  onStartWaiting,
  onWaitingOnChange,
  onSubmitWaiting,
  onCancelWaiting,
}: PlanStepControlsProps) {
  return (
    <div className="plan-step-aside">
      <div className="plan-step-status">
        <span>{planStepStatusLabel(step)}</span>
      </div>
      {step.status !== "complete" ? (
        <>
          <div className="plan-step-actions">
            {step.status === "waiting" ? (
              <button
                className="button button-ghost plan-step-button"
                disabled={isSaving}
                onClick={() => onMakeReady(step)}
                type="button"
              >
                Make ready
              </button>
            ) : !waitingOpen ? (
              <button
                className="button button-ghost plan-step-button"
                disabled={isSaving}
                onClick={() => onStartWaiting(step)}
                type="button"
              >
                Mark waiting
              </button>
            ) : null}
            <button
              className="button button-primary plan-step-button"
              disabled={isSaving}
              onClick={() => onMarkComplete(step)}
              type="button"
            >
              Mark complete
            </button>
          </div>
          {waitingOpen ? (
            <form
              className="plan-waiting-form"
              onSubmit={(event) => onSubmitWaiting(event, step)}
            >
              <p className="plan-waiting-form-title">
                What’s blocking this step?
              </p>
              <p
                className="plan-waiting-form-description"
                id={`waiting-on-description-${step.id}`}
              >
                Use Waiting only when you need a response, decision, or detail
                from someone or something else.
              </p>
              <label htmlFor={`waiting-on-${step.id}`}>
                What are you waiting for?
              </label>
              <input
                aria-describedby={`waiting-on-description-${step.id}`}
                disabled={isSaving}
                id={`waiting-on-${step.id}`}
                maxLength={2000}
                onChange={(event) => onWaitingOnChange(event.target.value)}
                placeholder="e.g. Confirmation from the venue"
                required
                value={waitingOn}
              />
              <div className="plan-step-actions">
                <button
                  className="button button-primary plan-step-button"
                  disabled={isSaving}
                  type="submit"
                >
                  Save as waiting
                </button>
                <button
                  className="button button-ghost plan-step-button"
                  disabled={isSaving}
                  onClick={onCancelWaiting}
                  type="button"
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : null}
        </>
      ) : null}
    </div>
  );
}
