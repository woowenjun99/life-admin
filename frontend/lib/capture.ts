import { ApiError } from "@/lib/api";

export function captureErrorMessage(error: unknown): string {
  if (!(error instanceof ApiError)) {
    return "Something went wrong. Please try again.";
  }

  switch (error.code) {
    case "VALIDATION_ERROR":
      return "Check the capture and try again.";
    case "FILE_TOO_LARGE":
      return "Files must not exceed 10 MiB.";
    case "STORAGE_UNAVAILABLE":
      return "Private file storage is temporarily unavailable. Please try again later.";
    default:
      return error.message;
  }
}
