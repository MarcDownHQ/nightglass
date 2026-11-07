/** Settings without secrets live in the webview's storage; sources (passwords) go through DPAPI. */
export const prefs = {
  get(key: string): string | null {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  },
  set(key: string, value: string | null) {
    try {
      if (value === null) localStorage.removeItem(key);
      else localStorage.setItem(key, value);
    } catch {
      // Private storage unavailable: the setting lasts for this session only.
    }
  },
  json<T>(key: string, fallback: T): T {
    try {
      const raw = this.get(key);
      return raw === null ? fallback : (JSON.parse(raw) as T);
    } catch {
      return fallback;
    }
  },
  setJson(key: string, value: unknown) {
    this.set(key, JSON.stringify(value));
  },
};
