import { type FormEvent, type ReactNode, useEffect, useState } from 'react';
import { UNAUTHORIZED_EVENT, fetchAuthStatus, login } from '../api';

type AuthState = 'checking' | 'login' | 'ready';

/**
 * Shows a login page while a `--auth token` server wants a token this browser
 * hasn't given yet.
 */
export default function AuthGate({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AuthState>('checking');

  useEffect(() => {
    let tokenLogin = false;
    fetchAuthStatus()
      .then((status) => {
        tokenLogin = status.mode === 'token';
        setState(tokenLogin && !status.authenticated ? 'login' : 'ready');
      })
      // A server without the auth endpoints has no token to ask for.
      .catch(() => setState('ready'));
    const onUnauthorized = () => {
      if (tokenLogin) setState('login');
    };
    window.addEventListener(UNAUTHORIZED_EVENT, onUnauthorized);
    return () => window.removeEventListener(UNAUTHORIZED_EVENT, onUnauthorized);
  }, []);

  if (state === 'checking') return null;
  if (state === 'login') return <LoginPage onLoggedIn={() => setState('ready')} />;
  return <>{children}</>;
}

// Helpers

function LoginPage({ onLoggedIn }: { onLoggedIn: () => void }) {
  const [token, setToken] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      if (await login(token)) onLoggedIn();
      else setError('Invalid token.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="login-page">
      <form className="panel login-form" onSubmit={submit}>
        <h2>Czar</h2>
        <label htmlFor="czar-api-token">API token</label>
        <input
          id="czar-api-token"
          type="password"
          autoComplete="current-password"
          autoFocus
          value={token}
          onChange={(event) => setToken(event.target.value)}
        />
        <p className="login-hint">Paste the token the server was started with.</p>
        {error && <p className="login-error">{error}</p>}
        <button type="submit" disabled={busy || !token}>Log in</button>
      </form>
    </div>
  );
}
