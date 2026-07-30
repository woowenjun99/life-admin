export type AuthMode = "sign-in" | "sign-up";

type FirebaseErrorLike = {
  code?: unknown;
};

function firebaseErrorCode(error: unknown): string | undefined {
  if (typeof error !== "object" || error === null || !("code" in error)) {
    return undefined;
  }

  const { code } = error as FirebaseErrorLike;
  return typeof code === "string" ? code : undefined;
}

export function authenticationErrorMessage(
  error: unknown,
  mode: AuthMode,
): string {
  switch (firebaseErrorCode(error)) {
    case "auth/invalid-email":
      return "Enter a valid email address.";
    case "auth/weak-password":
      return "Use a password with at least six characters.";
    case "auth/email-already-in-use":
      return "An account already exists for this email address.";
    case "auth/invalid-credential":
    case "auth/user-not-found":
    case "auth/wrong-password":
      return "Email or password is incorrect.";
    case "auth/too-many-requests":
      return "Too many attempts. Please wait a moment and try again.";
    default:
      return mode === "sign-up"
        ? "We could not create your account. Please try again."
        : "We could not sign you in. Please try again.";
  }
}
