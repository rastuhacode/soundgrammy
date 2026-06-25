"use client";

import { handleErrorResponse } from "../errored-response";

export async function useFetch(
  url: string,
  init?: RequestInit,
): Promise<Response> {
  const response = await fetch(url, init);

  if (!response.ok) {
    handleErrorResponse(response);
  }

  return response;
}

/** Probes a resource URL after a browser load failure (img/audio) to detect 401. */
export async function probeUnauthorized(
  url: string,
  init?: RequestInit,
): Promise<void> {
  try {
    const response = await fetch(url, init);
    handleErrorResponse(response);
  } catch {
    // Ignore network errors — only a 401 response triggers logout.
  }
}
