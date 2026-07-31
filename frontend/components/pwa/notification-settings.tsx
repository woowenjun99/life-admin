"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { env } from "@/env";
import {
  type IdTokenSource,
  removeFcmRegistrationToken,
  saveFcmRegistrationToken,
} from "@/lib/api";
import {
  isFcmSupported,
  listenForForegroundFcmMessages,
  registerFcmInstallation,
  unregisterFcmInstallation,
} from "@/lib/firebase/messaging";

type NotificationState =
  | { status: "checking" }
  | { status: "available"; message: string }
  | { status: "enabled"; message: string }
  | { status: "blocked"; message: string }
  | { status: "unavailable"; message: string }
  | { status: "error"; message: string };

export function NotificationSettings({ user }: { user: IdTokenSource }) {
  const [state, setState] = useState<NotificationState>({
    status: "checking",
  });
  const foregroundUnsubscribe = useRef<(() => void) | null>(null);

  const startForegroundListener = useCallback(async () => {
    foregroundUnsubscribe.current?.();
    foregroundUnsubscribe.current = await listenForForegroundFcmMessages();
  }, []);

  const refresh = useCallback(async () => {
    if (!env.NEXT_PUBLIC_FIREBASE_VAPID_KEY) {
      setState({
        status: "unavailable",
        message: "Notifications have not been configured yet.",
      });
      return;
    }
    if (!(await isFcmSupported())) {
      setState({
        status: "unavailable",
        message: "Notifications need a supported browser over HTTPS.",
      });
      return;
    }
    if (Notification.permission === "denied") {
      setState({
        status: "blocked",
        message: "Notifications are blocked in this browser.",
      });
      return;
    }
    if (Notification.permission !== "granted") {
      setState({
        status: "available",
        message:
          "Get an alert when Plans, suggestions, or due steps are ready.",
      });
      return;
    }

    try {
      const token = await registerFcmInstallation();
      if (!token) {
        throw new Error("Firebase did not provide an installation ID.");
      }
      await saveFcmRegistrationToken(user, token);
      await startForegroundListener();
      setState({ status: "enabled", message: "Notifications are on." });
    } catch {
      setState({
        status: "error",
        message: "We could not set up notifications. Please try again.",
      });
    }
  }, [startForegroundListener, user]);

  useEffect(() => {
    void refresh();
    return () => foregroundUnsubscribe.current?.();
  }, [refresh]);

  const enable = useCallback(async () => {
    if (Notification.permission === "denied") {
      setState({
        status: "blocked",
        message: "Notifications are blocked in this browser.",
      });
      return;
    }
    const permission = await Notification.requestPermission();
    if (permission !== "granted") {
      setState({
        status: "available",
        message: "Allow notifications to receive private Life Inbox alerts.",
      });
      return;
    }
    await refresh();
  }, [refresh]);

  const disable = useCallback(async () => {
    try {
      const installationId = await unregisterFcmInstallation();
      await removeFcmRegistrationToken(user, installationId);
      foregroundUnsubscribe.current?.();
      foregroundUnsubscribe.current = null;
      setState({ status: "available", message: "Notifications are off." });
    } catch {
      setState({
        status: "error",
        message: "We could not turn off notifications. Please try again.",
      });
    }
  }, [user]);

  if (state.status === "checking") return null;

  const isEnabled = state.status === "enabled";
  const canEnable = state.status === "available" || state.status === "error";
  const showStatus =
    state.status === "blocked" ||
    state.status === "unavailable" ||
    state.status === "error";
  return (
    <div className="notification-settings">
      {isEnabled ? (
        <button
          aria-describedby={
            showStatus ? "notification-settings-status" : undefined
          }
          className="button button-small button-ghost"
          onClick={() => void disable()}
          type="button"
        >
          Alerts on
        </button>
      ) : canEnable ? (
        <button
          aria-describedby={
            showStatus ? "notification-settings-status" : undefined
          }
          className="button button-small button-ghost"
          onClick={() => void enable()}
          type="button"
        >
          Turn on alerts
        </button>
      ) : null}
      {showStatus ? (
        <span
          aria-live="polite"
          className="notification-settings-status"
          id="notification-settings-status"
        >
          {state.message}
        </span>
      ) : null}
    </div>
  );
}
