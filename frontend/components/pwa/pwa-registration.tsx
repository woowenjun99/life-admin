"use client";

import { useEffect } from "react";

import { registerFcmServiceWorker } from "@/lib/firebase/messaging";

export function PwaRegistration() {
  useEffect(() => {
    void registerFcmServiceWorker().catch(() => undefined);
  }, []);

  return null;
}
