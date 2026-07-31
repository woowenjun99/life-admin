import type { Metadata } from "next";

import { PrivateWorkspace } from "@/components/auth/private-workspace";
import { PlanContent } from "@/components/plans/plan-content";

export const metadata: Metadata = { title: "Plan — Life Inbox" };

export default async function PlanPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  return (
    <PrivateWorkspace>
      <PlanContent planId={id} />
    </PrivateWorkspace>
  );
}
