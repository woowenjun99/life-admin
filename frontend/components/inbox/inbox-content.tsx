"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import { type CaptureResult, fetchInboxItems, type Plan } from "@/lib/api";

import { CaptureForms } from "./capture-forms";
import { InboxList, type InboxListState } from "./inbox-list";

export function InboxContent({ plans = [] }: { plans?: Plan[] }) {
  const { user } = useAuth();
  const [state, setState] = useState<InboxListState>({ status: "loading" });
  const currentRequest = useRef(0);

  const loadInboxItems = useCallback(async () => {
    if (!user) {
      return;
    }

    const request = ++currentRequest.current;
    setState({ status: "loading" });
    try {
      const items = await fetchInboxItems(user);
      if (request === currentRequest.current) {
        setState({ status: "ready", items });
      }
    } catch {
      if (request === currentRequest.current) {
        setState({ status: "error" });
      }
    }
  }, [user]);

  useEffect(() => {
    void loadInboxItems();
  }, [loadInboxItems]);

  const planSummaries = new Map(
    plans.map((plan) => [plan.inboxItemId, plan.summary]),
  );
  const listState =
    state.status === "ready"
      ? {
          status: "ready" as const,
          items: state.items.map((item) => {
            const planSummary = planSummaries.get(item.id);
            return planSummary ? { ...item, planSummary } : item;
          }),
        }
      : state;

  return (
    <>
      <CaptureForms
        onCaptured={(_result: CaptureResult) => void loadInboxItems()}
      />
      <InboxList onRetry={loadInboxItems} state={listState} />
    </>
  );
}
