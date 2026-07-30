import type { Metadata } from "next";

import { PrivateWorkspace } from "@/components/auth/private-workspace";

export const metadata: Metadata = {
  title: "Today — Life Inbox",
};

export default function TodayPage() {
  return <PrivateWorkspace />;
}
