export type IdTokenSource = {
  getIdToken(forceRefresh?: boolean): Promise<string>;
};

export type CurrentUser = { uid: string; email: string };

export type InboxItem = {
  id: string;
  planId?: string;
  sourceType: "text" | "image" | "pdf";
  status: "captured" | "reviewing" | "planned" | "archived";
  canRetryExtraction: boolean;
  createdAt: string;
  updatedAt: string;
};

export type SuggestionKind =
  | "task"
  | "date"
  | "person"
  | "context"
  | "question";

export type Suggestion = {
  id: string;
  kind: SuggestionKind;
  content: string;
  dueOn?: string;
  position: number;
};

export type EditableSuggestion = Omit<Suggestion, "id" | "position">;

export type InboxItemDetail = InboxItem & {
  originalText?: string;
  originalFilename?: string;
  contentType?: string;
  byteSize?: number;
  suggestions: Suggestion[];
};

export type CaptureResult = {
  item: InboxItem;
  extraction: "ready" | "retryable" | "not_supported";
};

export type PlanStep = {
  id: string;
  position: number;
  title: string;
  rationale: string;
  status: "ready" | "waiting" | "complete";
  dueOn?: string;
  waitingOn?: string;
  isNextAction: boolean;
};

export type PlanStepUpdate = {
  status: PlanStep["status"];
  waitingOn: string | null;
};

export type Plan = {
  id: string;
  inboxItemId: string;
  summary: string;
  status: "ready" | "waiting" | "complete";
  steps: PlanStep[];
  createdAt: string;
  updatedAt: string;
};

export const MAX_CAPTURE_FILE_BYTES = 10 * 1024 * 1024;

const ALLOWED_CAPTURE_FILE_TYPES = new Set([
  "application/pdf",
  "image/jpeg",
  "image/png",
]);

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
    public readonly code?: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export async function authorizationHeaders(
  user: IdTokenSource,
  headers?: HeadersInit,
): Promise<Headers> {
  const authorizationHeaders = new Headers(headers);
  authorizationHeaders.set(
    "Authorization",
    `Bearer ${await user.getIdToken()}`,
  );
  return authorizationHeaders;
}

export async function fetchCurrentUser(
  user: IdTokenSource,
): Promise<CurrentUser> {
  const response = await fetch("/api/v1/me", {
    cache: "no-store",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(
      response,
      "We could not open your private workspace.",
    );
  }
  return parseCurrentUser(await response.json());
}

export async function createTextCapture(
  user: IdTokenSource,
  text: string,
): Promise<CaptureResult> {
  return captureResponse(
    user,
    "/api/v1/inbox-items",
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text }),
    },
    "We could not save that note.",
  );
}

export async function uploadFileCapture(
  user: IdTokenSource,
  file: File,
): Promise<CaptureResult> {
  const formData = new FormData();
  formData.append("file", file);
  return captureResponse(
    user,
    "/api/v1/inbox-items/files",
    { method: "POST", body: formData },
    "We could not save that file.",
  );
}

async function captureResponse(
  user: IdTokenSource,
  path: string,
  init: RequestInit,
  fallback: string,
): Promise<CaptureResult> {
  const response = await fetch(path, {
    ...init,
    headers: await authorizationHeaders(user, init.headers),
  });
  if (!response.ok) {
    throw await responseError(response, fallback);
  }
  return parseCaptureResult(await response.json());
}

export async function fetchInboxItems(
  user: IdTokenSource,
): Promise<InboxItem[]> {
  const response = await fetch("/api/v1/inbox-items", {
    cache: "no-store",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not load your Inbox.");
  }
  return parseInboxItems(await response.json());
}

export async function fetchInboxItem(
  user: IdTokenSource,
  itemId: string,
): Promise<InboxItemDetail> {
  const response = await fetch(`/api/v1/inbox-items/${itemId}`, {
    cache: "no-store",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not load that Inbox item.");
  }
  return parseInboxItemDetail(await response.json());
}

export async function retryExtraction(
  user: IdTokenSource,
  itemId: string,
): Promise<InboxItemDetail> {
  const response = await fetch(`/api/v1/inbox-items/${itemId}/extract`, {
    method: "POST",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not sort that capture yet.");
  }
  return parseInboxItemDetail(await response.json());
}

export async function saveSuggestions(
  user: IdTokenSource,
  itemId: string,
  suggestions: EditableSuggestion[],
): Promise<InboxItemDetail> {
  const response = await fetch(`/api/v1/inbox-items/${itemId}`, {
    method: "PATCH",
    headers: await authorizationHeaders(user, {
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({ suggestions }),
  });
  if (!response.ok) {
    throw await responseError(
      response,
      "We could not save your reviewed suggestions.",
    );
  }
  return parseInboxItemDetail(await response.json());
}

export async function fetchPrivatePdf(
  user: IdTokenSource,
  itemId: string,
): Promise<Blob> {
  const response = await fetch(`/api/v1/inbox-items/${itemId}/file`, {
    cache: "no-store",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not load that private PDF.");
  }
  if (response.headers.get("Content-Type") !== "application/pdf") {
    throw new ApiError(502, "The private PDF response was invalid.");
  }
  return response.blob();
}

export async function generatePlan(
  user: IdTokenSource,
  itemId: string,
): Promise<Plan> {
  const response = await fetch(`/api/v1/inbox-items/${itemId}/plans`, {
    method: "POST",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not generate a plan yet.");
  }
  return parsePlan(await response.json());
}

export async function fetchPlan(
  user: IdTokenSource,
  planId: string,
): Promise<Plan> {
  const response = await fetch(`/api/v1/plans/${planId}`, {
    cache: "no-store",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not load that plan.");
  }
  return parsePlan(await response.json());
}

export async function updatePlanStep(
  user: IdTokenSource,
  planId: string,
  stepId: string,
  update: PlanStepUpdate,
): Promise<Plan> {
  const response = await fetch(`/api/v1/plans/${planId}/steps/${stepId}`, {
    method: "PATCH",
    headers: await authorizationHeaders(user, {
      "Content-Type": "application/json",
    }),
    body: JSON.stringify(update),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not update that Plan step.");
  }
  return parsePlan(await response.json());
}

export function validateCaptureFile(
  file: Pick<File, "size" | "type">,
): string | null {
  if (!ALLOWED_CAPTURE_FILE_TYPES.has(file.type)) {
    return "Choose a PDF, JPEG, or PNG file.";
  }
  if (file.size > MAX_CAPTURE_FILE_BYTES) {
    return "Files must not exceed 10 MiB.";
  }
  return null;
}

export function parseCurrentUser(payload: unknown): CurrentUser {
  if (!isRecord(payload) || !isRecord(payload.user)) {
    throw invalid("The workspace identity response was invalid.");
  }
  const { uid, email } = payload.user;
  if (typeof uid !== "string" || typeof email !== "string") {
    throw invalid("The workspace identity response was invalid.");
  }
  return { uid, email };
}

export function parseCaptureResult(payload: unknown): CaptureResult {
  if (!isRecord(payload) || !isExtractionState(payload.extraction)) {
    throw invalid("The capture response was invalid.");
  }
  return {
    item: parseInboxItem(payload, "The capture response was invalid."),
    extraction: payload.extraction,
  };
}

export function parseInboxItem(
  payload: unknown,
  message = "The capture response was invalid.",
): InboxItem {
  if (!isRecord(payload) || !isRecord(payload.inboxItem)) {
    throw invalid(message);
  }
  return parseInboxItemValue(payload.inboxItem, message);
}

export function parseInboxItems(payload: unknown): InboxItem[] {
  if (!isRecord(payload) || !Array.isArray(payload.inboxItems)) {
    throw invalid("The Inbox response was invalid.");
  }
  return payload.inboxItems.map((item) =>
    parseInboxItemValue(item, "The Inbox response was invalid."),
  );
}

export function parseInboxItemDetail(payload: unknown): InboxItemDetail {
  if (!isRecord(payload) || !isRecord(payload.inboxItem)) {
    throw invalid("The Inbox item response was invalid.");
  }
  const value = payload.inboxItem;
  const inboxItem = parseInboxItemValue(
    value,
    "The Inbox item response was invalid.",
    true,
  );
  const { originalText, originalFilename, contentType, byteSize, suggestions } =
    value;
  if (!Array.isArray(suggestions)) {
    throw invalid("The Inbox item response was invalid.");
  }
  const parsedSuggestions = suggestions.map(parseSuggestion);
  const fileFieldsAreNullable = [originalFilename, contentType, byteSize].every(
    (field) => field === null || field === undefined,
  );
  if (inboxItem.sourceType === "text") {
    if (typeof originalText !== "string" || !fileFieldsAreNullable) {
      throw invalid("The Inbox item response was invalid.");
    }
    return { ...inboxItem, originalText, suggestions: parsedSuggestions };
  }
  if (
    originalText !== null ||
    typeof originalFilename !== "string" ||
    typeof contentType !== "string" ||
    typeof byteSize !== "number" ||
    !Number.isSafeInteger(byteSize) ||
    byteSize <= 0
  ) {
    throw invalid("The Inbox item response was invalid.");
  }
  return {
    ...inboxItem,
    originalFilename,
    contentType,
    byteSize,
    suggestions: parsedSuggestions,
  };
}

export function parsePlan(payload: unknown): Plan {
  if (!isRecord(payload) || !isRecord(payload.plan)) {
    throw invalid("The Plan response was invalid.");
  }
  const plan = payload.plan;
  if (
    !Object.keys(plan).every((key) =>
      [
        "id",
        "inboxItemId",
        "summary",
        "status",
        "steps",
        "createdAt",
        "updatedAt",
      ].includes(key),
    )
  ) {
    throw invalid("The Plan response was invalid.");
  }
  const { id, inboxItemId, summary, status, steps, createdAt, updatedAt } =
    plan;
  if (
    typeof id !== "string" ||
    typeof inboxItemId !== "string" ||
    typeof summary !== "string" ||
    !isPlanStatus(status) ||
    !Array.isArray(steps) ||
    typeof createdAt !== "string" ||
    typeof updatedAt !== "string"
  ) {
    throw invalid("The Plan response was invalid.");
  }
  return {
    id,
    inboxItemId,
    summary,
    status,
    steps: steps.map(parsePlanStep),
    createdAt,
    updatedAt,
  };
}

function parseInboxItemValue(
  payload: unknown,
  message: string,
  allowDetailFields = false,
): InboxItem {
  if (!isRecord(payload)) throw invalid(message);
  const allowed = allowDetailFields
    ? [
        "id",
        "planId",
        "sourceType",
        "status",
        "canRetryExtraction",
        "originalText",
        "originalFilename",
        "contentType",
        "byteSize",
        "suggestions",
        "createdAt",
        "updatedAt",
      ]
    : [
        "id",
        "planId",
        "sourceType",
        "status",
        "canRetryExtraction",
        "createdAt",
        "updatedAt",
      ];
  if (!Object.keys(payload).every((key) => allowed.includes(key)))
    throw invalid(message);
  const {
    id,
    planId,
    sourceType,
    status,
    canRetryExtraction,
    createdAt,
    updatedAt,
  } = payload;
  if (
    typeof id !== "string" ||
    !(typeof planId === "string" || planId === undefined) ||
    !isSourceType(sourceType) ||
    !isInboxStatus(status) ||
    typeof canRetryExtraction !== "boolean" ||
    typeof createdAt !== "string" ||
    typeof updatedAt !== "string"
  )
    throw invalid(message);
  return {
    id,
    ...(planId === undefined ? {} : { planId }),
    sourceType,
    status,
    canRetryExtraction,
    createdAt,
    updatedAt,
  };
}

function parseSuggestion(value: unknown): Suggestion {
  if (
    !isRecord(value) ||
    !Object.keys(value).every((key) =>
      ["id", "kind", "content", "dueOn", "position"].includes(key),
    ) ||
    typeof value.id !== "string" ||
    !isSuggestionKind(value.kind) ||
    typeof value.content !== "string" ||
    !(typeof value.dueOn === "string" || value.dueOn === null) ||
    typeof value.position !== "number" ||
    !Number.isSafeInteger(value.position)
  ) {
    throw invalid("The Inbox item response was invalid.");
  }
  return {
    id: value.id,
    kind: value.kind,
    content: value.content,
    dueOn: value.dueOn ?? undefined,
    position: value.position,
  };
}

function parsePlanStep(value: unknown): PlanStep {
  if (
    !isRecord(value) ||
    !Object.keys(value).every((key) =>
      [
        "id",
        "position",
        "title",
        "rationale",
        "status",
        "dueOn",
        "waitingOn",
        "isNextAction",
      ].includes(key),
    ) ||
    typeof value.id !== "string" ||
    typeof value.position !== "number" ||
    !Number.isSafeInteger(value.position) ||
    typeof value.title !== "string" ||
    typeof value.rationale !== "string" ||
    !isPlanStatus(value.status) ||
    !(typeof value.dueOn === "string" || value.dueOn === null) ||
    !(typeof value.waitingOn === "string" || value.waitingOn === null) ||
    typeof value.isNextAction !== "boolean"
  ) {
    throw invalid("The Plan response was invalid.");
  }
  return {
    id: value.id,
    position: value.position,
    title: value.title,
    rationale: value.rationale,
    status: value.status,
    dueOn: value.dueOn ?? undefined,
    waitingOn: value.waitingOn ?? undefined,
    isNextAction: value.isNextAction,
  };
}

async function responseError(
  response: Response,
  fallback: string,
): Promise<ApiError> {
  const payload: unknown = await response.json().catch(() => undefined);
  const error =
    isRecord(payload) && isRecord(payload.error) ? payload.error : undefined;
  return new ApiError(
    response.status,
    error && typeof error.message === "string" ? error.message : fallback,
    error && typeof error.code === "string" ? error.code : undefined,
  );
}

function invalid(message: string): ApiError {
  return new ApiError(502, message);
}
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
function isSourceType(value: unknown): value is InboxItem["sourceType"] {
  return value === "text" || value === "image" || value === "pdf";
}
function isInboxStatus(value: unknown): value is InboxItem["status"] {
  return (
    value === "captured" ||
    value === "reviewing" ||
    value === "planned" ||
    value === "archived"
  );
}
function isExtractionState(
  value: unknown,
): value is CaptureResult["extraction"] {
  return (
    value === "ready" || value === "retryable" || value === "not_supported"
  );
}
function isSuggestionKind(value: unknown): value is SuggestionKind {
  return (
    value === "task" ||
    value === "date" ||
    value === "person" ||
    value === "context" ||
    value === "question"
  );
}
function isPlanStatus(value: unknown): value is Plan["status"] {
  return value === "ready" || value === "waiting" || value === "complete";
}
