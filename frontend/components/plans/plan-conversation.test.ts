import { expect, test } from "bun:test";

import type { PlanMessage } from "@/lib/api";

import {
  mergeInitialPlanConversationMessages,
  settlePlanConversationMessage,
} from "./plan-conversation";

function message(id: string, role: PlanMessage["role"]): PlanMessage {
  return {
    id,
    role,
    content: role === "user" ? "Could you revise this?" : "Here is a revision.",
    createdAt: "2026-08-01T00:00:00Z",
  };
}

test("initial conversation loading retains pending messages and completion de-duplicates persisted ones", () => {
  const pending = message("pending-123", "user");
  const userMessage = message("message-user", "user");
  const assistantMessage = message("message-assistant", "assistant");

  const whileStreaming = mergeInitialPlanConversationMessages(
    [pending],
    [userMessage, assistantMessage],
  );
  expect(whileStreaming.map((item) => item.id)).toEqual([
    "message-user",
    "message-assistant",
    "pending-123",
  ]);

  const settled = settlePlanConversationMessage(
    whileStreaming,
    pending.id,
    userMessage,
    assistantMessage,
  );
  expect(settled.map((item) => item.id)).toEqual([
    "message-user",
    "message-assistant",
  ]);
});
