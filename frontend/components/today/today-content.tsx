"use client";

import { useCallback, useEffect, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import { InboxContent } from "@/components/inbox/inbox-content";
import { fetchPlans, restorePlan } from "@/lib/api";

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
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [restoringPlanId, setRestoringPlanId] = useState<string | null>(null);

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

  useEffect(() => {
    void loadPlans();
    void loadArchivedPlans();
  }, [loadArchivedPlans, loadPlans]);

  const restoreArchivedPlan = useCallback(
    async (planId: string) => {
      if (!user) return;

      setRestoringPlanId(planId);
      setRestoreError(null);
      try {
        await restorePlan(user, planId);
        await Promise.all([loadPlans(), loadArchivedPlans()]);
      } catch {
        setRestoreError("We could not restore this Plan. Please try again.");
      } finally {
        setRestoringPlanId(null);
      }
    },
    [loadArchivedPlans, loadPlans, user],
  );

  return (
    <section className="workspace-panel inbox-panel">
      <p className="eyebrow">Your private workspace</p>
      <h1 className="workspace-heading">One clear next action.</h1>
      <TodayDashboard onRetry={() => void loadPlans()} state={plans} />
      <ArchivedPlans
        onRestore={(planId) => void restoreArchivedPlan(planId)}
        onRetry={() => void loadArchivedPlans()}
        restoreError={restoreError}
        restoringPlanId={restoringPlanId}
        state={archivedPlans}
      />
      <InboxContent
        plans={plans.status === "ready" ? plans.plans : undefined}
      />
    </section>
  );
}
