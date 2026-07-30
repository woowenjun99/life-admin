export type IdTokenSource = {
  getIdToken(forceRefresh?: boolean): Promise<string>;
};

export type CurrentUser = {
  uid: string;
  email: string;
};

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
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
    throw new ApiError(
      response.status,
      "We could not open your private workspace.",
    );
  }

  return parseCurrentUser(await response.json());
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
