"use client";

import {
  getMessaging,
  isSupported,
  type MessagePayload,
  onMessage,
  onRegistered,
  onUnregistered,
  register,
  unregister,
} from "firebase/messaging";

import { env } from "@/env";

import { firebaseApp } from "./client";

type FcmMessage = {
  body: string;
  tag: string;
  title: string;
  url: string;
};

export async function isFcmSupported(): Promise<boolean> {
  if (typeof window === "undefined" || !window.isSecureContext) return false;
  return isSupported();
}

export async function registerFcmServiceWorker(): Promise<ServiceWorkerRegistration> {
  if (!("serviceWorker" in navigator)) {
    throw new Error("Service workers are not available in this browser.");
  }
  return navigator.serviceWorker.register("/sw.js", { scope: "/" });
}

export async function registerFcmInstallation(): Promise<string> {
  const vapidKey = env.NEXT_PUBLIC_FIREBASE_VAPID_KEY;
  if (!vapidKey) {
    throw new Error("Firebase Cloud Messaging is not configured.");
  }
  const registration = await registerFcmServiceWorker();
  const messaging = getMessaging(firebaseApp);
  return new Promise((resolve, reject) => {
    const stopListening = onRegistered(messaging, (installationId) => {
      stopListening();
      resolve(installationId);
    });

    void register(messaging, {
      serviceWorkerRegistration: registration,
      vapidKey,
    }).catch((error: unknown) => {
      stopListening();
      reject(error);
    });
  });
}

export async function unregisterFcmInstallation(): Promise<string> {
  const messaging = getMessaging(firebaseApp);
  return new Promise((resolve, reject) => {
    const stopListening = onUnregistered(messaging, (installationId) => {
      stopListening();
      resolve(installationId);
    });

    void unregister(messaging).catch((error: unknown) => {
      stopListening();
      reject(error);
    });
  });
}

export async function listenForForegroundFcmMessages(): Promise<() => void> {
  const registration = await registerFcmServiceWorker();
  const messaging = getMessaging(firebaseApp);
  return onMessage(messaging, (payload) => {
    const message = fcmMessage(payload);
    void registration.showNotification(message.title, {
      badge: "/icon.svg",
      body: message.body,
      data: { url: message.url },
      icon: "/icon.svg",
      tag: message.tag,
    });
  });
}

function fcmMessage(payload: MessagePayload): FcmMessage {
  const data = payload.data ?? {};
  return {
    title: data.title ?? "Life Inbox",
    body: data.body ?? "You have a private update.",
    tag: data.tag ?? "life-inbox",
    url: localPath(data.url),
  };
}

function localPath(value: string | undefined): string {
  try {
    const url = new URL(value ?? "/today", window.location.origin);
    return url.origin === window.location.origin
      ? `${url.pathname}${url.search}${url.hash}`
      : "/today";
  } catch {
    return "/today";
  }
}
