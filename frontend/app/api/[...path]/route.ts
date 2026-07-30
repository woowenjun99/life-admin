import type { NextRequest } from "next/server";

import { env } from "@/env";

const HOP_BY_HOP_RESPONSE_HEADERS = [
  "connection",
  "keep-alive",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
];

type RouteContext = {
  params: Promise<{ path: string[] }>;
};

async function proxy(request: NextRequest, context: RouteContext) {
  const { path } = await context.params;

  if (path.some((segment) => segment === "." || segment === "..")) {
    return Response.json({ error: "invalid_api_path" }, { status: 400 });
  }

  const url = backendUrl(path, request.nextUrl.search);
  const headers = new Headers(request.headers);
  headers.delete("host");
  headers.delete("connection");
  const body =
    request.method === "GET" || request.method === "HEAD"
      ? undefined
      : await request.arrayBuffer();

  try {
    const response = await fetch(url, {
      method: request.method,
      headers,
      body,
      cache: "no-store",
      redirect: "manual",
    });
    const responseHeaders = new Headers(response.headers);

    for (const header of HOP_BY_HOP_RESPONSE_HEADERS) {
      responseHeaders.delete(header);
    }

    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: responseHeaders,
    });
  } catch (error) {
    console.error("Internal backend proxy request failed", {
      error,
      method: request.method,
    });
    return Response.json({ error: "backend_unavailable" }, { status: 502 });
  }
}

function backendUrl(path: string[], search: string): URL {
  const url = new URL(env.BACKEND_INTERNAL_URL);
  const basePath = url.pathname.replace(/\/$/, "");
  const encodedPath = path.map(encodeURIComponent).join("/");

  url.pathname = `${basePath}/api/${encodedPath}`;
  url.search = search;

  return url;
}

export const GET = proxy;
export const HEAD = proxy;
export const POST = proxy;
export const PUT = proxy;
export const PATCH = proxy;
export const DELETE = proxy;
export const OPTIONS = proxy;
