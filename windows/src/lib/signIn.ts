/** Calls `retry` once after `seconds`; nothing without them. Returns what cancels it. */
export function retryLater(seconds: number | null | undefined, retry: () => void): () => void {
  if (!seconds) return () => {};
  const timer = setTimeout(retry, seconds * 1000);
  return () => clearTimeout(timer);
}
