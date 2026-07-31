"use client";

import { useEffect, useRef } from "react";

import { focusTrapTargetIndex } from "@/lib/focus-trap";

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type=hidden])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

export function ArchivePlanDialog({
  error,
  isArchiving,
  onClose,
  onConfirm,
}: {
  error: string | null;
  isArchiving: boolean;
  onClose(): void;
  onConfirm(): void;
}) {
  const cancelButtonRef = useRef<HTMLButtonElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    cancelButtonRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        if (!isArchiving) onClose();
        return;
      }
      if (event.key !== "Tab") return;

      const dialog = dialogRef.current;
      if (!dialog) return;
      const focusable = Array.from(
        dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
      ).filter((element) => element.tabIndex >= 0);
      const targetIndex = focusTrapTargetIndex(
        focusable.length,
        focusable.indexOf(document.activeElement as HTMLElement),
        event.shiftKey,
      );
      if (targetIndex !== null) {
        event.preventDefault();
        focusable[targetIndex]?.focus();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isArchiving, onClose]);

  return (
    <div className="plan-archive-backdrop">
      <button
        aria-hidden="true"
        aria-label="Cancel archiving"
        className="plan-archive-backdrop-dismiss"
        disabled={isArchiving}
        onClick={onClose}
        tabIndex={-1}
        type="button"
      />
      <div
        aria-describedby="archive-plan-description"
        aria-labelledby="archive-plan-heading"
        aria-modal="true"
        className="plan-archive-dialog"
        ref={dialogRef}
        role="alertdialog"
      >
        <p className="workspace-empty-kicker">Archive Plan</p>
        <h2 id="archive-plan-heading">Archive this Plan?</h2>
        <p id="archive-plan-description">
          It will be hidden from Today and your Inbox. You can restore the same
          Plan and steps from Archived Plans on Today.
        </p>
        {error ? (
          <p className="plan-archive-error" role="alert">
            {error}
          </p>
        ) : null}
        <div className="plan-archive-actions">
          <button
            className="button button-ghost"
            disabled={isArchiving}
            onClick={onClose}
            ref={cancelButtonRef}
            type="button"
          >
            Cancel
          </button>
          <button
            className="button button-primary"
            disabled={isArchiving}
            onClick={onConfirm}
            type="button"
          >
            {isArchiving ? "Archiving…" : "Archive Plan"}
          </button>
        </div>
      </div>
    </div>
  );
}
