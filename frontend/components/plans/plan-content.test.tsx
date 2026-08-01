import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";

import type { Plan, PlanStep } from "@/lib/api";

import { ArchivePlanDialog } from "./archive-plan-dialog";
import { PlanConversation } from "./plan-conversation";
import { PlanEditor } from "./plan-editor";
import { PlanStepControls, planStepStatusLabel } from "./plan-step-controls";

const handlers = {
  onCancelWaiting: () => undefined,
  onMakeReady: () => undefined,
  onMarkComplete: () => undefined,
  onStartWaiting: () => undefined,
  onSubmitWaiting: () => undefined,
  onWaitingOnChange: () => undefined,
};

function step(status: PlanStep["status"], isNextAction = false): PlanStep {
  return {
    id: "step-123",
    position: 0,
    title: "Check requirements",
    rationale: "Confirm the deadline.",
    status,
    dueOn: undefined,
    waitingOn: status === "waiting" ? "A reply from the agency" : undefined,
    isNextAction,
    updatedAt: "2026-07-31T00:00:00Z",
  };
}

function plan(): Plan {
  return {
    id: "plan-123",
    inboxItemId: "item-123",
    summary: "Renew before travelling.",
    status: "ready",
    revision: 1,
    steps: [step("ready", true)],
    createdAt: "2026-07-31T00:00:00Z",
    updatedAt: "2026-07-31T00:00:00Z",
  };
}

test("Plan step controls keep the status pill and actions in one aligned aside", () => {
  const markup = renderToStaticMarkup(
    <PlanStepControls
      {...handlers}
      isSaving={false}
      step={step("ready", true)}
      waitingOn="A reply from the agency"
      waitingOpen
    />,
  );

  expect(markup).toContain('class="plan-step-aside"');
  expect(markup).toContain('class="plan-step-status"');
  expect(markup).toContain("Next action");
  expect(markup).toContain("Mark waiting");
  expect(markup).toContain("Mark complete");
  expect(markup).toContain('for="waiting-on-step-123"');
  expect(markup).toContain('id="waiting-on-step-123"');
  expect(markup).toContain("Save Waiting");
  expect(markup).toContain("required");
});

test("Plan step controls represent Waiting and completed steps accurately", () => {
  const waitingMarkup = renderToStaticMarkup(
    <PlanStepControls
      {...handlers}
      isSaving={false}
      step={step("waiting")}
      waitingOn="A reply from the agency"
      waitingOpen={false}
    />,
  );
  expect(waitingMarkup).toContain("Make ready");
  expect(waitingMarkup).toContain("Mark complete");
  const completeMarkup = renderToStaticMarkup(
    <PlanStepControls
      {...handlers}
      isSaving={false}
      step={step("complete")}
      waitingOn=""
      waitingOpen={false}
    />,
  );
  expect(completeMarkup).toContain("Complete");
  expect(completeMarkup).not.toContain("Mark complete");
  expect(completeMarkup).not.toContain("Reopen");
  expect(planStepStatusLabel(step("ready", true))).toBe("Next action");
  expect(planStepStatusLabel(step("waiting"))).toBe("Waiting");
  expect(planStepStatusLabel(step("complete"))).toBe("Complete");
});

test("archiving requires an accessible confirmation with cancel and error states", () => {
  const markup = renderToStaticMarkup(
    <ArchivePlanDialog
      error="We could not archive this Plan. Please try again."
      isArchiving={false}
      onClose={() => undefined}
      onConfirm={() => undefined}
    />,
  );

  expect(markup).toContain('role="alertdialog"');
  expect(markup).toContain("Archive this Plan?");
  expect(markup).toContain("You can restore the same Plan and steps");
  expect(markup).toContain("Cancel");
  expect(markup).toContain("Archive Plan");
  expect(markup).toContain("We could not archive this Plan.");
});

test("Plan editor and discussion retain explicit revision and approval controls", () => {
  const editor = renderToStaticMarkup(
    <PlanEditor
      isSaving={false}
      onCancel={() => undefined}
      onSave={() => undefined}
      plan={plan()}
    />,
  );
  expect(editor).toContain("Move up");
  expect(editor).toContain("Move down");
  expect(editor).toContain("Save Plan");

  const discussion = renderToStaticMarkup(
    <PlanConversation
      onPlanUpdated={() => undefined}
      onReloadPlan={() => undefined}
      plan={plan()}
      user={{ getIdToken: async () => "token" }}
    />,
  );
  expect(discussion).toContain("Discuss Plan");
});
