export function apiFetch(path: string, init: RequestInit = {}): Promise<Response> {
  const api = globalThis.__QUARK_API__;
  if (!api) return fetch(path, init);
  const headers = new Headers(init.headers);
  headers.set('Authorization', `Bearer ${api.token}`);
  return fetch(api.base + path, { ...init, headers });
}
