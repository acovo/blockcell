import { useEffect, useState } from 'react';
import {
  Package, Globe, Link2, Trash2, Download, CheckCircle, AlertCircle,
  Loader2, Search, ChevronDown, ChevronUp, RefreshCw, X, ExternalLink
} from 'lucide-react';
import {
  getSkills, deleteSkill, getHubSkills, installHubSkill, installExternalSkill,
  getToggles, updateToggle
} from '@/lib/api';
import { cn } from '@/lib/utils';
import { useT } from '@/lib/i18n';
import { wsManager } from '@/lib/ws';

type Tab = 'installed' | 'community' | 'external';

function SkillToggle({ enabled, onChange, enabledTitle, disabledTitle }: { enabled: boolean; onChange: (v: boolean) => void; enabledTitle?: string; disabledTitle?: string }) {
  return (
    <button
      type="button"
      onClick={(e) => { e.stopPropagation(); onChange(!enabled); }}
      title={enabled ? (enabledTitle ?? '') : (disabledTitle ?? '')}
      className={cn(
        'relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors focus:outline-none',
        enabled ? 'bg-cyber' : 'bg-muted'
      )}
    >
      <span className={cn(
        'inline-block h-3.5 w-3.5 transform rounded-full bg-white shadow transition-transform',
        enabled ? 'translate-x-[18px]' : 'translate-x-[3px]'
      )} />
    </button>
  );
}

// ─────────────────────────────────────────────
// Installed Skills Tab
// ─────────────────────────────────────────────
function InstalledSkillsTab({ onInstalledNamesChange }: { onInstalledNamesChange?: (names: Set<string>) => void }) {
  const [skills, setSkills] = useState<any[]>([]);
  const [toggles, setToggles] = useState<Record<string, boolean>>({});
  const t = useT();
  const [loading, setLoading] = useState(true);
  const [deletingName, setDeletingName] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [toast, setToast] = useState<{ type: 'success' | 'error'; msg: string } | null>(null);

  function showToast(type: 'success' | 'error', msg: string) {
    setToast({ type, msg });
    setTimeout(() => setToast(null), 3000);
  }

  function isEnabled(name: string) {
    return toggles[name] !== false;
  }

  async function load() {
    setLoading(true);
    try {
      const [skillsData, tg] = await Promise.allSettled([getSkills(), getToggles()]);
      const list = skillsData.status === 'fulfilled' ? (skillsData.value.skills ?? []) : [];
      setSkills(list);
      if (tg.status === 'fulfilled') setToggles(tg.value.skills ?? {});
      onInstalledNamesChange?.(new Set(list.map((s: any) => s.name as string)));
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { load(); }, []);

  useEffect(() => {
    const off = wsManager.on('evolution_triggered', () => {
      getToggles().then((tg) => setToggles(tg.skills ?? {})).catch(() => {});
    });
    return off;
  }, []);

  async function handleToggle(name: string) {
    const current = isEnabled(name);
    const next = !current;
    setToggles((prev) => ({ ...prev, [name]: next }));
    try {
      await updateToggle('skills', name, next);
    } catch {
      setToggles((prev) => ({ ...prev, [name]: current }));
    }
  }

  async function handleDelete(name: string) {
    setDeletingName(name);
    try {
      await deleteSkill(name);
      const updated = skills.filter((s) => s.name !== name);
      setSkills(updated);
      onInstalledNamesChange?.(new Set(updated.map((s: any) => s.name as string)));
      showToast('success', t('skills.deleted', { name }));
    } catch (e: any) {
      showToast('error', e.message || t('skills.deleteFailed'));
    } finally {
      setDeletingName(null);
      setConfirmDelete(null);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="w-5 h-5 animate-spin text-cyber" />
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {toast && (
        <div className={cn(
          'flex items-center gap-2 text-sm px-4 py-2.5 rounded-lg border',
          toast.type === 'success'
            ? 'bg-green-500/10 border-green-500/20 text-green-400'
            : 'bg-red-500/10 border-red-500/20 text-red-400'
        )}>
          {toast.type === 'success' ? <CheckCircle className="w-4 h-4" /> : <AlertCircle className="w-4 h-4" />}
          {toast.msg}
        </div>
      )}

      <div className="flex items-center justify-between mb-2">
        <span className="text-sm text-muted-foreground">{t('skills.count', { n: skills.length })}</span>
        <button onClick={load} className="text-xs text-muted-foreground hover:text-foreground flex items-center gap-1">
          <RefreshCw className="w-3.5 h-3.5" /> {t('skills.refresh')}
        </button>
      </div>

      {skills.length === 0 ? (
        <div className="text-center py-12 text-muted-foreground text-sm">
          <Package className="w-10 h-10 mx-auto mb-3 opacity-30" />
          {t('skills.empty')}
        </div>
      ) : (
        skills.map((skill) => (
          <div
            key={skill.name}
            className="flex items-center justify-between p-4 rounded-xl border border-border/40 bg-card/60 hover:border-border transition-colors"
          >
            <div className="flex items-center gap-3 min-w-0">
              <div className={cn(
                'p-2 rounded-lg',
                skill.source === 'builtin' ? 'bg-muted/60 text-muted-foreground' : 'bg-cyber/10 text-cyber'
              )}>
                <Package className="w-4 h-4" />
              </div>
              <div className="min-w-0">
                <p className="text-sm font-medium text-foreground truncate">{skill.name}</p>
                <p className="text-xs text-muted-foreground">
                  {skill.source === 'builtin' ? t('skills.builtin') : t('skills.user')}
                  {skill.has_rhai && ' · Rhai'}
                  {skill.has_md && ' · MD'}
                </p>
                {skill.meta?.description && (
                  <p className="text-xs text-muted-foreground/80 mt-0.5 truncate max-w-xs">
                    {typeof skill.meta.description === 'string' ? skill.meta.description : ''}
                  </p>
                )}
              </div>
            </div>

            <div className="flex items-center gap-3 shrink-0">
              {/* Enable/disable toggle — calls same API as dashboard */}
              <div className="flex items-center gap-1.5">
                <span className={cn('text-xs', isEnabled(skill.name) ? 'text-cyber' : 'text-muted-foreground')}>
                  {isEnabled(skill.name) ? t('skills.enabled') : t('skills.disabled')}
                </span>
                <SkillToggle
                  enabled={isEnabled(skill.name)}
                  enabledTitle={t('skills.clickDisable')}
                  disabledTitle={t('skills.clickEnable')}
                  onChange={() => handleToggle(skill.name)}
                />
              </div>

              {/* Delete (user skills only) */}
              {skill.source !== 'builtin' && (
                confirmDelete === skill.name ? (
                  <div className="flex items-center gap-1">
                    <button
                      onClick={() => handleDelete(skill.name)}
                      disabled={deletingName === skill.name}
                      className="text-xs px-2.5 py-1 rounded-md bg-red-500/20 border border-red-500/30 text-red-400 hover:bg-red-500/30 transition-colors"
                    >
                      {deletingName === skill.name ? <Loader2 className="w-3 h-3 animate-spin" /> : t('skills.confirmDelete')}
                    </button>
                    <button
                      onClick={() => setConfirmDelete(null)}
                      className="text-xs px-2 py-1 rounded-md border border-border/40 text-muted-foreground hover:text-foreground"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </div>
                ) : (
                  <button
                    onClick={() => setConfirmDelete(skill.name)}
                    className="p-1.5 rounded-md text-muted-foreground hover:text-red-400 hover:bg-red-400/10 transition-colors"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                )
              )}
            </div>
          </div>
        ))
      )}
    </div>
  );
}

// ─────────────────────────────────────────────
// Community Skills Tab
// ─────────────────────────────────────────────
function CommunitySkillsTab({ installedNames }: { installedNames: Set<string> }) {
  const [skills, setSkills] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const t = useT();
  const [error, setError] = useState('');
  const [search, setSearch] = useState('');
  const [installing, setInstalling] = useState<string | null>(null);
  // 'pre-installed' = already installed before this session; 'installing' | 'done' | 'error' = this session
  const [progress, setProgress] = useState<Record<string, 'pre-installed' | 'installing' | 'done' | 'error'>>({});
  const [expanded, setExpanded] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    setError('');
    try {
      const data = await getHubSkills();
      const list = data.skills ?? data.trending_skills ?? data ?? [];
      const arr = Array.isArray(list) ? list : [];
      setSkills(arr);
      // Mark already-installed skills
      const preInstalled: Record<string, 'pre-installed'> = {};
      arr.forEach((s: any) => {
        const name = s.name ?? s.skill_name ?? '';
        if (name && installedNames.has(name)) preInstalled[name] = 'pre-installed';
      });
      setProgress((prev) => ({ ...preInstalled, ...prev }));
    } catch (e: any) {
      setError(e.message || t('skills.communityEmpty'));
      setSkills([]);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { load(); }, []);

  async function handleInstall(skillName: string) {
    setInstalling(skillName);
    setProgress((p) => ({ ...p, [skillName]: 'installing' }));
    try {
      await installHubSkill(skillName);
      setProgress((p) => ({ ...p, [skillName]: 'done' }));
    } catch {
      setProgress((p) => ({ ...p, [skillName]: 'error' }));
    } finally {
      setInstalling(null);
    }
  }

  const filtered = skills.filter((s: any) => {
    if (!search) return true;
    const q = search.toLowerCase();
    return (
      (s.name ?? '').toLowerCase().includes(q) ||
      (s.description ?? '').toLowerCase().includes(q) ||
      (s.tags ?? []).some((t: string) => t.toLowerCase().includes(q))
    );
  });

  if (loading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="w-5 h-5 animate-spin text-cyber" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center py-12 gap-3">
        <AlertCircle className="w-8 h-8 text-muted-foreground" />
        <p className="text-sm text-muted-foreground">{error}</p>
        <button onClick={load} className="text-xs text-cyber hover:underline">{t('skills.retry')}</button>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t('skills.searchPlaceholder')}
          className="w-full bg-muted/40 border border-border rounded-lg pl-9 pr-4 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-cyber/60"
        />
      </div>

      {filtered.length === 0 ? (
        <div className="text-center py-10 text-muted-foreground text-sm">
          <Globe className="w-8 h-8 mx-auto mb-2 opacity-30" />
          {search ? t('skills.noMatch') : t('skills.communityEmpty')}
        </div>
      ) : (
        filtered.map((skill: any) => {
          const name = skill.name ?? skill.skill_name ?? '';
          const state = progress[name];
          const isExpanded = expanded === name;

          return (
            <div key={name} className="rounded-xl border border-border/40 bg-card/60 overflow-hidden">
              <div className="flex items-start gap-3 p-4">
                <div className="p-2 rounded-lg bg-cyber/10 text-cyber shrink-0 mt-0.5">
                  <Globe className="w-4 h-4" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-sm font-medium text-foreground">{name}</p>
                    <div className="flex items-center gap-1.5 shrink-0">
                      {skill.version && (
                        <span className="text-xs text-muted-foreground bg-muted/60 px-1.5 py-0.5 rounded">
                          v{skill.version}
                        </span>
                      )}
                      <button
                        onClick={() => setExpanded(isExpanded ? null : name)}
                        className="text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {isExpanded ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
                      </button>
                    </div>
                  </div>
                  <p className="text-xs text-muted-foreground mt-0.5 line-clamp-2">
                    {skill.description ?? t('skills.noDesc')}
                  </p>
                  {skill.tags?.length > 0 && (
                    <div className="flex flex-wrap gap-1 mt-1.5">
                      {skill.tags.slice(0, 4).map((t: string) => (
                        <span key={t} className="text-xs bg-muted/50 text-muted-foreground px-1.5 py-0.5 rounded">
                          {t}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              </div>

              {/* Expanded details */}
              {isExpanded && (
                <div className="border-t border-border/30 px-4 pb-4 pt-3 space-y-2">
                  {skill.readme && (
                    <p className="text-xs text-muted-foreground whitespace-pre-wrap max-h-32 overflow-y-auto">
                      {skill.readme}
                    </p>
                  )}
                  {skill.author && (
                    <p className="text-xs text-muted-foreground">{t('skills.author')}: {skill.author}</p>
                  )}
                </div>
              )}

              {/* Install bar */}
              <div className="border-t border-border/30 px-4 py-3 flex items-center justify-between">
                {state === 'installing' ? (
                  <div className="flex-1 flex items-center gap-3">
                    <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
                      <div className="h-full bg-cyber rounded-full animate-pulse w-2/3" />
                    </div>
                    <span className="text-xs text-cyber">{t('skills.installing')}</span>
                  </div>
                ) : state === 'done' ? (
                  <span className="flex items-center gap-1.5 text-xs text-green-400">
                    <CheckCircle className="w-3.5 h-3.5" /> {t('skills.installDone')}
                  </span>
                ) : state === 'pre-installed' ? (
                  <span className="flex items-center gap-1.5 text-xs text-cyber/70">
                    <CheckCircle className="w-3.5 h-3.5" /> {t('skills.installed')}
                  </span>
                ) : state === 'error' ? (
                  <span className="flex items-center gap-1.5 text-xs text-red-400">
                    <AlertCircle className="w-3.5 h-3.5" /> {t('skills.installError')}
                  </span>
                ) : (
                  <button
                    onClick={() => handleInstall(name)}
                    disabled={installing !== null}
                    className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-lg bg-cyber/15 border border-cyber/30 text-cyber hover:bg-cyber/25 transition-colors disabled:opacity-50"
                  >
                    <Download className="w-3.5 h-3.5" /> {t('skills.oneClickInstall')}
                  </button>
                )}
              </div>
            </div>
          );
        })
      )}
    </div>
  );
}

// ─────────────────────────────────────────────
// External Skill Install Tab
// ─────────────────────────────────────────────
function ExternalSkillTab() {
  const t = useT();
  const [url, setUrl] = useState('');
  const [status, setStatus] = useState<'idle' | 'installing' | 'done' | 'error'>('idle');
  const [message, setMessage] = useState('');
  const [result, setResult] = useState<any>(null);

  async function handleInstall() {
    if (!url.trim()) return;
    setStatus('installing');
    setMessage('');
    setResult(null);
    try {
      const data = await installExternalSkill(url.trim());
      setResult(data);
      if (data.status === 'error') {
        setStatus('error');
        setMessage(data.message || t('skills.installFailed'));
      } else {
        setStatus('done');
        setMessage(data.message || t('skills.installDone'));
      }
    } catch (e: any) {
      setStatus('error');
      setMessage(e.message || t('skills.installFailed'));
    }
  }

  function reset() {
    setUrl('');
    setStatus('idle');
    setMessage('');
    setResult(null);
  }

  const skillSources = [
    {
      name: t('skills.officialRepo'),
      desc: t('skills.officialRepoDesc'),
      url: 'https://github.com/openclaw/skills',
      icon: <Globe className="w-5 h-5" />,
    },
    {
      name: t('skills.marketplace'),
      desc: t('skills.marketplaceDesc'),
      url: 'https://openclawskills.best',
      icon: <Package className="w-5 h-5" />,
    },
    {
      name: t('skills.awesomeList'),
      desc: t('skills.awesomeListDesc'),
      url: 'https://github.com/VoltAgent/awesome-openclaw-skills',
      icon: <ExternalLink className="w-5 h-5" />,
    },
  ];

  return (
    <div className="space-y-5 max-w-2xl">
      {/* Install Form */}
      <div className="p-4 rounded-xl bg-cyber/5 border border-cyber/20">
        <h3 className="text-sm font-medium text-foreground mb-1">{t('skills.installExternal')}</h3>
        <p className="text-xs text-muted-foreground">
          {t('skills.installExternalDesc')}
        </p>
      </div>

      <div className="space-y-2">
        <label className="text-sm font-medium text-foreground">{t('skills.skillUrl')}</label>
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Link2 className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/my-skill.zip"
              disabled={status === 'installing'}
              className="w-full bg-muted/40 border border-border rounded-lg pl-9 pr-4 py-2.5 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-cyber/60 transition-colors disabled:opacity-50"
            />
          </div>
          <button
            onClick={handleInstall}
            disabled={!url.trim() || status === 'installing'}
            className="flex items-center gap-2 px-4 py-2.5 rounded-lg bg-cyber/20 border border-cyber/40 text-cyber text-sm font-medium hover:bg-cyber/30 transition-colors disabled:opacity-50 shrink-0"
          >
            {status === 'installing' ? (
              <><Loader2 className="w-4 h-4 animate-spin" /> {t('skills.installing2')}</>
            ) : (
              <><Download className="w-4 h-4" /> {t('skills.install')}</>
            )}
          </button>
        </div>
      </div>

      {/* Progress */}
      {status === 'installing' && (
        <div className="space-y-2">
          <div className="h-2 bg-muted rounded-full overflow-hidden">
            <div className="h-full bg-cyber rounded-full animate-pulse w-1/2 transition-all" />
          </div>
          <p className="text-xs text-muted-foreground">{t('skills.downloadingParsing')}</p>
        </div>
      )}

      {/* Result */}
      {(status === 'done' || status === 'error') && (
        <div className={cn(
          'p-4 rounded-xl border space-y-2',
          status === 'done'
            ? 'bg-green-500/5 border-green-500/20'
            : 'bg-red-500/5 border-red-500/20'
        )}>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {status === 'done' ? (
                <CheckCircle className="w-4 h-4 text-green-400" />
              ) : (
                <AlertCircle className="w-4 h-4 text-red-400" />
              )}
              <span className={cn('text-sm font-medium', status === 'done' ? 'text-green-400' : 'text-red-400')}>
                {status === 'done' ? t('skills.evolutionTriggered') : t('skills.installFailed')}
              </span>
            </div>
            <button onClick={reset} className="text-xs text-muted-foreground hover:text-foreground">
              <X className="w-4 h-4" />
            </button>
          </div>
          {message && <p className="text-xs text-muted-foreground">{message}</p>}
          {result?.evolution_id && (
            <p className="text-xs font-mono text-muted-foreground/70 truncate">{t('skills.evolutionId')}: {result.evolution_id}</p>
          )}
        </div>
      )}

      {/* Examples */}
      <div className="space-y-2">
        <p className="text-xs font-medium text-muted-foreground">{t('skills.supportedFormats')}</p>
        <ul className="space-y-1.5 text-xs text-muted-foreground">
          <li className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-cyber/60" />
            {t('skills.fmt.githubDir')}
          </li>
          <li className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-cyber/60" />
            {t('skills.fmt.githubFile')}
          </li>
          <li className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-cyber/60" />
            {t('skills.fmt.zip')}
          </li>
        </ul>
        <a
          href="https://github.com/blockcell-labs/blockcell/blob/main/docs/04_skill_system.md"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-1.5 text-xs text-cyber hover:underline mt-2"
        >
          <ExternalLink className="w-3.5 h-3.5" />
          {t('skills.devDocs')}
        </a>
      </div>

      {/* Skill Sources Section */}
      <div className="space-y-3">
        <div>
          <h3 className="text-sm font-medium text-foreground">{t('skills.sources')}</h3>
          <p className="text-xs text-muted-foreground mt-0.5">{t('skills.sourcesDesc')}</p>
        </div>
        <div className="grid gap-3">
          {skillSources.map((source) => (
            <a
              key={source.url}
              href={source.url}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-start gap-3 p-4 rounded-xl border border-border/40 bg-card/60 hover:border-cyber/40 hover:bg-card transition-all group"
            >
              <div className="p-2 rounded-lg bg-cyber/10 text-cyber shrink-0 group-hover:bg-cyber/20 transition-colors">
                {source.icon}
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <p className="text-sm font-medium text-foreground group-hover:text-cyber transition-colors">
                    {source.name}
                  </p>
                  <ExternalLink className="w-3.5 h-3.5 text-muted-foreground group-hover:text-cyber transition-colors" />
                </div>
                <p className="text-xs text-muted-foreground mt-0.5">{source.desc}</p>
              </div>
            </a>
          ))}
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────
// Main Page
// ─────────────────────────────────────────────
export function SkillsPage() {
  const [activeTab, setActiveTab] = useState<Tab>('installed');
  // Shared installed names — updated by InstalledSkillsTab, consumed by CommunitySkillsTab
  const [installedNames, setInstalledNames] = useState<Set<string>>(new Set());

  const t = useT();
  const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
    { id: 'installed', label: t('skills.tab.installed'), icon: <Package className="w-4 h-4" /> },
    { id: 'community', label: t('skills.tab.community'), icon: <Globe className="w-4 h-4" /> },
    { id: 'external', label: t('skills.tab.external'), icon: <Link2 className="w-4 h-4" /> },
  ];

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="border-b border-border px-6 py-4">
        <h1 className="text-xl font-semibold text-foreground">{t('skills.title')}</h1>
        <p className="text-sm text-muted-foreground mt-0.5">{t('skills.subtitle')}</p>
      </div>

      {/* Tabs */}
      <div className="border-b border-border px-6">
        <div className="flex gap-1">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                'flex items-center gap-2 px-4 py-3 text-sm font-medium border-b-2 transition-colors',
                activeTab === tab.id
                  ? 'border-cyber text-cyber'
                  : 'border-transparent text-muted-foreground hover:text-foreground hover:border-border'
              )}
            >
              {tab.icon}
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Tab content — keep InstalledSkillsTab mounted to maintain installedNames */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className={activeTab === 'installed' ? '' : 'hidden'}>
          <InstalledSkillsTab onInstalledNamesChange={setInstalledNames} />
        </div>
        {activeTab === 'community' && <CommunitySkillsTab installedNames={installedNames} />}
        {activeTab === 'external' && <ExternalSkillTab />}
      </div>
    </div>
  );
}
