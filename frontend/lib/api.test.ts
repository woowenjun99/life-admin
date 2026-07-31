import { expect, test } from "bun:test";

import {
  authorizationHeaders,
  createTextCapture,
  fetchInboxItem,
  fetchInboxItems,
  fetchPlan,
  fetchPrivatePdf,
  generatePlan,
  parseCurrentUser,
  parseInboxItemDetail,
  parseInboxItems,
  parsePlan,
  retryExtraction,
  saveSuggestions,
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
