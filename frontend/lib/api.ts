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
  updatedAt: string;
};

export type PlanStepUpdate = {
  expectedRevision: number;
  status: PlanStep["status"];
  waitingOn: string | null;
};

export type PlanDraftStep = {
  id?: string;
  title: string;
  rationale: string;
  status: PlanStep["status"];
  dueOn?: string;
  waitingOn?: string;
};

export type PlanUpdate = {
  expectedRevision: number;
  summary: string;
  steps: PlanDraftStep[];
};

export type PlanMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  proposal?: { summary: string; steps: PlanDraftStep[] };
  baseRevision?: number;
  appliedRevision?: number;
  createdAt: string;
};

export type PlanConversation = {
  messages: PlanMessage[];
  hasMore: boolean;
};

export type Plan = {
  id: string;
  inboxItemId: string;
  summary: string;
  status: "ready" | "waiting" | "complete";
  revision: number;
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

export async function createSession(user: IdTokenSource): Promise<void> {
  const response = await fetch("/api/v1/auth/session", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ idToken: await user.getIdToken() }),
  });
  if (!response.ok) {
    throw await responseError(
      response,
      "We could not secure your private session.",
    );
  }
}

export async function clearSession(): Promise<void> {
  const response = await fetch("/api/v1/auth/session", {
    method: "DELETE",
  });
  if (!response.ok) {
    throw await responseError(response, "We could not sign you out.");
  }
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

export async function fetchPlans(
  user: IdTokenSource,
  options: { archived?: boolean } = {},
): Promise<Plan[]> {
  const search = options.archived ? "?archived=true" : "";
  const response = await fetch(`/api/v1/plans${search}`, {
    cache: "no-store",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not load your Plans.");
  }
  return parsePlans(await response.json());
}

export async function archivePlan(
  user: IdTokenSource,
  planId: string,
): Promise<void> {
  return changePlanArchiveState(user, planId, "archive");
}

export async function restorePlan(
  user: IdTokenSource,
  planId: string,
): Promise<void> {
  return changePlanArchiveState(user, planId, "restore");
}

async function changePlanArchiveState(
  user: IdTokenSource,
  planId: string,
  action: "archive" | "restore",
): Promise<void> {
  const response = await fetch(`/api/v1/plans/${planId}/${action}`, {
    method: "POST",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(
      response,
      `We could not ${action} this Plan. Please try again.`,
    );
  }
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

export async function updatePlan(
  user: IdTokenSource,
  planId: string,
  update: PlanUpdate,
): Promise<Plan> {
  const body = {
    ...update,
    steps: update.steps.map((step) => ({
      ...step,
      dueOn: step.dueOn ?? null,
      waitingOn: step.waitingOn ?? null,
    })),
  };
  const response = await fetch(`/api/v1/plans/${planId}`, {
    method: "PUT",
    headers: await authorizationHeaders(user, {
      "Content-Type": "application/json",
    }),
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not update this Plan.");
  }
  return parsePlan(await response.json());
}

export async function fetchPlanConversation(
  user: IdTokenSource,
  planId: string,
  before?: string,
): Promise<PlanConversation> {
  const search = before ? `?before=${encodeURIComponent(before)}` : "";
  const response = await fetch(`/api/v1/plans/${planId}/conversation${search}`, {
    cache: "no-store",
    headers: await authorizationHeaders(user),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not load the Plan conversation.");
  }
  return parsePlanConversation(await response.json());
}

export async function streamPlanMessage(
  user: IdTokenSource,
  planId: string,
  content: string,
  onAssistantDelta: (content: string) => void,
  onAssistantReset: () => void = () => undefined,
): Promise<{ userMessage: PlanMessage; assistantMessage: PlanMessage }> {
  const response = await fetch(`/api/v1/plans/${planId}/conversation`, {
    method: "POST",
    headers: await authorizationHeaders(user, {
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({ content }),
  });
  if (!response.ok) {
    throw await responseError(response, "We could not discuss this Plan.");
  }
  if (!response.headers.get("content-type")?.includes("text/event-stream") || !response.body) {
    throw invalid("The Plan conversation response was invalid.");
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let complete: { userMessage: PlanMessage; assistantMessage: PlanMessage } | undefined;
  let streamError: ApiError | undefined;

  const handleEvent = (frame: string) => {
    let event = "message";
    const data: string[] = [];
    for (const line of frame.split(/\r?\n/)) {
      if (line.startsWith("event:")) event = line.slice("event:".length).trim();
      if (line.startsWith("data:")) {
        const value = line.slice("data:".length);
        data.push(value.startsWith(" ") ? value.slice(1) : value);
      }
    }
    if (data.length === 0) return;
    let payload: unknown;
    try {
      payload = JSON.parse(data.join("\n"));
    } catch {
      throw invalid("The Plan conversation response was invalid.");
    }
    if (event === "delta") {
      if (!isRecord(payload) || typeof payload.content !== "string") {
        throw invalid("The Plan conversation response was invalid.");
      }
      onAssistantDelta(payload.content);
      return;
    }
    if (event === "reset") {
      onAssistantReset();
      return;
    }
    if (event === "complete") {
      if (!isRecord(payload) || !isRecord(payload.userMessage) || !isRecord(payload.assistantMessage)) {
        throw invalid("The Plan conversation response was invalid.");
      }
      complete = {
        userMessage: parsePlanMessage(payload.userMessage),
        assistantMessage: parsePlanMessage(payload.assistantMessage),
      };
      return;
    }
    if (event === "error") {
      const message = isRecord(payload) && typeof payload.message === "string"
        ? payload.message
        : "We could not discuss this Plan.";
      const code = isRecord(payload) && typeof payload.code === "string"
        ? payload.code
        : undefined;
      streamError = new ApiError(502, message, code);
    }
  };

  while (true) {
    const { done, value } = await reader.read();
    buffer += decoder.decode(value, { stream: !done });
    let boundary = /\r?\n\r?\n/.exec(buffer);
    while (boundary?.index !== undefined) {
      handleEvent(buffer.slice(0, boundary.index));
      buffer = buffer.slice(boundary.index + boundary[0].length);
      boundary = /\r?\n\r?\n/.exec(buffer);
    }
    if (done) break;
  }
  if (buffer.trim()) handleEvent(buffer);
  if (streamError) throw streamError;
  if (complete) return complete;
  throw invalid("The Plan conversation response was incomplete.");
}

export async function applyPlanProposal(
  user: IdTokenSource,
  planId: string,
  messageId: string,
  expectedRevision: number,
): Promise<Plan> {
  const response = await fetch(
    `/api/v1/plans/${planId}/conversation/${messageId}/apply`,
    {
      method: "POST",
      headers: await authorizationHeaders(user, {
        "Content-Type": "application/json",
      }),
      body: JSON.stringify({ expectedRevision }),
    },
  );
  if (!response.ok) {
    throw await responseError(response, "We could not apply this Plan proposal.");
  }
  return parsePlan(await response.json());
}

export async function saveFcmRegistrationToken(
  user: IdTokenSource,
  token: string,
): Promise<void> {
  const response = await fetch("/api/v1/fcm-registration-tokens", {
    method: "PUT",
    headers: await authorizationHeaders(user, {
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({ token }),
  });
  if (!response.ok) {
    throw await responseError(
      response,
      "We could not save notification settings.",
    );
  }
}

export async function removeFcmRegistrationToken(
  user: IdTokenSource,
  token: string,
): Promise<void> {
  const response = await fetch("/api/v1/fcm-registration-tokens", {
    method: "DELETE",
    headers: await authorizationHeaders(user, {
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({ token }),
  });
  if (!response.ok) {
    throw await responseError(
      response,
      "We could not update notification settings.",
    );
  }
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
  return parsePlanValue(payload.plan);
}

export function parsePlans(payload: unknown): Plan[] {
  if (!isRecord(payload) || !Array.isArray(payload.plans)) {
    throw invalid("The Plans response was invalid.");
  }
  return payload.plans.map((plan) => {
    try {
      return parsePlanValue(plan);
    } catch {
      throw invalid("The Plans response was invalid.");
    }
  });
}

function parsePlanValue(plan: unknown): Plan {
  if (
    !isRecord(plan) ||
    !Object.keys(plan).every((key) =>
      [
        "id",
        "inboxItemId",
        "summary",
        "status",
        "revision",
        "steps",
        "createdAt",
        "updatedAt",
      ].includes(key),
    )
  ) {
    throw invalid("The Plan response was invalid.");
  }
  const { id, inboxItemId, summary, status, revision, steps, createdAt, updatedAt } =
    plan;
  if (
    typeof id !== "string" ||
    typeof inboxItemId !== "string" ||
    typeof summary !== "string" ||
    !isPlanStatus(status) ||
    typeof revision !== "number" ||
    !Number.isSafeInteger(revision) ||
    revision < 1 ||
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
    revision,
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
        "updatedAt",
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
    typeof value.isNextAction !== "boolean" ||
    typeof value.updatedAt !== "string"
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
    updatedAt: value.updatedAt,
  };
}

function parsePlanConversation(payload: unknown): PlanConversation {
  if (
    !isRecord(payload) ||
    !Array.isArray(payload.messages) ||
    typeof payload.hasMore !== "boolean" ||
    !Object.keys(payload).every((key) => ["messages", "hasMore"].includes(key))
  ) {
    throw invalid("The Plan conversation response was invalid.");
  }
  return { messages: payload.messages.map(parsePlanMessage), hasMore: payload.hasMore };
}

function parsePlanMessage(value: unknown): PlanMessage {
  if (
    !isRecord(value) ||
    !Object.keys(value).every((key) =>
      [
        "id",
        "role",
        "content",
        "proposal",
        "baseRevision",
        "appliedRevision",
        "createdAt",
      ].includes(key),
    ) ||
    typeof value.id !== "string" ||
    (value.role !== "user" && value.role !== "assistant") ||
    typeof value.content !== "string" ||
    !(value.proposal === null || value.proposal === undefined || isRecord(value.proposal)) ||
    !(typeof value.baseRevision === "number" || value.baseRevision === null || value.baseRevision === undefined) ||
    !(typeof value.appliedRevision === "number" || value.appliedRevision === null || value.appliedRevision === undefined) ||
    typeof value.createdAt !== "string"
  ) {
    throw invalid("The Plan conversation response was invalid.");
  }
  const proposal =
    value.proposal === undefined || value.proposal === null
      ? undefined
      : parsePlanDraft(value.proposal);
  return {
    id: value.id,
    role: value.role,
    content: value.content,
    ...(proposal === undefined ? {} : { proposal }),
    ...(typeof value.baseRevision === "number"
      ? { baseRevision: value.baseRevision }
      : {}),
    ...(typeof value.appliedRevision === "number"
      ? { appliedRevision: value.appliedRevision }
      : {}),
    createdAt: value.createdAt,
  };
}

function parsePlanDraft(value: Record<string, unknown>): {
  summary: string;
  steps: PlanDraftStep[];
} {
  if (
    !Object.keys(value).every((key) => ["summary", "steps"].includes(key)) ||
    typeof value.summary !== "string" ||
    !Array.isArray(value.steps)
  ) {
    throw invalid("The Plan conversation response was invalid.");
  }
  return { summary: value.summary, steps: value.steps.map(parsePlanDraftStep) };
}

function parsePlanDraftStep(value: unknown): PlanDraftStep {
  if (
    !isRecord(value) ||
    !Object.keys(value).every((key) =>
      ["id", "title", "rationale", "status", "dueOn", "waitingOn"].includes(key),
    ) ||
    !(typeof value.id === "string" || value.id === null || value.id === undefined) ||
    typeof value.title !== "string" ||
    typeof value.rationale !== "string" ||
    !isPlanStatus(value.status) ||
    !(typeof value.dueOn === "string" || value.dueOn === null || value.dueOn === undefined) ||
    !(typeof value.waitingOn === "string" || value.waitingOn === null || value.waitingOn === undefined)
  ) {
    throw invalid("The Plan conversation response was invalid.");
  }
  return {
    ...(typeof value.id === "string" ? { id: value.id } : {}),
    title: value.title,
    rationale: value.rationale,
    status: value.status,
    ...(typeof value.dueOn === "string" ? { dueOn: value.dueOn } : {}),
    ...(typeof value.waitingOn === "string" ? { waitingOn: value.waitingOn } : {}),
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
