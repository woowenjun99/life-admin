"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import {
  type EditableSuggestion,
  fetchInboxItem,
  fetchPrivatePdf,
  generatePlan,
  type InboxItemDetail,
  retryExtraction,
  type SuggestionKind,
  saveSuggestions,
} from "@/lib/api";

const SUGGESTION_KINDS: Array<{ value: SuggestionKind; label: string }> = [
  { value: "task", label: "Task" },
  { value: "date", label: "Date" },
  { value: "person", label: "Person" },
  { value: "context", label: "Context" },
  { value: "question", label: "Question" },
];

type ReviewState =
  | { status: "loading" }
  | { status: "ready"; item: InboxItemDetail }
  | { status: "error"; message: string };

type DraftSuggestion = EditableSuggestion & { localId: string };

function toEditable(item: InboxItemDetail): DraftSuggestion[] {
  return item.suggestions.map(({ id, kind, content, dueOn }) => ({
    localId: id,
    kind,
    content,
    dueOn,
  }));
}

export function ReviewContent({ itemId }: { itemId: string }) {
  const { user } = useAuth();
  const router = useRouter();
  const [state, setState] = useState<ReviewState>({ status: "loading" });
  const [draft, setDraft] = useState<DraftSuggestion[]>([]);
  const [dirty, setDirty] = useState(false);
  const [isRetrying, setIsRetrying] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!user) return;
    setState({ status: "loading" });
    try {
      const item = await fetchInboxItem(user, itemId);
      setState({ status: "ready", item });
      setDraft(toEditable(item));
      setDirty(false);
    } catch {
      setState({
        status: "error",
        message: "We could not load this private capture.",
      });
    }
  }, [itemId, user]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleRetry() {
    if (!user) return;
    setIsRetrying(true);
    setActionError(null);
    try {
      const item = await retryExtraction(user, itemId);
      setState({ status: "ready", item });
      setDraft(toEditable(item));
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : "We could not sort this capture yet.",
      );
    } finally {
      setIsRetrying(false);
    }
  }

  async function handleSave() {
    if (!user) return;
    setIsSaving(true);
    setActionError(null);
    try {
      const item = await saveSuggestions(
        user,
        itemId,
        draft.map(({ localId: _localId, ...suggestion }) => suggestion),
      );
      setState({ status: "ready", item });
      setDraft(toEditable(item));
      setDirty(false);
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : "We could not save your reviewed suggestions.",
      );
    } finally {
      setIsSaving(false);
    }
  }

  async function handleGenerate() {
    if (!user || dirty) return;
    setIsGenerating(true);
    setActionError(null);
    try {
      const plan = await generatePlan(user, itemId);
      router.push(`/plans/${plan.id}`);
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : "We could not generate a plan yet.",
      );
      setIsGenerating(false);
    }
  }

  function updateSuggestion(
    index: number,
    update: Partial<EditableSuggestion>,
  ) {
    setDraft((current) =>
      current.map((suggestion, itemIndex) =>
        itemIndex === index ? { ...suggestion, ...update } : suggestion,
      ),
    );
    setDirty(true);
  }

  function removeSuggestion(index: number) {
    setDraft((current) =>
      current.filter((_, itemIndex) => itemIndex !== index),
    );
    setDirty(true);
  }

  function addSuggestion() {
    setDraft((current) => [
      ...current,
      { localId: crypto.randomUUID(), kind: "task", content: "" },
    ]);
    setDirty(true);
  }

  if (state.status === "loading") {
    return (
      <section aria-busy="true" className="workspace-panel review-panel">
        <p>Opening your private review…</p>
      </section>
    );
  }
  if (state.status === "error") {
    return (
      <section className="workspace-panel review-panel">
        <div className="workspace-error" role="alert">
          <p>{state.message}</p>
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
  }
  if (state.item.status === "captured" && !state.item.canRetryExtraction) {
    const message =
      state.item.sourceType === "pdf"
        ? "This PDF is stored privately, but the configured AI provider cannot sort PDFs."
        : "Images are stored privately but cannot be sorted yet.";
    return (
      <section className="workspace-panel review-panel review-empty">
        <p>{message}</p>
        <Link className="button button-primary" href="/today">
          Return to Today
        </Link>
      </section>
    );
  }
  if (state.item.status === "captured") {
    return (
      <section className="workspace-panel review-panel review-empty">
        <p className="workspace-empty-kicker">Private capture</p>
        <h1>Ready when sorting is.</h1>
        <p>
          This capture is safely saved. Try again to create suggestions for your
          review.
        </p>
        {actionError ? (
          <p className="form-error" role="alert">
            {actionError}
          </p>
        ) : null}
        <button
          className="button button-primary"
          disabled={isRetrying}
          onClick={() => void handleRetry()}
          type="button"
        >
          {isRetrying ? "Sorting…" : "Try sorting again"}
        </button>
      </section>
    );
  }
  if (state.item.status === "planned") {
    return (
      <section className="workspace-panel review-panel review-empty">
        <p>This capture already has an approved plan.</p>
        {state.item.planId ? (
          <Link
            className="button button-primary"
            href={`/plans/${state.item.planId}`}
          >
            Open Plan
          </Link>
        ) : (
          <Link className="button button-primary" href="/today">
            Return to Today
          </Link>
        )}
      </section>
    );
  }
  if (state.item.sourceType === "image") {
    return (
      <section className="workspace-panel review-panel review-empty">
        <p>Images are stored privately but cannot be sorted yet.</p>
        <Link className="button button-primary" href="/today">
          Return to Today
        </Link>
      </section>
    );
  }

  return (
    <section className="workspace-panel review-panel">
      <div className="review-heading">
        <div>
          <p className="workspace-empty-kicker">Review before planning</p>
          <h1>Keep what matters. Change the rest.</h1>
        </div>
        <Link className="text-link" href="/today">
          Back to Inbox <span aria-hidden="true">←</span>
        </Link>
      </div>
      <p className="review-safety">
        These are private suggestions, not actions. Life Inbox will not contact
        anyone, schedule anything, or make changes for you.
      </p>
      <div className="review-grid">
        <OriginalCapture item={state.item} user={user} />
        <form
          className="review-suggestions"
          onSubmit={(event) => {
            event.preventDefault();
            void handleSave();
          }}
        >
          <div className="review-section-heading">
            <div>
              <p className="workspace-empty-kicker">
                Your reviewed suggestions
              </p>
              <h2>Edit every detail</h2>
            </div>
            <button className="text-link" onClick={addSuggestion} type="button">
              Add suggestion
            </button>
          </div>
          {draft.length === 0 ? (
            <p className="review-notice">
              Add a suggestion before you generate a Plan.
            </p>
          ) : null}
          <div className="suggestion-list">
            {draft.map((suggestion, index) => (
              <fieldset className="suggestion-editor" key={suggestion.localId}>
                <legend className="visually-hidden">
                  Suggestion {index + 1}
                </legend>
                <label>
                  Type
                  <select
                    aria-label={`Suggestion ${index + 1} type`}
                    onChange={(event) =>
                      updateSuggestion(index, {
                        kind: event.target.value as SuggestionKind,
                      })
                    }
                    value={suggestion.kind}
                  >
                    {SUGGESTION_KINDS.map((kind) => (
                      <option key={kind.value} value={kind.value}>
                        {kind.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  Detail
                  <textarea
                    aria-label={`Suggestion ${index + 1} detail`}
                    onChange={(event) =>
                      updateSuggestion(index, { content: event.target.value })
                    }
                    required
                    rows={3}
                    value={suggestion.content}
                  />
                </label>
                <label>
                  Due date{" "}
                  <input
                    aria-label={`Suggestion ${index + 1} due date`}
                    onChange={(event) =>
                      updateSuggestion(index, {
                        dueOn: event.target.value || undefined,
                      })
                    }
                    type="date"
                    value={suggestion.dueOn ?? ""}
                  />
                </label>
                <button
                  className="button button-small button-ghost"
                  onClick={() => removeSuggestion(index)}
                  type="button"
                >
                  Remove
                </button>
              </fieldset>
            ))}
          </div>
          {actionError ? (
            <p className="form-error" role="alert">
              {actionError}
            </p>
          ) : null}
          <div className="review-actions">
            <button
              className="button button-ghost"
              disabled={isSaving || isGenerating}
              type="submit"
            >
              {isSaving ? "Saving…" : "Save suggestions"}
            </button>
            <button
              className="button button-primary"
              disabled={dirty || draft.length === 0 || isSaving || isGenerating}
              onClick={() => void handleGenerate()}
              type="button"
            >
              {isGenerating ? "Generating plan…" : "Generate plan"}
            </button>
          </div>
          {dirty ? (
            <p className="review-notice">
              Save your edits before generating a Plan.
            </p>
          ) : null}
        </form>
      </div>
    </section>
  );
}

function OriginalCapture({
  item,
  user,
}: {
  item: InboxItemDetail;
  user: ReturnType<typeof useAuth>["user"];
}) {
  const [pdf, setPdf] = useState<{
    status: "loading" | "ready" | "error";
    url?: string;
  }>({ status: "loading" });
  const isPdf = item.sourceType === "pdf";

  useEffect(() => {
    if (!isPdf || !user) return undefined;
    let active = true;
    let objectUrl: string | undefined;
    void fetchPrivatePdf(user, item.id)
      .then((blob) => {
        objectUrl = URL.createObjectURL(blob);
        if (active) setPdf({ status: "ready", url: objectUrl });
      })
      .catch(() => {
        if (active) setPdf({ status: "error" });
      });
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [isPdf, item.id, user]);

  return (
    <article className="review-original">
      <p className="workspace-empty-kicker">Original private capture</p>
      {item.sourceType === "text" ? (
        <p className="review-original-text">{item.originalText}</p>
      ) : (
        <>
          {item.originalFilename ? (
            <p className="review-file-name">{item.originalFilename}</p>
          ) : null}
          {pdf.status === "loading" ? <p>Loading private PDF…</p> : null}
          {pdf.status === "error" ? (
            <p className="form-error" role="alert">
              We could not load this private PDF.
            </p>
          ) : null}
          {pdf.status === "ready" && pdf.url ? (
            <iframe
              className="review-pdf"
              src={pdf.url}
              title="Original private PDF"
            />
          ) : null}
        </>
      )}
    </article>
  );
}
