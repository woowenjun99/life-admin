export type IdTokenSource = {
  getIdToken(forceRefresh?: boolean): Promise<string>;
};

export type CurrentUser = {
  uid: string;
  email: string;
};

export type InboxItem = {
  id: string;
  sourceType: "text" | "image" | "pdf";
  status: "captured" | "reviewing" | "planned" | "archived";
  createdAt: string;
  updatedAt: string;
};

export type InboxItemDetail = InboxItem & {
  originalText?: string;
  originalFilename?: string;
  contentType?: string;
  byteSize?: number;
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
  const token = await user.getIdToken();
  const authorizationHeaders = new Headers(headers);
  authorizationHeaders.set("Authorization", `Bearer ${token}`);

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
): Promise<InboxItem> {
  const response = await fetch("/api/v1/inbox-items", {
    method: "POST",
    headers: await authorizationHeaders(user, {
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({ text }),
  });

  if (!response.ok) {
    throw await responseError(response, "We could not save that note.");
  }

  return parseInboxItem(await response.json());
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

export async function uploadFileCapture(
  user: IdTokenSource,
  file: File,
): Promise<InboxItem> {
  const formData = new FormData();
  formData.append("file", file);

  const response = await fetch("/api/v1/inbox-items/files", {
    method: "POST",
    headers: await authorizationHeaders(user),
    body: formData,
  });

  if (!response.ok) {
    throw await responseError(response, "We could not save that file.");
  }

  return parseInboxItem(await response.json());
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
    throw new ApiError(502, "The workspace identity response was invalid.");
  }

  const { uid, email } = payload.user;
  if (typeof uid !== "string" || typeof email !== "string") {
    throw new ApiError(502, "The workspace identity response was invalid.");
  }

  return { uid, email };
}

export function parseInboxItem(payload: unknown): InboxItem {
  if (!isRecord(payload) || !isRecord(payload.inboxItem)) {
    throw new ApiError(502, "The capture response was invalid.");
  }

  return parseInboxItemValue(
    payload.inboxItem,
    "The capture response was invalid.",
  );
}

export function parseInboxItems(payload: unknown): InboxItem[] {
  if (!isRecord(payload) || !Array.isArray(payload.inboxItems)) {
    throw new ApiError(502, "The Inbox response was invalid.");
  }

  return payload.inboxItems.map((item) =>
    parseInboxItemValue(item, "The Inbox response was invalid."),
  );
}

export function parseInboxItemDetail(payload: unknown): InboxItemDetail {
  if (!isRecord(payload) || !isRecord(payload.inboxItem)) {
    throw new ApiError(502, "The Inbox item response was invalid.");
  }

  const inboxItem = parseInboxItemValue(
    payload.inboxItem,
    "The Inbox item response was invalid.",
    true,
  );
  const { originalText, originalFilename, contentType, byteSize } =
    payload.inboxItem;
  const fileFieldsAreNullable = [originalFilename, contentType, byteSize].every(
    (value) => value === null || value === undefined,
  );

  if (inboxItem.sourceType === "text") {
    if (typeof originalText !== "string" || !fileFieldsAreNullable) {
      throw new ApiError(502, "The Inbox item response was invalid.");
    }

    return { ...inboxItem, originalText };
  }

  if (
    originalText !== null ||
    typeof originalFilename !== "string" ||
    typeof contentType !== "string" ||
    typeof byteSize !== "number" ||
    !Number.isSafeInteger(byteSize) ||
    byteSize <= 0
  ) {
    throw new ApiError(502, "The Inbox item response was invalid.");
  }

  return {
    ...inboxItem,
    originalFilename,
    contentType,
    byteSize,
  };
}

function parseInboxItemValue(
  payload: unknown,
  errorMessage: string,
  allowDetailFields = false,
): InboxItem {
  if (!isRecord(payload)) {
    throw new ApiError(502, errorMessage);
  }

  const allowedFields = allowDetailFields
    ? [
        "id",
        "sourceType",
        "status",
        "originalText",
        "originalFilename",
        "contentType",
        "byteSize",
        "createdAt",
        "updatedAt",
      ]
    : ["id", "sourceType", "status", "createdAt", "updatedAt"];
  if (!Object.keys(payload).every((field) => allowedFields.includes(field))) {
    throw new ApiError(502, errorMessage);
  }

  const { id, sourceType, status, createdAt, updatedAt } = payload;
  if (
    typeof id !== "string" ||
    (sourceType !== "text" && sourceType !== "image" && sourceType !== "pdf") ||
    (status !== "captured" &&
      status !== "reviewing" &&
      status !== "planned" &&
      status !== "archived") ||
    typeof createdAt !== "string" ||
    typeof updatedAt !== "string"
  ) {
    throw new ApiError(502, errorMessage);
  }

  return { id, sourceType, status, createdAt, updatedAt };
}

async function responseError(
  response: Response,
  fallbackMessage: string,
): Promise<ApiError> {
  const payload: unknown = await response.json().catch(() => undefined);
  const error =
    isRecord(payload) && isRecord(payload.error) ? payload.error : undefined;
  const message =
    error && typeof error.message === "string"
      ? error.message
      : fallbackMessage;
  const code = error && typeof error.code === "string" ? error.code : undefined;

  return new ApiError(response.status, message, code);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
