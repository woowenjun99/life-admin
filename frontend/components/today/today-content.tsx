"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import { InboxContent } from "@/components/inbox/inbox-content";
import type { InboxListState } from "@/components/inbox/inbox-list";
import { fetchInboxItems, fetchPlans, restorePlan } from "@/lib/api";

import { isBrandNewWorkspace } from "./first-workspace";
import {
  ArchivedPlans,
  type ArchivedPlansState,
  TodayDashboard,
  type TodayPlansState,
} from "./today-dashboard";

export function TodayContent() {
  const { user } = useAuth();
  const [plans, setPlans] = useState<TodayPlansState>({ status: "loading" });
  const [archivedPlans, setArchivedPlans] = useState<ArchivedPlansState>({
    status: "loading",
  });
  const [inbox, setInbox] = useState<InboxListState>({ status: "loading" });
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [restoringPlanId, setRestoringPlanId] = useState<string | null>(null);
  const currentInboxRequest = useRef(0);

  const loadPlans = useCallback(async () => {
    if (!user) return;

    setPlans({ status: "loading" });
    try {
      setPlans({ status: "ready", plans: await fetchPlans(user) });
    } catch {
      setPlans({ status: "error" });
    }
  }, [user]);

  const loadArchivedPlans = useCallback(async () => {
    if (!user) return;

    setArchivedPlans({ status: "loading" });
    try {
      setArchivedPlans({
        status: "ready",
        plans: await fetchPlans(user, { archived: true }),
      });
    } catch {
      setArchivedPlans({ status: "error" });
    }
  }, [user]);

  const loadInboxItems = useCallback(async () => {
    if (!user) return;

    const request = ++currentInboxRequest.current;
    setInbox({ status: "loading" });
    try {
      const items = await fetchInboxItems(user);
      if (request === currentInboxRequest.current) {
        setInbox({ status: "ready", items });
      }
    } catch {
      if (request === currentInboxRequest.current) {
        setInbox({ status: "error" });
      }
    }
  }, [user]);

  useEffect(() => {
    void loadPlans();
    void loadArchivedPlans();
    void loadInboxItems();
  }, [loadArchivedPlans, loadInboxItems, loadPlans]);

  const restoreArchivedPlan = useCallback(
    async (planId: string) => {
      if (!user) return;

      setRestoringPlanId(planId);
      setRestoreError(null);
      try {
        await restorePlan(user, planId);
        await Promise.all([loadPlans(), loadArchivedPlans(), loadInboxItems()]);
      } catch {
        setRestoreError("We could not restore this Plan. Please try again.");
      } finally {
        setRestoringPlanId(null);
      }
    },
    [loadArchivedPlans, loadInboxItems, loadPlans, user],
  );

  const brandNewWorkspace = isBrandNewWorkspace({
    plans,
    archivedPlans,
    inbox,
  });

  return (
    <section className="workspace-panel inbox-panel">
      <p className="eyebrow">Your private workspace</p>
      <h1 className="workspace-heading">One clear next action.</h1>
      {!brandNewWorkspace ? (
        <TodayDashboard onRetry={() => void loadPlans()} state={plans} />
      ) : null}
      {!brandNewWorkspace ? (
        <ArchivedPlans
          onRestore={(planId) => void restoreArchivedPlan(planId)}
          onRetry={() => void loadArchivedPlans()}
          restoreError={restoreError}
          restoringPlanId={restoringPlanId}
          state={archivedPlans}
        />
      ) : null}
      <InboxContent
        firstTask={brandNewWorkspace}
        inboxState={inbox}
        onCaptured={() => void loadInboxItems()}
        onRetry={() => void loadInboxItems()}
        plans={plans.status === "ready" ? plans.plans : undefined}
      />
    </section>
  );
}
