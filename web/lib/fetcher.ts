export const fetcher = async (url: string) => {
  const res = await fetch(url);
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    const message = body && typeof body === "object" && "error" in body
      ? String(body.error)
      : `HTTP ${res.status}`;
    throw new Error(message);
  }
  return res.json();
};
