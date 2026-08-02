type AsyncAction = () => Promise<void>;

export async function clearSessionBeforeFirebaseSignOut(
  clearSessionCookie: AsyncAction,
  signOutOfFirebase: AsyncAction,
): Promise<void> {
  await clearSessionCookie();
  await signOutOfFirebase();
}
