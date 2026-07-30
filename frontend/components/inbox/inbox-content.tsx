"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import { fetchInboxItems } from "@/lib/api";

import { CaptureForms } from "./capture-forms";
import { InboxList, type InboxListState } from "./inbox-list";

export function InboxContent() {
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

  return (
    <>
      <CaptureForms onCaptured={() => void loadInboxItems()} />
      <InboxList onRetry={loadInboxItems} state={state} />
    </>
  );
}
