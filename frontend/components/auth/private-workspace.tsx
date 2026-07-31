"use client";

import { signOut } from "firebase/auth";
import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useState,
} from "react";

import { ApiError, type CurrentUser, fetchCurrentUser } from "@/lib/api";
import { firebaseAuth } from "@/lib/firebase/client";

import { useAuth } from "./auth-provider";

type WorkspaceState =
  | { status: "loading" }
  | { status: "ready"; currentUser: CurrentUser }
  | { status: "error"; message: string };

const WorkspaceUserContext = createContext<CurrentUser | null>(null);

export function useWorkspaceUser(): CurrentUser {
  const user = useContext(WorkspaceUserContext);
  if (!user) {
    throw new Error("useWorkspaceUser must be used inside PrivateWorkspace.");
  }

  return user;
}

type PrivateWorkspaceProps = {
  children: ReactNode;
};

export function PrivateWorkspace({ children }: PrivateWorkspaceProps) {
  const router = useRouter();
  const { isLoading, user } = useAuth();
  const [workspace, setWorkspace] = useState<WorkspaceState>({
    status: "loading",
  });
  const [isSigningOut, setIsSigningOut] = useState(false);

  useEffect(() => {
    if (isLoading) {
      return undefined;
    }

    if (!user) {
      router.replace("/?auth=sign-in");
      return undefined;
    }

    const currentFirebaseUser = user;
    let cancelled = false;

    async function loadWorkspace() {
      setWorkspace({ status: "loading" });

      try {
        const currentUser = await fetchCurrentUser(currentFirebaseUser);
        if (!cancelled) {
          setWorkspace({ status: "ready", currentUser });
        }
      } catch (error) {
        if (error instanceof ApiError && error.status === 401) {
          await signOut(firebaseAuth).catch(() => undefined);
          if (!cancelled) {
            router.replace("/?auth=sign-in");
          }
          return;
        }

        if (!cancelled) {
          setWorkspace({
            status: "error",
            message:
              "We could not open your private workspace. Please refresh and try again.",
          });
        }
      }
    }

    void loadWorkspace();

    return () => {
      cancelled = true;
    };
  }, [isLoading, router, user]);

  async function handleSignOut() {
    setIsSigningOut(true);

    try {
      await signOut(firebaseAuth);
      router.replace("/");
    } catch {
      setWorkspace({
        status: "error",
        message: "We could not sign you out. Please try again.",
      });
      setIsSigningOut(false);
    }
  }

  if (isLoading || !user || workspace.status === "loading") {
    return (
      <main aria-busy="true" className="workspace-page workspace-loading">
        <p>Opening your private workspace…</p>
      </main>
    );
  }

  if (workspace.status === "error") {
    return (
      <main className="workspace-page workspace-loading">
        <div className="workspace-error" role="alert">
          <p>{workspace.message}</p>
          <Link className="text-link" href="/">
            Back to Life Inbox <span aria-hidden="true">→</span>
          </Link>
        </div>
      </main>
    );
  }

  return (
    <main className="workspace-page">
      <nav aria-label="Workspace navigation" className="workspace-nav">
        <Link aria-label="Life Inbox Today" className="brand" href="/today">
          <span aria-hidden="true" className="brand-mark">
            L
          </span>
          <span>Life Inbox</span>
        </Link>
        <div className="workspace-nav-actions">
          <Link
            className="button button-small workspace-capture-link"
            href="/today?capture=text"
          >
            Capture
          </Link>
          <button
            className="button button-small button-ghost"
            disabled={isSigningOut}
            onClick={handleSignOut}
            type="button"
          >
            {isSigningOut ? "Signing out…" : "Sign out"}
          </button>
        </div>
      </nav>

      <WorkspaceUserContext value={workspace.currentUser}>
        {children}
      </WorkspaceUserContext>
    </main>
  );
}
