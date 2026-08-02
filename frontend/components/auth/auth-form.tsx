"use client";

import {
  createUserWithEmailAndPassword,
  signInWithEmailAndPassword,
  signOut,
} from "firebase/auth";
import Link from "next/link";
import { useRouter } from "next/navigation";
import type { FormEvent } from "react";
import { useCallback, useEffect, useRef, useState } from "react";

import { type AuthMode, authenticationErrorMessage } from "@/lib/auth";
import { createSession } from "@/lib/api";
import { firebaseAuth } from "@/lib/firebase/client";

import { useAuth } from "./auth-provider";

type AuthFormProps = {
  mode: AuthMode;
  onModeChange?: (mode: AuthMode) => void;
};

export function AuthForm({ mode, onModeChange }: AuthFormProps) {
  const router = useRouter();
  const { isLoading, user } = useAuth();
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const sessionEstablished = useRef(false);
  const sessionRequest = useRef<Promise<void> | null>(null);
  const isSignUp = mode === "sign-up";

  const establishSession = useCallback(async (firebaseUser: typeof user) => {
    if (!firebaseUser || sessionEstablished.current) return;

    if (!sessionRequest.current) {
      sessionRequest.current = createSession(firebaseUser)
        .then(() => {
          sessionEstablished.current = true;
        })
        .finally(() => {
          sessionRequest.current = null;
        });
    }

    await sessionRequest.current;
  }, []);

  useEffect(() => {
    if (isLoading || !user) {
      return;
    }

    let cancelled = false;
    void establishSession(user)
      .then(() => {
        if (!cancelled) {
          router.replace("/today");
        }
      })
      .catch(async () => {
        await signOut(firebaseAuth).catch(() => undefined);
        if (!cancelled) {
          setErrorMessage(
            "We could not secure your private session. Please try again.",
          );
        }
      });

    return () => {
      cancelled = true;
    };
  }, [establishSession, isLoading, router, user]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const formData = new FormData(event.currentTarget);
    const email = String(formData.get("email") ?? "").trim();
    const password = String(formData.get("password") ?? "");

    setErrorMessage(null);
    setIsSubmitting(true);

    let firebaseUser: NonNullable<typeof user>;
    try {
      const credential = isSignUp
        ? await createUserWithEmailAndPassword(firebaseAuth, email, password)
        : await signInWithEmailAndPassword(firebaseAuth, email, password);
      firebaseUser = credential.user;
    } catch (error) {
      setErrorMessage(authenticationErrorMessage(error, mode));
      setIsSubmitting(false);
      return;
    }

    try {
      await establishSession(firebaseUser);
      router.replace("/today");
    } catch {
      await signOut(firebaseAuth).catch(() => undefined);
      setErrorMessage(
        "We could not secure your private session. Please try again.",
      );
    } finally {
      setIsSubmitting(false);
    }
  }

  if (isLoading || user) {
    return (
      <div aria-busy="true" className="auth-form-loading">
        <p>Opening Life Inbox…</p>
      </div>
    );
  }

  return (
    <div className="auth-card">
      <Link aria-label="Life Inbox home" className="brand" href="/">
        <span aria-hidden="true" className="brand-mark">
          L
        </span>
        <span>Life Inbox</span>
      </Link>

      <div className="auth-copy">
        <p className="eyebrow">Your private workspace</p>
        <h1 id="auth-dialog-heading">
          {isSignUp ? "Make room for what matters." : "Welcome back."}
        </h1>
        <p>
          {isSignUp
            ? "Create an account to keep your captures and plans private."
            : "Sign in to return to your Life Inbox."}
        </p>
      </div>

      <form className="auth-form" onSubmit={handleSubmit}>
        <label htmlFor="email">Email address</label>
        <input
          autoComplete="email"
          id="email"
          name="email"
          required
          type="email"
        />

        <label htmlFor="password">Password</label>
        <input
          autoComplete={isSignUp ? "new-password" : "current-password"}
          id="password"
          minLength={6}
          name="password"
          required
          type="password"
        />

        {errorMessage ? (
          <p className="form-error" role="alert">
            {errorMessage}
          </p>
        ) : null}

        <button
          className="button button-primary"
          disabled={isSubmitting}
          type="submit"
        >
          {isSubmitting
            ? "Please wait…"
            : isSignUp
              ? "Create your workspace"
              : "Sign in"}
          <span aria-hidden="true">↗</span>
        </button>
      </form>

      <p className="auth-switch">
        {isSignUp ? "Already have an account?" : "New to Life Inbox?"}{" "}
        {onModeChange ? (
          <button
            className="auth-switch-button"
            onClick={() => onModeChange(isSignUp ? "sign-in" : "sign-up")}
            type="button"
          >
            {isSignUp ? "Sign in" : "Create one"}
          </button>
        ) : (
          <Link href={isSignUp ? "/sign-in" : "/sign-up"}>
            {isSignUp ? "Sign in" : "Create one"}
          </Link>
        )}
      </p>
    </div>
  );
}
