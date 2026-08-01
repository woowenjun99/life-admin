import { expect, test } from "bun:test";

import {
  archivePlan,
  applyPlanProposal,
  authorizationHeaders,
  createTextCapture,
  fetchInboxItem,
  fetchInboxItems,
  fetchPlan,
  fetchPlanConversation,
  fetchPlans,
  fetchPrivatePdf,
  generatePlan,
  parseCurrentUser,
  parseInboxItemDetail,
  parseInboxItems,
  parsePlan,
  parsePlans,
  removeFcmRegistrationToken,
  restorePlan,
  retryExtraction,
  saveFcmRegistrationToken,
  saveSuggestions,
  streamPlanMessage,
  updatePlan,
  updatePlanStep,
  uploadFileCapture,
  validateCaptureFile,
} from "./api";

test("authorizationHeaders preserves existing headers and attaches a bearer token", async () => {
  const headers = await authorizationHeaders(
    {
      getIdToken: async () => "firebase-id-token",
    },
    { "X-Request-Id": "request-123" },
  );

  expect(headers.get("Authorization")).toBe("Bearer firebase-id-token");
  expect(headers.get("X-Request-Id")).toBe("request-123");
});

test("parseCurrentUser rejects an invalid identity response", () => {
  expect(() => parseCurrentUser({ user: { uid: "user-123" } })).toThrow(
    "The workspace identity response was invalid.",
  );
});

test("createTextCapture sends a bearer JSON request and parses the safe item response", async () => {
  const originalFetch = globalThis.fetch;
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return Response.json({
      inboxItem: {
        id: "item-123",
        sourceType: "text",
        status: "captured",
        canRetryExtraction: true,
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-30T00:00:00Z",
      },
      extraction: "retryable",
    });
  }) as typeof fetch;

  try {
    const item = await createTextCapture(
      { getIdToken: async () => "firebase-id-token" },
      "Renew passport",
    );

    expect(item.item.sourceType).toBe("text");
    expect(item.extraction).toBe("retryable");
    expect(request?.input).toBe("/api/v1/inbox-items");
    const headers = new Headers(request?.init?.headers);
    expect(headers.get("Authorization")).toBe("Bearer firebase-id-token");
    expect(headers.get("Content-Type")).toBe("application/json");
    expect(request?.init?.body).toBe('{"text":"Renew passport"}');
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("fetchInboxItems sends a bearer request and parses metadata-only results", async () => {
  const originalFetch = globalThis.fetch;
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return Response.json({
      inboxItems: [
        {
          id: "item-789",
          planId: "plan-789",
          sourceType: "pdf",
          status: "captured",
          canRetryExtraction: false,
          createdAt: "2026-07-30T00:00:00Z",
          updatedAt: "2026-07-30T00:00:00Z",
        },
      ],
    });
  }) as typeof fetch;

  try {
    const items = await fetchInboxItems({
      getIdToken: async () => "firebase-id-token",
    });

    expect(items).toHaveLength(1);
    expect(items[0]?.sourceType).toBe("pdf");
    expect(items[0]?.planId).toBe("plan-789");
    expect(items[0]?.canRetryExtraction).toBe(false);
    expect(request?.input).toBe("/api/v1/inbox-items");
    expect(request?.init?.cache).toBe("no-store");
    expect(new Headers(request?.init?.headers).get("Authorization")).toBe(
      "Bearer firebase-id-token",
    );
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("Inbox parsers reject private fields in a list and require valid detail payloads", () => {
  expect(() =>
    parseInboxItems({
      inboxItems: [
        {
          id: "item-123",
          sourceType: "text",
          status: "captured",
          canRetryExtraction: true,
          createdAt: "2026-07-30T00:00:00Z",
          updatedAt: "2026-07-30T00:00:00Z",
          originalText: "This field is not list metadata",
        },
      ],
    }),
  ).toThrow("The Inbox response was invalid.");

  expect(
    parseInboxItemDetail({
      inboxItem: {
        id: "item-123",
        sourceType: "text",
        status: "captured",
        canRetryExtraction: true,
        originalText: "Renew passport",
        originalFilename: null,
        contentType: null,
        byteSize: null,
        suggestions: [],
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-30T00:00:00Z",
      },
    }),
  ).toMatchObject({ originalText: "Renew passport" });

  expect(() =>
    parseInboxItemDetail({
      inboxItem: {
        id: "item-456",
        sourceType: "pdf",
        status: "captured",
        canRetryExtraction: false,
        originalText: null,
        originalFilename: "letter.pdf",
        contentType: "application/pdf",
        byteSize: 0,
        suggestions: [],
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-30T00:00:00Z",
      },
    }),
  ).toThrow("The Inbox item response was invalid.");
});

test("fetchInboxItem requests one private item and keeps not-found errors", async () => {
  const originalFetch = globalThis.fetch;
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return Response.json(
      {
        error: { code: "NOT_FOUND", message: "Inbox item not found." },
      },
      { status: 404 },
    );
  }) as typeof fetch;

  try {
    await expect(
      fetchInboxItem(
        { getIdToken: async () => "firebase-id-token" },
        "item-123",
      ),
    ).rejects.toMatchObject({ status: 404, code: "NOT_FOUND" });
    expect(request?.input).toBe("/api/v1/inbox-items/item-123");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("uploadFileCapture sends multipart without a browser-selected content type header", async () => {
  const originalFetch = globalThis.fetch;
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return Response.json({
      inboxItem: {
        id: "item-456",
        sourceType: "pdf",
        status: "captured",
        canRetryExtraction: true,
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-30T00:00:00Z",
      },
      extraction: "ready",
    });
  }) as typeof fetch;

  try {
    await uploadFileCapture(
      { getIdToken: async () => "firebase-id-token" },
      new File(["%PDF-1.7"], "letter.pdf", { type: "application/pdf" }),
    );

    expect(request?.input).toBe("/api/v1/inbox-items/files");
    const headers = new Headers(request?.init?.headers);
    expect(headers.get("Authorization")).toBe("Bearer firebase-id-token");
    expect(headers.get("Content-Type")).toBeNull();
    expect(request?.init?.body).toBeInstanceOf(FormData);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("upload errors keep the server error code for capture state messaging", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () =>
    Response.json(
      {
        error: {
          code: "STORAGE_UNAVAILABLE",
          message: "Private file storage is temporarily unavailable.",
        },
      },
      { status: 503 },
    )) as unknown as typeof fetch;

  try {
    await expect(
      uploadFileCapture(
        { getIdToken: async () => "firebase-id-token" },
        new File(["%PDF-1.7"], "letter.pdf", { type: "application/pdf" }),
      ),
    ).rejects.toMatchObject({
      status: 503,
      code: "STORAGE_UNAVAILABLE",
    });
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("validateCaptureFile gives immediate type and size feedback", () => {
  expect(validateCaptureFile({ type: "text/plain", size: 3 })).toBe(
    "Choose a PDF, JPEG, or PNG file.",
  );
  expect(
    validateCaptureFile({
      type: "application/pdf",
      size: 10 * 1024 * 1024 + 1,
    }),
  ).toBe("Files must not exceed 10 MiB.");
  expect(validateCaptureFile({ type: "image/png", size: 10 })).toBeNull();
});

test("review and Plan API calls require a bearer token and use only safe response fields", async () => {
  const originalFetch = globalThis.fetch;
  const requests: Array<{ input: RequestInfo | URL; init?: RequestInit }> = [];
  globalThis.fetch = (async (input, init) => {
    requests.push({ input, init });
    if (String(input).includes("/plans")) {
      return Response.json({
        plan: {
          id: "plan-123",
          inboxItemId: "item-123",
          summary: "Renew before the trip.",
          status: "ready",
          revision: 1,
          steps: [
            {
              id: "step-123",
              position: 0,
              title: "Check requirements",
              rationale: "Confirm the deadline.",
              status: "ready",
              dueOn: null,
              waitingOn: null,
              isNextAction: true,
              updatedAt: "2026-07-30T00:00:00Z",
            },
          ],
          createdAt: "2026-07-30T00:00:00Z",
          updatedAt: "2026-07-30T00:00:00Z",
        },
      });
    }
    if (String(input).includes("/extract")) {
      return Response.json({
        inboxItem: {
          id: "item-123",
          sourceType: "text",
          status: "reviewing",
          canRetryExtraction: false,
          originalText: "Renew passport",
          originalFilename: null,
          contentType: null,
          byteSize: null,
          suggestions: [],
          createdAt: "2026-07-30T00:00:00Z",
          updatedAt: "2026-07-30T00:00:00Z",
        },
      });
    }
    return Response.json({
      inboxItem: {
        id: "item-123",
        sourceType: "text",
        status: "reviewing",
        canRetryExtraction: false,
        originalText: "Renew passport",
        originalFilename: null,
        contentType: null,
        byteSize: null,
        suggestions: [],
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-30T00:00:00Z",
      },
    });
  }) as typeof fetch;

  try {
    const user = { getIdToken: async () => "firebase-id-token" };
    await retryExtraction(user, "item-123");
    await saveSuggestions(user, "item-123", []);
    const plan = await generatePlan(user, "item-123");
    await fetchPlan(user, plan.id);

    expect(plan.steps[0]?.isNextAction).toBe(true);
    expect(requests.map(({ input }) => input)).toEqual([
      "/api/v1/inbox-items/item-123/extract",
      "/api/v1/inbox-items/item-123",
      "/api/v1/inbox-items/item-123/plans",
      "/api/v1/plans/plan-123",
    ]);
    expect(new Headers(requests[1]?.init?.headers).get("Content-Type")).toBe(
      "application/json",
    );
    for (const request of requests) {
      expect(new Headers(request.init?.headers).get("Authorization")).toBe(
        "Bearer firebase-id-token",
      );
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("updatePlanStep sends the complete status change and preserves API errors", async () => {
  const originalFetch = globalThis.fetch;
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return Response.json({
      plan: {
        id: "plan-123",
        inboxItemId: "item-123",
        summary: "Renew before the trip.",
        status: "waiting",
        revision: 1,
        steps: [
          {
            id: "step-123",
            position: 0,
            title: "Check requirements",
            rationale: "Confirm the deadline.",
            status: "waiting",
            dueOn: null,
            waitingOn: "A reply from the agency",
            isNextAction: false,
            updatedAt: "2026-07-31T00:00:00Z",
          },
        ],
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-31T00:00:00Z",
      },
    });
  }) as typeof fetch;

  try {
    const plan = await updatePlanStep(
      { getIdToken: async () => "firebase-id-token" },
      "plan-123",
      "step-123",
      { expectedRevision: 1, status: "waiting", waitingOn: "A reply from the agency" },
    );

    expect(plan.status).toBe("waiting");
    expect(plan.steps[0]?.waitingOn).toBe("A reply from the agency");
    expect(request?.input).toBe("/api/v1/plans/plan-123/steps/step-123");
    expect(request?.init?.method).toBe("PATCH");
    expect(new Headers(request?.init?.headers).get("Authorization")).toBe(
      "Bearer firebase-id-token",
    );
    expect(new Headers(request?.init?.headers).get("Content-Type")).toBe(
      "application/json",
    );
    expect(request?.init?.body).toBe(
      '{"expectedRevision":1,"status":"waiting","waitingOn":"A reply from the agency"}',
    );
  } finally {
    globalThis.fetch = originalFetch;
  }

  globalThis.fetch = (async () =>
    Response.json(
      {
        error: {
          code: "INVALID_STATE",
          message: "This Plan step can no longer be changed.",
        },
      },
      { status: 409 },
    )) as unknown as typeof fetch;
  try {
    await expect(
      updatePlanStep(
        { getIdToken: async () => "firebase-id-token" },
        "plan-123",
        "step-123",
        { expectedRevision: 1, status: "complete", waitingOn: null },
      ),
    ).rejects.toMatchObject({ status: 409, code: "INVALID_STATE" });
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("Plan edits and discussions use revision-bound owner-authenticated contracts", async () => {
  const originalFetch = globalThis.fetch;
  const requests: Array<{ input: RequestInfo | URL; init?: RequestInit }> = [];
  const plan = {
    id: "plan-123",
    inboxItemId: "item-123",
    summary: "Renew before the trip.",
    status: "ready",
    revision: 2,
    steps: [
      {
        id: "step-123",
        position: 0,
        title: "Check requirements",
        rationale: "Confirm the deadline.",
        status: "ready",
        dueOn: null,
        waitingOn: null,
        isNextAction: true,
        updatedAt: "2026-08-01T00:00:00Z",
      },
    ],
    createdAt: "2026-07-30T00:00:00Z",
    updatedAt: "2026-08-01T00:00:00Z",
  };
  globalThis.fetch = (async (input, init) => {
    requests.push({ input, init });
    const path = String(input);
    if (path.endsWith("/apply") || init?.method === "PUT") {
      return Response.json({ plan });
    }
    if (init?.method === "POST") {
      const events = [
        "event: delta\ndata: {\"content\":\"Here is a \"}\n\n",
        "event: delta\ndata: {\"content\":\"revision to review.\"}\n\n",
        "event: complete\ndata: {\"userMessage\":{\"id\":\"message-user\",\"role\":\"user\",\"content\":\"Could you revise this?\",\"proposal\":null,\"baseRevision\":null,\"appliedRevision\":null,\"createdAt\":\"2026-08-01T00:00:00Z\"},\"assistantMessage\":{\"id\":\"message-assistant\",\"role\":\"assistant\",\"content\":\"Here is a revision to review.\",\"proposal\":{\"summary\":\"Renew before travel.\",\"steps\":[{\"id\":\"step-123\",\"title\":\"Confirm requirements\",\"rationale\":\"Clarifies the deadline.\",\"status\":\"ready\",\"dueOn\":null,\"waitingOn\":null}]},\"baseRevision\":1,\"appliedRevision\":null,\"createdAt\":\"2026-08-01T00:00:01Z\"}}\n\n",
      ].join("");
      const split = Math.floor(events.length / 2);
      const encoder = new TextEncoder();
      return new Response(new ReadableStream({
        start(controller) {
          controller.enqueue(encoder.encode(events.slice(0, split)));
          controller.enqueue(encoder.encode(events.slice(split)));
          controller.close();
        },
      }),
        { headers: { "content-type": "text/event-stream" } },
      );
    }
    return Response.json({
      messages: [],
      hasMore: false,
    });
  }) as typeof fetch;

  try {
    const user = { getIdToken: async () => "firebase-id-token" };
    await updatePlan(user, "plan-123", {
      expectedRevision: 1,
      summary: plan.summary,
      steps: [
        {
          id: "step-123",
          title: "Check requirements",
          rationale: "Confirm the deadline.",
          status: "ready",
        },
      ],
    });
    const conversation = await fetchPlanConversation(user, "plan-123");
    expect(conversation.hasMore).toBe(false);
    const deltas: string[] = [];
    const reply = await streamPlanMessage(user, "plan-123", "Could you revise this?", (delta) => {
      deltas.push(delta);
    });
    expect(reply.assistantMessage.proposal?.steps[0]?.id).toBe("step-123");
    expect(deltas.join("")).toBe("Here is a revision to review.");
    await applyPlanProposal(user, "plan-123", "message-assistant", 1);

    expect(requests.map(({ input }) => input)).toEqual([
      "/api/v1/plans/plan-123",
      "/api/v1/plans/plan-123/conversation",
      "/api/v1/plans/plan-123/conversation",
      "/api/v1/plans/plan-123/conversation/message-assistant/apply",
    ]);
    expect(requests.map(({ init }) => init?.method)).toEqual(["PUT", undefined, "POST", "POST"]);
    expect(JSON.parse(String(requests[0]?.init?.body))).toEqual({
      expectedRevision: 1,
      summary: "Renew before the trip.",
      steps: [
        {
          id: "step-123",
          title: "Check requirements",
          rationale: "Confirm the deadline.",
          status: "ready",
          dueOn: null,
          waitingOn: null,
        },
      ],
    });
    expect(requests[3]?.init?.body).toBe('{"expectedRevision":1}');
    for (const request of requests) {
      expect(new Headers(request.init?.headers).get("Authorization")).toBe(
        "Bearer firebase-id-token",
      );
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("streamPlanMessage reports an SSE error after partial assistant text", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response(
    "event: delta\ndata: {\"content\":\"Partial reply\"}\n\nevent: error\ndata: {\"code\":\"AI_UNAVAILABLE\",\"message\":\"Try again later.\"}\n\n",
    { headers: { "content-type": "text/event-stream" } },
  )) as unknown as typeof fetch;

  try {
    const deltas: string[] = [];
    await expect(streamPlanMessage(
      { getIdToken: async () => "firebase-id-token" },
      "plan-123",
      "Could you revise this?",
      (delta) => deltas.push(delta),
    )).rejects.toMatchObject({ code: "AI_UNAVAILABLE", message: "Try again later." });
    expect(deltas).toEqual(["Partial reply"]);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("Firebase messaging token calls are owner-authenticated and do not expose a token in a URL", async () => {
  const originalFetch = globalThis.fetch;
  const requests: Array<{ input: RequestInfo | URL; init?: RequestInit }> = [];
  globalThis.fetch = (async (input, init) => {
    requests.push({ input, init });
    return new Response(null, { status: 204 });
  }) as typeof fetch;

  try {
    const user = { getIdToken: async () => "firebase-id-token" };
    await saveFcmRegistrationToken(user, "fcm-token-123");
    await removeFcmRegistrationToken(user, "fcm-token-123");

    expect(requests.map(({ input }) => input)).toEqual([
      "/api/v1/fcm-registration-tokens",
      "/api/v1/fcm-registration-tokens",
    ]);
    expect(requests.map(({ init }) => init?.method)).toEqual(["PUT", "DELETE"]);
    for (const request of requests) {
      expect(new Headers(request.init?.headers).get("Authorization")).toBe(
        "Bearer firebase-id-token",
      );
      expect(request.init?.body).toBe('{"token":"fcm-token-123"}');
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("fetchPlans uses a bearer token and rejects malformed step timestamps", async () => {
  const originalFetch = globalThis.fetch;
  let request: { input: RequestInfo | URL; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input, init) => {
    request = { input, init };
    return Response.json({
      plans: [
        {
          id: "plan-123",
          inboxItemId: "item-123",
          summary: "Renew before the trip.",
          status: "ready",
          revision: 1,
          steps: [
            {
              id: "step-123",
              position: 0,
              title: "Check requirements",
              rationale: "Confirm the deadline.",
              status: "ready",
              dueOn: null,
              waitingOn: null,
              isNextAction: true,
              updatedAt: "2026-07-31T00:00:00Z",
            },
          ],
          createdAt: "2026-07-30T00:00:00Z",
          updatedAt: "2026-07-31T00:00:00Z",
        },
      ],
    });
  }) as typeof fetch;

  try {
    const user = { getIdToken: async () => "firebase-id-token" };
    const plans = await fetchPlans(user);

    expect(plans[0]?.steps[0]?.updatedAt).toBe("2026-07-31T00:00:00Z");
    expect(request?.input).toBe("/api/v1/plans");
    expect(request?.init?.cache).toBe("no-store");
    expect(new Headers(request?.init?.headers).get("Authorization")).toBe(
      "Bearer firebase-id-token",
    );

    await fetchPlans(user, { archived: true });
    expect(request?.input).toBe("/api/v1/plans?archived=true");
  } finally {
    globalThis.fetch = originalFetch;
  }

  expect(() =>
    parsePlans({
      plans: [
        {
          id: "plan-123",
          inboxItemId: "item-123",
          summary: "Renew before the trip.",
          status: "ready",
          steps: [
            {
              id: "step-123",
              position: 0,
              title: "Check requirements",
              rationale: "Confirm the deadline.",
              status: "ready",
              dueOn: null,
              waitingOn: null,
              isNextAction: true,
              updatedAt: 123,
            },
          ],
          createdAt: "2026-07-30T00:00:00Z",
          updatedAt: "2026-07-31T00:00:00Z",
        },
      ],
    }),
  ).toThrow("The Plans response was invalid.");
});

test("archive and restore Plan calls use bearer POST requests without parsing a body", async () => {
  const originalFetch = globalThis.fetch;
  const requests: Array<{ input: RequestInfo | URL; init?: RequestInit }> = [];
  globalThis.fetch = (async (input, init) => {
    requests.push({ input, init });
    return new Response(null, { status: 204 });
  }) as typeof fetch;

  try {
    const user = { getIdToken: async () => "firebase-id-token" };
    await archivePlan(user, "plan-123");
    await restorePlan(user, "plan-123");

    expect(requests.map(({ input }) => input)).toEqual([
      "/api/v1/plans/plan-123/archive",
      "/api/v1/plans/plan-123/restore",
    ]);
    for (const request of requests) {
      expect(request.init?.method).toBe("POST");
      expect(new Headers(request.init?.headers).get("Authorization")).toBe(
        "Bearer firebase-id-token",
      );
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("PDF preview rejects a non-PDF response and plan parsing rejects provider identifiers", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () =>
    new Response("not a PDF", {
      headers: { "Content-Type": "text/plain" },
    })) as unknown as typeof fetch;

  try {
    await expect(
      fetchPrivatePdf(
        { getIdToken: async () => "firebase-id-token" },
        "item-123",
      ),
    ).rejects.toMatchObject({ status: 502 });
  } finally {
    globalThis.fetch = originalFetch;
  }

  expect(() =>
    parsePlan({
      plan: {
        id: "plan-123",
        inboxItemId: "item-123",
        summary: "Renew before the trip.",
        status: "ready",
        steps: [],
        createdAt: "2026-07-30T00:00:00Z",
        updatedAt: "2026-07-30T00:00:00Z",
        providerFileId: "file-secret",
      },
    }),
  ).toThrow("The Plan response was invalid.");
});
