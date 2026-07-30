import type { Metadata } from "next";

import { PrivateWorkspace } from "@/components/auth/private-workspace";
import { TodayContent } from "@/components/today/today-content";

export const metadata: Metadata = {
  title: "Today — Life Inbox",
};

export default function TodayPage() {
  return (
    <PrivateWorkspace>
      <TodayContent />
    </PrivateWorkspace>
  );
}
