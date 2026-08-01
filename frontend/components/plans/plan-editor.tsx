"use client";

import { type FormEvent, useState } from "react";

import type { Plan, PlanDraftStep, PlanUpdate } from "@/lib/api";

function initialSteps(plan: Plan): PlanDraftStep[] {
  return plan.steps.map((step) => ({
    id: step.id,
    title: step.title,
    rationale: step.rationale,
    status: step.status,
    dueOn: step.dueOn,
    waitingOn: step.waitingOn,
  }));
}

const newStep = (): PlanDraftStep => ({
  title: "",
  rationale: "",
  status: "ready",
});

type PlanEditorProps = {
  plan: Plan;
  isSaving: boolean;
  onCancel: () => void;
  onSave: (update: PlanUpdate) => void;
};

export function PlanEditor({
  plan,
  isSaving,
  onCancel,
  onSave,
}: PlanEditorProps) {
  const [summary, setSummary] = useState(plan.summary);
  const [steps, setSteps] = useState<PlanDraftStep[]>(() => initialSteps(plan));
  const [error, setError] = useState<string | null>(null);

  const updateStep = (index: number, patch: Partial<PlanDraftStep>) => {
    setSteps((current) =>
      current.map((step, currentIndex) =>
        currentIndex === index ? { ...step, ...patch } : step,
      ),
    );
  };

  const moveStep = (index: number, direction: -1 | 1) => {
    setSteps((current) => {
      const destination = index + direction;
      if (destination < 0 || destination >= current.length) return current;
      const next = [...current];
      const [step] = next.splice(index, 1);
      if (!step) return current;
      next.splice(destination, 0, step);
      return next;
    });
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const normalizedSummary = summary.trim();
    const normalizedSteps = steps.map((step) => ({
      ...step,
      title: step.title.trim(),
      rationale: step.rationale.trim(),
      dueOn: step.dueOn?.trim() || undefined,
      waitingOn: step.waitingOn?.trim() || undefined,
    }));
    if (!normalizedSummary || normalizedSteps.some((step) => !step.title || !step.rationale)) {
      setError("Add a summary, title, and rationale for every step.");
      return;
    }
    if (normalizedSteps.some((step) => step.status === "waiting" && !step.waitingOn)) {
      setError("Explain what every Waiting step is waiting on.");
      return;
    }
    if (normalizedSteps.some((step) => step.status !== "waiting" && step.waitingOn)) {
      setError("Only Waiting steps can include a waiting-on detail.");
      return;
    }
    setError(null);
    onSave({
      expectedRevision: plan.revision,
      summary: normalizedSummary,
      steps: normalizedSteps,
    });
  };

  return (
    <form className="plan-editor" onSubmit={submit}>
      <div className="review-section-heading">
        <div>
          <p className="workspace-empty-kicker">Edit Plan</p>
          <h2>Make the Plan fit real life.</h2>
        </div>
        <p className="plan-editor-note">Changes are saved together as one revision.</p>
      </div>
      <label className="plan-editor-summary" htmlFor="plan-summary">
        Summary
        <textarea
          disabled={isSaving}
          id="plan-summary"
          maxLength={2000}
          onChange={(event) => setSummary(event.target.value)}
          required
          value={summary}
        />
      </label>
      <ol className="plan-editor-steps">
        {steps.map((step, index) => (
          <li key={step.id ?? `new-step-${index}`}>
            <div className="plan-editor-step-heading">
              <p>Step {index + 1}</p>
              <div className="plan-editor-order-actions">
                <button
                  className="button button-ghost plan-step-button"
                  disabled={isSaving || index === 0}
                  onClick={() => moveStep(index, -1)}
                  type="button"
                >
                  Move up
                </button>
                <button
                  className="button button-ghost plan-step-button"
                  disabled={isSaving || index === steps.length - 1}
                  onClick={() => moveStep(index, 1)}
                  type="button"
                >
                  Move down
                </button>
                <button
                  className="button button-ghost plan-step-button"
                  disabled={isSaving || steps.length === 1}
                  onClick={() => setSteps((current) => current.filter((_, currentIndex) => currentIndex !== index))}
                  type="button"
                >
                  Remove
                </button>
              </div>
            </div>
            <div className="plan-editor-grid">
              <label htmlFor={`plan-step-title-${index}`}>
                Step
                <input
                  disabled={isSaving}
                  id={`plan-step-title-${index}`}
                  maxLength={2000}
                  onChange={(event) => updateStep(index, { title: event.target.value })}
                  required
                  value={step.title}
                />
              </label>
              <label htmlFor={`plan-step-status-${index}`}>
                Status
                <select
                  disabled={isSaving || step.status === "complete"}
                  id={`plan-step-status-${index}`}
                  onChange={(event) =>
                    updateStep(index, {
                      status: event.target.value as PlanDraftStep["status"],
                      waitingOn:
                        event.target.value === "waiting" ? step.waitingOn : undefined,
                    })
                  }
                  value={step.status}
                >
                  {step.status === "complete" ? (
                    <option value="complete">Complete</option>
                  ) : (
                    <>
                      <option value="ready">Ready</option>
                      <option value="waiting">Waiting</option>
                      <option value="complete">Complete</option>
                    </>
                  )}
                </select>
              </label>
              <label htmlFor={`plan-step-rationale-${index}`}>
                Why it matters
                <textarea
                  disabled={isSaving}
                  id={`plan-step-rationale-${index}`}
                  maxLength={2000}
                  onChange={(event) => updateStep(index, { rationale: event.target.value })}
                  required
                  value={step.rationale}
                />
              </label>
              <label htmlFor={`plan-step-due-${index}`}>
                Due date <span>(optional)</span>
                <input
                  disabled={isSaving}
                  id={`plan-step-due-${index}`}
                  onChange={(event) => updateStep(index, { dueOn: event.target.value || undefined })}
                  type="date"
                  value={step.dueOn ?? ""}
                />
              </label>
              {step.status === "waiting" ? (
                <label htmlFor={`plan-step-waiting-${index}`}>
                  Waiting on
                  <input
                    disabled={isSaving}
                    id={`plan-step-waiting-${index}`}
                    maxLength={2000}
                    onChange={(event) => updateStep(index, { waitingOn: event.target.value })}
                    required
                    value={step.waitingOn ?? ""}
                  />
                </label>
              ) : null}
            </div>
          </li>
        ))}
      </ol>
      {error ? <p className="workspace-error plan-action-error" role="alert">{error}</p> : null}
      <div className="plan-editor-actions">
        <button
          className="button button-ghost"
          disabled={isSaving || steps.length >= 20}
          onClick={() => setSteps((current) => [...current, newStep()])}
          type="button"
        >
          Add step
        </button>
        <div>
          <button className="button button-ghost" disabled={isSaving} onClick={onCancel} type="button">
            Cancel
          </button>
          <button className="button button-primary" disabled={isSaving} type="submit">
            {isSaving ? "Saving…" : "Save Plan"}
          </button>
        </div>
      </div>
    </form>
  );
}
