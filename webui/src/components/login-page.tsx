import { useState } from 'react';
import { Loader2, AlertCircle, Eye, EyeOff } from 'lucide-react';
import { BlockcellLogo } from './blockcell-logo';
import { cn } from '@/lib/utils';
import { useI18nStore, useT, type Locale } from '@/lib/i18n';
import { API_BASE } from '@/lib/api';
import { setAuthToken } from '@/lib/auth';

interface LoginPageProps {
  onLogin: () => void;
}

export function LoginPage({ onLogin }: LoginPageProps) {
  const t = useT();
  const { locale, setLocale } = useI18nStore();
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);

  const languages: { value: Locale; label: string }[] = [
    { value: 'zh', label: '中文' },
    { value: 'en', label: 'English' },
  ];

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!password.trim()) return;

    setLoading(true);
    setError('');

    try {
      const res = await fetch(`${API_BASE}/v1/auth/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ password }),
      });

      const data = await res.json();

      if (res.ok && data.token) {
        setAuthToken(data.token);
        onLogin();
      } else {
        setError(data.error || t('login.invalidPassword'));
      }
    } catch {
      setError(t('login.cannotConnect'));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="relative flex items-center justify-center min-h-screen bg-background">
      <div className="absolute top-4 right-4">
        <div className="flex items-center gap-1 bg-accent/50 rounded-lg p-0.5">
          {languages.map((lang) => (
            <button
              key={lang.value}
              onClick={() => setLocale(lang.value)}
              className={cn(
                'px-3 py-1.5 text-xs rounded-md transition-colors',
                locale === lang.value
                  ? 'bg-primary text-primary-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground hover:bg-accent'
              )}
            >
              {lang.label}
            </button>
          ))}
        </div>
      </div>
      <div className="w-full max-w-sm mx-4">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center mb-4">
            <BlockcellLogo size="lg" />
          </div>
          <h1 className="text-2xl font-bold tracking-tight">
            BLOCK<span className="text-[#ea580c]">CELL</span>
          </h1>
          <p className="text-sm text-muted-foreground mt-1">{t('login.subtitle')}</p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="relative">
            <input
              type={showPassword ? 'text' : 'password'}
              value={password}
              onChange={(e) => { setPassword(e.target.value); setError(''); }}
              placeholder={t('login.password')}
              autoFocus
              className="w-full px-4 py-3 pr-10 text-sm bg-card border border-border rounded-xl outline-none focus:ring-2 focus:ring-ring placeholder:text-muted-foreground"
            />
            <button
              type="button"
              onClick={() => setShowPassword(!showPassword)}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
              tabIndex={-1}
            >
              {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </div>

          {error && (
            <div className="flex items-center gap-2 text-sm text-red-500">
              <AlertCircle size={14} />
              <span>{error}</span>
            </div>
          )}

          <button
            type="submit"
            disabled={loading || !password.trim()}
            className="w-full py-3 text-sm font-medium rounded-xl bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 flex items-center justify-center gap-2"
          >
            {loading ? <Loader2 size={16} className="animate-spin" /> : null}
            {loading ? t('login.signingIn') : t('login.signIn')}
          </button>
        </form>

        <p className="text-xs text-muted-foreground text-center mt-6">
          {t('login.hint')}
        </p>
      </div>
    </div>
  );
}
