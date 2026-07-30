"use client";

import { onAuthStateChanged, type User } from "firebase/auth";
import type { ReactNode } from "react";
import { createContext, useContext, useEffect, useState } from "react";

import { firebaseAuth } from "@/lib/firebase/client";

type AuthContextValue = {
  isLoading: boolean;
  user: User | null;
};

const AuthContext = createContext<AuthContextValue | undefined>(undefined);

export function AuthProvider({ children }: Readonly<{ children: ReactNode }>) {
  const [value, setValue] = useState<AuthContextValue>({
    isLoading: true,
    user: null,
  });

  useEffect(() => {
    return onAuthStateChanged(firebaseAuth, (user) => {
      setValue({
        isLoading: false,
        user,
      });
    });
  }, []);

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const value = useContext(AuthContext);

  if (!value) {
    throw new Error("useAuth must be used within AuthProvider.");
  }

  return value;
}
