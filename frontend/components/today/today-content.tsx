"use client";

import { useCallback, useEffect, useState } from "react";

import { useAuth } from "@/components/auth/auth-provider";
import { InboxContent } from "@/components/inbox/inbox-content";
import { fetchPlans } from "@/lib/api";

import { TodayDashboard, type TodayPlansState } from "./today-dashboard";

export function TodayContent() {
  const { user } = useAuth();
  const [plans, setPlans] = useState<TodayPlansState>({ status: "loading" });

  const loadPlans = useCallback(async () => {
    if (!user) return;

    setPlans({ status: "loading" });
    try {
      setPlans({ status: "ready", plans: await fetchPlans(user) });
    } catch {
      setPlans({ status: "error" });
    }
  }, [user]);

  useEffect(() => {
    void loadPlans();
  }, [loadPlans]);

  return (
    <section className="workspace-panel inbox-panel">
      <p className="eyebrow">Your private workspace</p>
      <h1 className="workspace-heading">One clear next action.</h1>
      <TodayDashboard onRetry={() => void loadPlans()} state={plans} />
      <InboxContent
        plans={plans.status === "ready" ? plans.plans : undefined}
      />
    </section>
  );
}
