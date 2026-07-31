import { env } from "@/env";

export const dynamic = "force-dynamic";

const firebaseConfig = {
  apiKey: env.NEXT_PUBLIC_FIREBASE_API_KEY,
  authDomain: env.NEXT_PUBLIC_FIREBASE_AUTH_DOMAIN,
  projectId: env.NEXT_PUBLIC_FIREBASE_PROJECT_ID,
  storageBucket: env.NEXT_PUBLIC_FIREBASE_STORAGE_BUCKET,
  messagingSenderId: env.NEXT_PUBLIC_FIREBASE_MESSAGING_SENDER_ID,
  appId: env.NEXT_PUBLIC_FIREBASE_APP_ID,
};

export function GET() {
  const source = [
    'importScripts("https://www.gstatic.com/firebasejs/12.16.0/firebase-app-compat.js");',
    'importScripts("https://www.gstatic.com/firebasejs/12.16.0/firebase-messaging-compat.js");',
    `firebase.initializeApp(${JSON.stringify(firebaseConfig)});`,
    "const messaging = firebase.messaging();",
    "function localPath(value) {",
    "  try {",
    "    const url = new URL(value || '/today', self.location.origin);",
    "    return url.origin === self.location.origin ? url.pathname + url.search + url.hash : '/today';",
    "  } catch { return '/today'; }",
    "}",
    "messaging.onBackgroundMessage((payload) => {",
    "  const data = payload.data || {};",
    "  return self.registration.showNotification(data.title || 'Life Inbox', {",
    "    body: data.body || 'You have a private update.',",
    "    icon: '/icon.svg',",
    "    badge: '/icon.svg',",
    "    tag: data.tag || 'life-inbox',",
    "    data: { url: localPath(data.url) },",
    "  });",
    "});",
    "self.addEventListener('notificationclick', (event) => {",
    "  event.notification.close();",
    "  const url = localPath(event.notification.data && event.notification.data.url);",
    "  event.waitUntil((async () => {",
    "    const clients = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });",
    "    const existing = clients.find((client) => new URL(client.url).origin === self.location.origin);",
    "    if (existing) { await existing.navigate(url); return existing.focus(); }",
    "    return self.clients.openWindow(url);",
    "  })());",
    "});",
  ].join("\n");

  return new Response(source, {
    headers: {
      "Cache-Control": "no-cache, no-store, must-revalidate",
      "Content-Type": "application/javascript; charset=utf-8",
      "Service-Worker-Allowed": "/",
    },
  });
}
