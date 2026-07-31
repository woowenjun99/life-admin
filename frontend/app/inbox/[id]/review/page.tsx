import type { Metadata } from "next";

import { PrivateWorkspace } from "@/components/auth/private-workspace";
import { ReviewContent } from "@/components/inbox/review-content";

export const metadata: Metadata = { title: "Review capture — Life Inbox" };

export default async function ReviewPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  return (
    <PrivateWorkspace>
      <ReviewContent itemId={id} />
    </PrivateWorkspace>
  );
}
