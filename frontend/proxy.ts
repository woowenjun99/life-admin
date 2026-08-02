import { NextResponse, type NextRequest } from "next/server";

import { env } from "@/env";

const SESSION_COOKIE_NAME = "life_inbox_session";

type SessionState = "authenticated" | "invalid" | "unavailable";

export async function proxy(request: NextRequest): Promise<NextResponse> {
  const sessionCookie = request.cookies.get(SESSION_COOKIE_NAME)?.value;
  const isHome = request.nextUrl.pathname === "/";
  const isPrivateRoute = !isHome;

  if (!sessionCookie) {
    return isPrivateRoute
      ? redirectToSignIn(request, false)
      : NextResponse.next();
  }

  const session = await sessionState(sessionCookie);
  if (session === "authenticated") {
    return isHome
      ? NextResponse.redirect(new URL("/today", request.url))
      : NextResponse.next();
  }

  if (session === "invalid") {
    return isPrivateRoute
      ? redirectToSignIn(request, true)
      : clearInvalidSession(request);
  }

  return isPrivateRoute
    ? redirectToSignIn(request, false)
    : NextResponse.next();
}

async function sessionState(sessionCookie: string): Promise<SessionState> {
  try {
    const response = await fetch(sessionUrl(), {
      headers: { Cookie: `${SESSION_COOKIE_NAME}=${sessionCookie}` },
      cache: "no-store",
      redirect: "manual",
    });
    if (response.status === 204) return "authenticated";
    if (response.status === 401) return "invalid";
    return "unavailable";
  } catch {
    return "unavailable";
  }
}

function sessionUrl(): URL {
  const url = new URL(env.BACKEND_INTERNAL_URL);
  const basePath = url.pathname.replace(/\/$/, "");
  url.pathname = `${basePath}/api/v1/auth/session`;
  url.search = "";
  return url;
}

function redirectToSignIn(
  request: NextRequest,
  clearSession: boolean,
): NextResponse {
  const response = NextResponse.redirect(
    new URL("/?auth=sign-in", request.url),
  );
  if (clearSession) {
    expireSessionCookie(response, request);
  }
  return response;
}

function clearInvalidSession(request: NextRequest): NextResponse {
  const response = NextResponse.next();
  expireSessionCookie(response, request);
  return response;
}

function expireSessionCookie(response: NextResponse, request: NextRequest) {
  response.cookies.set({
    name: SESSION_COOKIE_NAME,
    value: "",
    path: "/",
    maxAge: 0,
    httpOnly: true,
    sameSite: "lax",
    secure: request.nextUrl.protocol === "https:",
  });
}

export const config = {
  matcher: ["/", "/today/:path*", "/plans/:path*", "/inbox/:path*"],
};
