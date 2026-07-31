import type { Ref } from "react";

export type CaptureLauncherVariant = "default" | "first_task";

export function CaptureLauncher({
  onCaptureFile,
  onCaptureText,
  primaryButtonRef,
  variant,
}: {
  onCaptureFile(): void;
  onCaptureText(): void;
  primaryButtonRef?: Ref<HTMLButtonElement>;
  variant: CaptureLauncherVariant;
}) {
  const firstTask = variant === "first_task";
  const steps = firstTask
    ? [
        ["Capture something", "Save one loose end while it is fresh."],
        ["Review it", "Adjust every suggestion before you make a Plan."],
        [
          "Get one next action",
          "Nothing happens outside Life Inbox unless you choose it.",
        ],
      ]
    : [
        ["Capture it", "Save the loose end while it is fresh."],
        ["Review it", "Adjust every suggestion before a Plan is made."],
        ["Choose one next action", "Nothing happens outside Life Inbox."],
      ];

  return (
    <section
      className={
        firstTask
          ? "capture-launcher capture-launcher-first-task"
          : "capture-launcher"
      }
    >
      <div className="capture-launcher-copy">
        <p className="workspace-empty-kicker">
          {firstTask ? "Your first task" : "Start a private capture"}
        </p>
        <h2>
          {firstTask
            ? "Turn one loose end into a clear next action."
            : "Save the thing you want to remember."}
        </h2>
        <p>
          {firstTask
            ? "Start with a note or one supported file. You will review every suggestion before a Plan is made."
            : "Capture a note or one supported file. Files are checked before they are stored."}
        </p>
        <div className="capture-launcher-actions">
          <button
            className="button button-primary"
            onClick={onCaptureText}
            ref={primaryButtonRef}
            type="button"
          >
            {firstTask ? (
              "Capture something"
            ) : (
              <>
                Save a note <span aria-hidden="true">↗</span>
              </>
            )}
          </button>
          <button
            className="button button-ghost"
            onClick={onCaptureFile}
            type="button"
          >
            Upload a file
          </button>
        </div>
      </div>
      <aside
        aria-label={
          firstTask ? "Your first Life Inbox task" : "How Life Inbox works"
        }
        className="capture-launcher-guide"
      >
        <p className="workspace-empty-kicker">
          {firstTask ? "Your path forward" : "A private flow"}
        </p>
        <ol>
          {steps.map(([title, detail], index) => (
            <li key={title}>
              <span>{index + 1}</span>
              <div>
                <strong>{title}</strong>
                <p>{detail}</p>
              </div>
            </li>
          ))}
        </ol>
      </aside>
    </section>
  );
}
