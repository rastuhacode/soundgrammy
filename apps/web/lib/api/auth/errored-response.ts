"use client";

import { TRPCClientError } from "@trpc/client";
import { performClientLogout } from "./client/logout";

let handlingUnauthorized = false;

export function isUnauthorizedTrpcError(error: unknown): boolean {
  if (!(error instanceof TRPCClientError)) {
    return false;
  }

  return (
    error.data?.code === "UNAUTHORIZED"
    || error.data?.httpStatus === 401
  );
}

export async function handleUnauthorized(): Promise<void> {
  if (handlingUnauthorized || typeof window === "undefined") {
    return;
  }

  handlingUnauthorized = true;

  try {
    await performClientLogout();
  } finally {
    window.location.assign("/login");
  }
}

export function handleErrorResponse(response: Response): void {
  if (response.status === 401) {
    handleUnauthorized();
  }
}

export function handleTrpcError(error: unknown): void {
  if (isUnauthorizedTrpcError(error)) {
    handleUnauthorized();
  }
}
