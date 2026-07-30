export type CaptureMode = "text" | "file";

export function captureModeFromSearchParam(
  value: string | null,
): CaptureMode | null {
  return value === "text" || value === "file" ? value : null;
}
