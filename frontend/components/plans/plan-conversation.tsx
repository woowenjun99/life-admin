"use client";

import { type FormEvent, useCallback, useEffect, useMemo, useState } from "react";

import {
  ApiError,
  applyPlanProposal,
  fetchPlanConversation,
  type IdTokenSource,
  type Plan,
  type PlanDraftStep,
  type PlanMessage,
  sendPlanMessage,
} from "@/lib/api";

type PlanConversationProps = {
  plan: Plan;
  user: IdTokenSource | null;
  onPlanUpdated: (plan: Plan) => void;
  onReloadPlan: () => void;
};

function proposalChanges(plan: Plan, steps: PlanDraftStep[]): string[] {
  const current = new Map(plan.steps.map((step) => [step.id, step]));
  const retained = new Set(steps.flatMap((step) => (step.id ? [step.id] : [])));
  const changes = steps.flatMap((step) => {
    if (!step.id) return [`Add: ${step.title}`];
    const previous = current.get(step.id);
    if (!previous) return [`Update: ${step.title}`];
    return previous.title !== step.title || previous.rationale !== step.rationale || previous.status !== step.status || previous.dueOn !== step.dueOn || previous.waitingOn !== step.waitingOn
      ? [`Update: ${step.title}`]
      : [];
  });
  for (const step of plan.steps) {
    if (!retained.has(step.id)) changes.push(`Remove: ${step.title}`);
  }
  return changes;
}

export function PlanConversation({
  plan,
  user,
  onPlanUpdated,
  onReloadPlan,
}: PlanConversationProps) {
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<PlanMessage[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [content, setContent] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const [applyingMessageId, setApplyingMessageId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (before?: string) => {
    if (!user) return;
    setIsLoading(true);
    setError(null);
    try {
      const page = await fetchPlanConversation(user, plan.id, before);
      setMessages((current) => (before ? [...page.messages, ...current] : page.messages));
      setHasMore(page.hasMore);
    } catch {
      setError("We could not load this Plan discussion. Please try again.");
    } finally {
      setIsLoading(false);
    }
  }, [plan.id, user]);

  useEffect(() => {
    if (open && messages.length === 0) void load();
  }, [open, messages.length, load]);

  const oldestTimestamp = messages[0]?.createdAt;
  const messageChanges = useMemo(
    () => new Map(messages.map((message) => [message.id, message.proposal ? proposalChanges(plan, message.proposal.steps) : []])),
    [messages, plan],
  );

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const question = content.trim();
    if (!user || !question || isSending) return;
    setIsSending(true);
    setError(null);
    void sendPlanMessage(user, plan.id, question)
      .then(({ userMessage, assistantMessage }) => {
        setMessages((current) => [...current, userMessage, assistantMessage]);
        setContent("");
      })
      .catch(() => setError("We could not discuss this Plan. Please try again."))
      .finally(() => setIsSending(false));
  };

  const apply = (message: PlanMessage) => {
    if (!user || !message.proposal || !message.baseRevision || applyingMessageId) return;
    setApplyingMessageId(message.id);
    setError(null);
    void applyPlanProposal(user, plan.id, message.id, plan.revision)
      .then((updated) => {
        onPlanUpdated(updated);
        setMessages((current) =>
          current.map((currentMessage) =>
            currentMessage.id === message.id
              ? { ...currentMessage, appliedRevision: updated.revision }
              : currentMessage,
          ),
        );
      })
      .catch((cause: unknown) => {
        if (cause instanceof ApiError && cause.code === "PLAN_REVISION_CONFLICT") {
          onReloadPlan();
          setError("This proposal is based on an older Plan. Reloaded the current Plan instead.");
          return;
        }
        setError("We could not apply this Plan proposal. Please try again.");
      })
      .finally(() => setApplyingMessageId(null));
  };

  return (
    <section className="plan-conversation">
      <button
        aria-expanded={open}
        className="button button-ghost plan-discuss-button"
        onClick={() => setOpen((current) => !current)}
        type="button"
      >
        {open ? "Hide discussion" : "Discuss Plan"}
      </button>
      {open ? (
        <div className="plan-conversation-panel">
          <div>
            <p className="workspace-empty-kicker">Plan discussion</p>
            <h2>Think it through together.</h2>
            <p>The assistant can suggest a revision, but only you can apply it.</p>
          </div>
          {hasMore && oldestTimestamp ? (
            <button
              className="button button-ghost"
              disabled={isLoading}
              onClick={() => void load(oldestTimestamp)}
              type="button"
            >
              Load older messages
            </button>
          ) : null}
          {isLoading && messages.length === 0 ? <p>Opening discussion…</p> : null}
          <div aria-live="polite" className="plan-messages">
            {messages.map((message) => {
              const stale = Boolean(message.proposal && message.baseRevision !== plan.revision);
              const changes = messageChanges.get(message.id) ?? [];
              return (
                <article className={`plan-message plan-message-${message.role}`} key={message.id}>
                  <p className="workspace-empty-kicker">{message.role === "user" ? "You" : "Life Inbox"}</p>
                  <p>{message.content}</p>
                  {message.proposal ? (
                    <div className="plan-proposal">
                      <p className="plan-proposal-heading">Proposed Plan revision</p>
                      <p>{message.proposal.summary}</p>
                      {changes.length > 0 ? (
                        <ul>{changes.map((change) => <li key={change}>{change}</li>)}</ul>
                      ) : (
                        <p>No content changes were proposed.</p>
                      )}
                      {message.appliedRevision ? <p>Applied in revision {message.appliedRevision}.</p> : null}
                      {stale ? <p>This proposal is based on an older Plan and cannot be applied.</p> : null}
                      {!message.appliedRevision && !stale ? (
                        <button
                          className="button button-primary"
                          disabled={applyingMessageId !== null}
                          onClick={() => apply(message)}
                          type="button"
                        >
                          {applyingMessageId === message.id ? "Applying…" : "Apply proposal"}
                        </button>
                      ) : null}
                    </div>
                  ) : null}
                </article>
              );
            })}
          </div>
          {error ? <p className="workspace-error plan-action-error" role="alert">{error}</p> : null}
          <form className="plan-conversation-form" onSubmit={submit}>
            <label htmlFor="plan-conversation-message">Ask about this Plan</label>
            <textarea
              disabled={isSending}
              id="plan-conversation-message"
              maxLength={2000}
              onChange={(event) => setContent(event.target.value)}
              placeholder="What should change, or what do you need to decide?"
              required
              value={content}
            />
            <button className="button button-primary" disabled={isSending || !content.trim()} type="submit">
              {isSending ? "Thinking…" : "Send"}
            </button>
          </form>
        </div>
      ) : null}
    </section>
  );
}
