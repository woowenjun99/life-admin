"use client";

import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";

import type { AuthMode } from "@/lib/auth";
import { focusTrapTargetIndex } from "@/lib/focus-trap";

import { AuthForm } from "./auth-form";

type AuthModalProps = {
  initialMode: AuthMode;
};

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type=hidden])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

export function AuthModal({ initialMode }: AuthModalProps) {
  const router = useRouter();
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const [mode, setMode] = useState(initialMode);

  useEffect(() => {
    setMode(initialMode);
  }, [initialMode]);

  useEffect(() => {
    closeButtonRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        router.replace("/", { scroll: false });
        return;
      }

      if (event.key !== "Tab") {
        return;
      }

      const dialog = dialogRef.current;
      if (!dialog) {
        return;
      }

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
  }, [router]);

  function closeModal() {
    router.replace("/", { scroll: false });
  }

  return (
    <div className="auth-modal-backdrop">
      <button
        aria-hidden="true"
        aria-label="Close sign in"
        className="auth-modal-backdrop-dismiss"
        onClick={closeModal}
        tabIndex={-1}
        type="button"
      />
      <div
        aria-labelledby="auth-dialog-heading"
        aria-modal="true"
        className="auth-modal"
        ref={dialogRef}
        role="dialog"
      >
        <button
          aria-label="Close sign in"
          className="auth-modal-close"
          onClick={closeModal}
          ref={closeButtonRef}
          type="button"
        >
          <span aria-hidden="true">×</span>
        </button>
        <AuthForm mode={mode} onModeChange={setMode} />
      </div>
    </div>
  );
}
