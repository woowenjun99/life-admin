"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useRef, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import {
  type CaptureResult,
  createTextCapture,
  uploadFileCapture,
  validateCaptureFile,
} from "@/lib/api";
import { captureErrorMessage } from "@/lib/capture";
import {
  type CaptureMode,
  captureModeFromSearchParam,
} from "@/lib/capture-mode";
import { focusTrapTargetIndex } from "@/lib/focus-trap";

type CaptureState =
  | { status: "idle" }
  | { status: "submitting" }
  | { status: "success"; message: string }
  | { status: "error"; message: string };

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type=hidden])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

type CaptureFormsProps = {
  onCaptured?(result: CaptureResult): void;
};

export function CaptureForms({ onCaptured }: CaptureFormsProps) {
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const [mode, setMode] = useState<CaptureMode | null>(null);
  const launchButtonRef = useRef<HTMLButtonElement>(null);
  const requestedMode = captureModeFromSearchParam(searchParams.get("capture"));

  useEffect(() => {
    if (requestedMode) {
      setMode(requestedMode);
    }
  }, [requestedMode]);

  function closeModal() {
    setMode(null);
    if (requestedMode) {
      router.replace(pathname, { scroll: false });
    }
    requestAnimationFrame(() => launchButtonRef.current?.focus());
  }

  return (
    <>
      <section className="capture-launcher">
        <div className="capture-launcher-copy">
          <p className="workspace-empty-kicker">Start a private capture</p>
          <h2>Save the thing you want to remember.</h2>
          <p>
            Capture a note or one supported file. Files are checked before they
            are stored.
          </p>
          <div className="capture-launcher-actions">
            <button
              className="button button-primary"
              onClick={() => setMode("text")}
              ref={launchButtonRef}
              type="button"
            >
              Save a note <span aria-hidden="true">↗</span>
            </button>
            <button
              className="button button-ghost"
              onClick={() => setMode("file")}
              type="button"
            >
              Upload a file
            </button>
          </div>
        </div>
        <aside
          aria-label="How Life Inbox works"
          className="capture-launcher-guide"
        >
          <p className="workspace-empty-kicker">A private flow</p>
          <ol>
            <li>
              <span>1</span>
              <div>
                <strong>Capture it</strong>
                <p>Save the loose end while it is fresh.</p>
              </div>
            </li>
            <li>
              <span>2</span>
              <div>
                <strong>Review it</strong>
                <p>Adjust every suggestion before a Plan is made.</p>
              </div>
            </li>
            <li>
              <span>3</span>
              <div>
                <strong>Choose one next action</strong>
                <p>Nothing happens outside Life Inbox.</p>
              </div>
            </li>
          </ol>
        </aside>
      </section>

      {mode ? (
        <CaptureModal
          initialMode={mode}
          onCaptured={onCaptured}
          onClose={closeModal}
        />
      ) : null}
    </>
  );
}

function CaptureModal({
  initialMode,
  onCaptured,
  onClose,
}: {
  initialMode: CaptureMode;
  onCaptured?(result: CaptureResult): void;
  onClose(): void;
}) {
  const { user } = useAuth();
  const router = useRouter();
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const [mode, setMode] = useState<CaptureMode>(initialMode);
  const [text, setText] = useState("");
  const [textState, setTextState] = useState<CaptureState>({ status: "idle" });
  const [file, setFile] = useState<File | null>(null);
  const [fileValidation, setFileValidation] = useState<string | null>(null);
  const [fileState, setFileState] = useState<CaptureState>({ status: "idle" });

  useEffect(() => {
    closeButtonRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
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
  }, [onClose]);

  async function submitText(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!user) {
      setTextState({
        status: "error",
        message: "Your session has ended. Sign in again.",
      });
      return;
    }
    if (!text.trim()) {
      setTextState({
        status: "error",
        message: "Write a note before saving it.",
      });
      return;
    }

    setTextState({ status: "submitting" });
    try {
      const result = await createTextCapture(user, text);
      setText("");
      onCaptured?.(result);
      if (result.extraction === "ready") {
        router.push(`/inbox/${result.item.id}/review`);
        return;
      }
      setTextState({
        status: "success",
        message:
          result.extraction === "retryable"
            ? "Private note saved. Sorting is unavailable; retry it from your Inbox."
            : "Private note captured.",
      });
    } catch (error) {
      setTextState({ status: "error", message: captureErrorMessage(error) });
    }
  }

  function selectFile(event: React.ChangeEvent<HTMLInputElement>) {
    const selectedFile = event.target.files?.[0] ?? null;
    const validation = selectedFile ? validateCaptureFile(selectedFile) : null;
    setFile(selectedFile);
    setFileValidation(validation);
    setFileState({ status: "idle" });
  }

  async function submitFile(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!user) {
      setFileState({
        status: "error",
        message: "Your session has ended. Sign in again.",
      });
      return;
    }
    if (!file) {
      setFileValidation("Choose a PDF, JPEG, or PNG file.");
      return;
    }
    const validation = validateCaptureFile(file);
    if (validation) {
      setFileValidation(validation);
      return;
    }

    setFileState({ status: "submitting" });
    try {
      const result = await uploadFileCapture(user, file);
      setFile(null);
      setFileValidation(null);
      if (fileInput.current) {
        fileInput.current.value = "";
      }
      onCaptured?.(result);
      if (result.extraction === "ready") {
        router.push(`/inbox/${result.item.id}/review`);
        return;
      }
      setFileState({
        status: "success",
        message:
          result.extraction === "retryable"
            ? "Private PDF saved. Sorting is unavailable; retry it from your Inbox."
            : "Private file saved. Sorting is not available for this file type.",
      });
    } catch (error) {
      setFileState({ status: "error", message: captureErrorMessage(error) });
    }
  }

  const textIsSubmitting = textState.status === "submitting";
  const fileIsSubmitting = fileState.status === "submitting";

  return (
    <div className="capture-modal-backdrop">
      <button
        aria-hidden="true"
        aria-label="Close capture"
        className="capture-modal-backdrop-dismiss"
        onClick={onClose}
        tabIndex={-1}
        type="button"
      />
      <div
        aria-labelledby="capture-dialog-heading"
        aria-modal="true"
        className="capture-modal"
        ref={dialogRef}
        role="dialog"
      >
        <button
          aria-label="Close capture"
          className="capture-modal-close"
          onClick={onClose}
          ref={closeButtonRef}
          type="button"
        >
          <span aria-hidden="true">×</span>
        </button>
        <p className="workspace-empty-kicker">Private Inbox capture</p>
        <h2 id="capture-dialog-heading">Save it while it is on your mind.</h2>
        <fieldset className="capture-mode-switcher">
          <legend className="visually-hidden">Capture type</legend>
          <button
            aria-pressed={mode === "text"}
            className={
              mode === "text"
                ? "capture-mode-button is-active"
                : "capture-mode-button"
            }
            onClick={() => setMode("text")}
            type="button"
          >
            Note
          </button>
          <button
            aria-pressed={mode === "file"}
            className={
              mode === "file"
                ? "capture-mode-button is-active"
                : "capture-mode-button"
            }
            onClick={() => setMode("file")}
            type="button"
          >
            File
          </button>
        </fieldset>

        {mode === "text" ? (
          <form className="capture-form" onSubmit={submitText}>
            <div className="capture-form-heading">
              <h3>Save a note</h3>
              <p>
                Keep a thought, reminder, or loose end in your private Inbox.
                Notes are sent to the configured AI provider to draft
                suggestions for your review.
              </p>
            </div>
            <label htmlFor="capture-text">What do you want to remember?</label>
            <textarea
              disabled={textIsSubmitting}
              id="capture-text"
              maxLength={10_000}
              onChange={(event) => setText(event.target.value)}
              placeholder="Remember to renew the passport before the trip…"
              required
              rows={6}
              value={text}
            />
            <div className="capture-form-footer">
              <span aria-live="polite" className="capture-character-count">
                {text.length}/10,000
              </span>
              <button
                className="button button-primary"
                disabled={textIsSubmitting}
                type="submit"
              >
                {textIsSubmitting ? "Saving…" : "Save private note"}
              </button>
            </div>
            <CaptureNotice state={textState} />
          </form>
        ) : (
          <form className="capture-form" onSubmit={submitFile}>
            <div className="capture-form-heading">
              <h3>Save one file</h3>
              <p>
                PDF, JPEG, or PNG only. Maximum 10 MiB. PDFs are sent to the
                configured AI provider to draft suggestions for your review;
                images are stored only.
              </p>
            </div>
            <label htmlFor="capture-file">Choose a file</label>
            <input
              accept="application/pdf,image/jpeg,image/png"
              aria-describedby={
                fileValidation ? "capture-file-validation" : undefined
              }
              disabled={fileIsSubmitting}
              id="capture-file"
              onChange={selectFile}
              ref={fileInput}
              type="file"
            />
            {fileValidation ? (
              <p
                className="form-error"
                id="capture-file-validation"
                role="alert"
              >
                {fileValidation}
              </p>
            ) : null}
            <div className="capture-form-footer">
              <span aria-live="polite" className="capture-character-count">
                {file ? file.name : "No file selected"}
              </span>
              <button
                className="button button-primary"
                disabled={fileIsSubmitting || Boolean(fileValidation)}
                type="submit"
              >
                {fileIsSubmitting ? "Saving…" : "Capture private file"}
              </button>
            </div>
            <CaptureNotice state={fileState} />
          </form>
        )}
      </div>
    </div>
  );
}

function CaptureNotice({ state }: { state: CaptureState }) {
  if (state.status !== "success" && state.status !== "error") {
    return null;
  }

  return (
    <p
      className={state.status === "error" ? "form-error" : "capture-success"}
      role={state.status === "error" ? "alert" : "status"}
    >
      {state.message}
    </p>
  );
}
