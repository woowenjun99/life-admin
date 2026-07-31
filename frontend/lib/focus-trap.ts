export function focusTrapTargetIndex(
  focusableCount: number,
  activeIndex: number,
  isReverse: boolean,
): number | null {
  if (focusableCount === 0) {
    return null;
  }

  if (activeIndex === -1) {
    return isReverse ? focusableCount - 1 : 0;
  }

  if (isReverse && activeIndex === 0) {
    return focusableCount - 1;
  }

  if (!isReverse && activeIndex === focusableCount - 1) {
    return 0;
  }

  return null;
}

export function restoreFocusAfterDialogClose(
  target: Pick<HTMLElement, "focus"> | null,
  schedule: (callback: () => void) => unknown = requestAnimationFrame,
): void {
  schedule(() => target?.focus());
}
