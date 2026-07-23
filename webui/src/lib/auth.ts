const TOKEN_KEY = 'blockcell_token';

function browserSessionStorage(): Storage | undefined {
  return typeof window === 'undefined' ? undefined : window.sessionStorage;
}

function browserLocalStorage(): Storage | undefined {
  return typeof window === 'undefined' ? undefined : window.localStorage;
}

export function getAuthToken(
  session: Storage | undefined = browserSessionStorage(),
  local: Storage | undefined = browserLocalStorage(),
): string | null {
  const current = session?.getItem(TOKEN_KEY) ?? null;
  if (current) return current;
  const legacy = local?.getItem(TOKEN_KEY) ?? null;
  if (legacy && session) session.setItem(TOKEN_KEY, legacy);
  if (legacy) local?.removeItem(TOKEN_KEY);
  return legacy;
}

export function setAuthToken(
  token: string,
  session: Storage | undefined = browserSessionStorage(),
  local: Storage | undefined = browserLocalStorage(),
) {
  session?.setItem(TOKEN_KEY, token);
  local?.removeItem(TOKEN_KEY);
}

export function clearAuthToken(
  session: Storage | undefined = browserSessionStorage(),
  local: Storage | undefined = browserLocalStorage(),
) {
  session?.removeItem(TOKEN_KEY);
  local?.removeItem(TOKEN_KEY);
}

export function authProtocol(token: string): string {
  const encoded = [...new TextEncoder().encode(token)]
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
  return `blockcell-auth.${encoded}`;
}
