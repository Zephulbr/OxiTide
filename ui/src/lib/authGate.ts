import { AUTH_ERROR_EVENT, AUTH_SUCCESS_EVENT, type SessionInfoDto } from "./api";

export interface AuthGateDeps {
  listen: (
    event: string,
    handler: (event: { payload: unknown }) => void,
  ) => Promise<() => void>;
  takeResult: () => Promise<SessionInfoDto | null>;
  onLogin: () => void;
  onError: (message: string) => void;
}

/// Bridges the deep-link login events and the cold-start buffered result to the
/// app's logged-in state. One instance lives for the app's lifetime.
export function createAuthGate(deps: AuthGateDeps): { dispose: () => void } {
  const offs: Array<() => void> = [];
  let disposed = false;
  const keep = (off: () => void) => {
    if (disposed) off();
    else offs.push(off);
  };

  void deps
    .listen(AUTH_SUCCESS_EVENT, () => deps.onLogin())
    .then(keep)
    .catch(() => {});
  void deps
    .listen(AUTH_ERROR_EVENT, (event) => deps.onError(String(event.payload ?? "")))
    .then(keep)
    .catch(() => {});

  void deps
    .takeResult()
    .then((session) => {
      if (!disposed && session) deps.onLogin();
    })
    .catch((e) => {
      if (!disposed) deps.onError(String(e));
    });

  return {
    dispose() {
      disposed = true;
      for (const off of offs) off();
      offs.length = 0;
    },
  };
}
