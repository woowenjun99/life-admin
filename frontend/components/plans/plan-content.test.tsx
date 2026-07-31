import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";

import type { PlanStep } from "@/lib/api";

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
  };
}

test("Plan step controls expose Complete and Waiting actions with a labelled reason", () => {
  const markup = renderToStaticMarkup(
    <PlanStepControls
      {...handlers}
      isSaving={false}
      step={step("ready", true)}
      waitingOn="A reply from the agency"
      waitingOpen
    />,
  );

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
  expect(
    renderToStaticMarkup(
      <PlanStepControls
        {...handlers}
        isSaving={false}
        step={step("complete")}
        waitingOn=""
        waitingOpen={false}
      />,
    ),
  ).toBe("");
  expect(planStepStatusLabel(step("ready", true))).toBe("Next action");
  expect(planStepStatusLabel(step("waiting"))).toBe("Waiting");
  expect(planStepStatusLabel(step("complete"))).toBe("Complete");
});
