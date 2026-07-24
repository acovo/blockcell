import { useEffect, useState, useCallback, useRef } from 'react';
import { User, Save, Sparkles, RefreshCw, FileText, Loader2, AlertTriangle, RotateCcw, ChevronDown, ChevronUp, Send } from 'lucide-react';
import { getPersonaFiles, savePersonaFile, sendChat, type PersonaFile } from '@/lib/api';
import { wsManager } from '@/lib/ws';
import { applyPersonaContent, saveBeforeSwitch } from '@/lib/persona-operations';
import { useT } from '@/lib/i18n';

const FILE_META: Record<string, { label: string; desc: string; placeholder: string }> = {
  'AGENTS.md': {
    label: 'Agent 角色设定',
    desc: '定义 Agent 的角色、职责和行为模式',
    placeholder: '# Agent 角色设定\n\n## 角色定位\n你是一个...\n\n## 核心职责\n-\n\n## 行为准则\n-',
  },
  'SOUL.md': {
    label: '灵魂与性格',
    desc: '定义 Agent 的性格特征、价值观和思维方式',
    placeholder: '# 灵魂设定\n\n## 性格特征\n- 积极主动\n\n## 价值观\n-\n\n## 说话风格\n-',
  },
  'USER.md': {
    label: '用户信息',
    desc: '关于用户的背景、偏好和习惯信息',
    placeholder: '# 用户信息\n\n## 基本信息\n- 姓名：\n- 职业：\n\n## 偏好\n-\n\n## 工作习惯\n-',
  },
};

// Per-file quick prompts — each scoped to that file's purpose
const FILE_QUICK_PROMPTS: Record<string, string[]> = {
  'AGENTS.md': [
    '让角色定位更清晰，突出核心能力',
    '加强主动性：遇到问题自动给出方案',
    '精简行为准则，去掉废话只留关键约束',
    '从头生成一份适合开发者助手的角色设定',
  ],
  'SOUL.md': [
    '优化说话风格：简洁直接，不啰嗦',
    '加强专业感：理性、精准、技术导向',
    '调整为更有温度的对话风格',
    '从头生成性格设定，突出主动思考特质',
  ],
  'USER.md': [
    '精简结构，只保留影响 AI 行为的关键信息',
    '补充用户偏好和工作习惯字段',
    '整理为结构化 Markdown，方便 AI 读取',
    '从头生成一份用户信息模板',
  ],
};

// Per-file AI system prompt — strict scope so each file stays focused
const FILE_SYSTEM_PROMPTS: Record<string, string> = {
  'AGENTS.md': `你是 AI Agent 配置专家。当前任务：优化 AGENTS.md 文件。
该文件只应包含：Agent 的角色定位、核心职责、行为准则。
严禁写入：性格特征、用户信息、输出格式、项目背景——那些属于其他文件。
输出要求：精炼、无废话、直接可用于系统提示词注入。`,
  'SOUL.md': `你是 AI Agent 配置专家。当前任务：优化 SOUL.md 文件。
该文件只应包含：Agent 的性格特征、价值观、说话风格、思维方式。
严禁写入：角色职责、用户偏好、输出格式规范——那些属于其他文件。
输出要求：描述 Agent 内在特质，语言简洁，避免和 AGENTS.md 重复。`,
  'USER.md': `你是 AI Agent 配置专家。当前任务：优化 USER.md 文件。
该文件只应包含：用户的基本信息、职业背景、偏好、工作习惯、沟通风格。
严禁写入：Agent 的行为规则、性格设定——那些属于其他文件。
输出要求：结构化、字段清晰、专注描述用户而非 Agent。`,
};

const DEFAULT_QUICK_PROMPTS = [
  '精简内容，去掉冗余',
  '补充缺失的关键信息',
  '整理结构，提升可读性',
  '从头生成初始内容',
];

// Unsaved changes confirmation dialog
interface UnsavedDialogProps {
  fileName: string;
  onDiscard: () => void;
  onCancel: () => void;
  onSave: () => void;
  saving: boolean;
  t: (key: string, params?: Record<string, string | number>) => string;
}

function UnsavedDialog({ fileName, onDiscard, onCancel, onSave, saving, t }: UnsavedDialogProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-sm p-5 space-y-4">
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 rounded-full bg-rust/15 flex items-center justify-center shrink-0 mt-0.5">
            <AlertTriangle size={14} className="text-rust" />
          </div>
          <div>
            <p className="text-sm font-semibold">{t('persona.unsavedChanges')}</p>
            <p className="text-xs text-muted-foreground mt-1">
              <span className="font-medium text-foreground">{fileName}</span> {t('persona.unsavedDesc')}
            </p>
          </div>
        </div>
        <div className="flex gap-2 justify-end">
          <button
            onClick={onCancel}
            className="px-3 py-1.5 text-xs rounded-lg border border-border hover:bg-accent transition-colors"
          >
            {t('persona.stayHere')}
          </button>
          <button
            onClick={onDiscard}
            className="px-3 py-1.5 text-xs rounded-lg bg-muted hover:bg-accent border border-border transition-colors"
          >
            {t('persona.discardChanges')}
          </button>
          <button
            onClick={onSave}
            disabled={saving}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-lg bg-rust text-white hover:bg-rust/90 disabled:opacity-50 transition-colors"
          >
            {saving ? <Loader2 size={11} className="animate-spin" /> : <Save size={11} />}
            {t('persona.saveAndSwitch')}
          </button>
        </div>
      </div>
    </div>
  );
}

export function PersonaPage() {
  const t = useT();
  const [files, setFiles] = useState<PersonaFile[]>([]);
  const [activeFile, setActiveFile] = useState('AGENTS.md');
  const [contents, setContents] = useState<Record<string, string>>({});
  const [savedContents, setSavedContents] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saved' | 'error'>('idle');

  // AI panel state (inline, below editor)
  const [showAiPanel, setShowAiPanel] = useState(false);
  const [aiPrompt, setAiPrompt] = useState('');
  const [aiStreaming, setAiStreaming] = useState(false);
  const [aiError, setAiError] = useState('');
  const [aiProgress, setAiProgress] = useState(0); // 0–100
  const [originalContent, setOriginalContent] = useState<string | null>(null); // for undo
  const aiChatIdRef = useRef('');
  const aiProgressTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const aiPromptRef = useRef<HTMLTextAreaElement>(null);
  const aiBufferRef = useRef(''); // accumulates streamed tokens, applied on done
  const aiFileRef = useRef('');
  const aiOriginalContentRef = useRef('');

  // Unsaved-changes dialog state
  const [pendingSwitch, setPendingSwitch] = useState<string | null>(null); // target file name

  const fetchFiles = useCallback(async () => {
    setLoading(true);
    try {
      const res = await getPersonaFiles();
      setFiles(res.files);
      const m: Record<string, string> = {};
      for (const f of res.files) m[f.name] = f.content;
      setContents(m);
      setSavedContents(m);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetchFiles(); }, [fetchFiles]);

  const activeContent = contents[activeFile] ?? '';
  const isDirty = activeContent !== (savedContents[activeFile] ?? '');

  // Check if any file has unsaved changes
  const hasAnyDirty = files.some(f => (contents[f.name] ?? '') !== (savedContents[f.name] ?? ''));

  const handleSave = useCallback(async (fileName = activeFile, content = activeContent): Promise<boolean> => {
    setSaving(true);
    try {
      await savePersonaFile(fileName, content);
      setSavedContents(prev => ({ ...prev, [fileName]: content }));
      // Mark file as exists so sidebar 「未创建」label disappears immediately
      setFiles(prev => prev.map(f => f.name === fileName ? { ...f, exists: true } : f));
      setSaveStatus('saved');
      setTimeout(() => setSaveStatus('idle'), 2000);
      return true;
    } catch {
      setSaveStatus('error');
      return false;
    } finally {
      setSaving(false);
    }
  }, [activeFile, activeContent]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's' && isDirty && !saving) {
        e.preventDefault();
        handleSave();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [isDirty, saving, handleSave]);

  // Warn on browser page leave if there are unsaved changes
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (hasAnyDirty) {
        e.preventDefault();
        e.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [hasAnyDirty]);

  useEffect(() => {
    return () => {
      if (aiProgressTimerRef.current) {
        clearInterval(aiProgressTimerRef.current);
        aiProgressTimerRef.current = null;
      }
    };
  }, []);

  // Handle file switch with unsaved check
  const handleFileSwitch = useCallback((targetFile: string) => {
    if (targetFile === activeFile) return;
    if (isDirty) {
      setPendingSwitch(targetFile);
    } else {
      setActiveFile(targetFile);
      setShowAiPanel(false);
      setAiPrompt('');
      setAiError('');
      setOriginalContent(null);
    }
  }, [activeFile, isDirty]);

  const confirmSwitch = useCallback(async (saveFirst: boolean) => {
    if (!pendingSwitch) return;
    if (saveFirst) {
      const sourceFile = activeFile;
      const sourceContent = activeContent;
      const switched = await saveBeforeSwitch(
        () => handleSave(sourceFile, sourceContent),
        () => setActiveFile(pendingSwitch),
      );
      if (!switched) return;
    } else {
      setActiveFile(pendingSwitch);
    }
    setPendingSwitch(null);
    setShowAiPanel(false);
    setAiPrompt('');
    setAiError('');
    setOriginalContent(null);
  }, [pendingSwitch, handleSave, activeFile, activeContent]);

  // AI streaming: accumulate tokens in buffer ref, apply to editor only on done
  useEffect(() => {
    const offToken = wsManager.on('token', (ev: any) => {
      if (ev.chat_id !== aiChatIdRef.current) return;
      aiBufferRef.current += ev.delta || '';
    });
    const offDone = wsManager.on('message_done', (ev: any) => {
      if (ev.chat_id !== aiChatIdRef.current) return;
      // Use buffered tokens if available (streaming), else fall back to ev.content (non-streaming)
      const result = aiBufferRef.current || ev.content || '';
      setContents(prev => applyPersonaContent(prev, aiFileRef.current, result));
      setAiStreaming(false);
      setAiProgress(100);
      if (aiProgressTimerRef.current) clearInterval(aiProgressTimerRef.current);
    });
    const offErr = wsManager.on('error', (ev: any) => {
      if (ev.chat_id !== aiChatIdRef.current) return;
      setAiError(ev.message || '生成失败');
      setAiStreaming(false);
      setAiProgress(0);
      if (aiProgressTimerRef.current) clearInterval(aiProgressTimerRef.current);
      // restore original content on error
      setContents(prev => applyPersonaContent(prev, aiFileRef.current, aiOriginalContentRef.current));
    });
    return () => { offToken(); offDone(); offErr(); };
  }, []);

  const handleAiGenerate = useCallback(async () => {
    if (!aiPrompt.trim() || aiStreaming) return;
    const chatId = `persona-opt-${Date.now()}`;
    aiChatIdRef.current = chatId;
    aiFileRef.current = activeFile;
    aiOriginalContentRef.current = activeContent;
    aiBufferRef.current = ''; // reset buffer
    setAiError('');
    // save original content so we can undo (editor stays unchanged during generation)
    setOriginalContent(activeContent);
    setAiStreaming(true);
    setAiProgress(5);

    // Fake progress ticks while waiting for real tokens
    if (aiProgressTimerRef.current) clearInterval(aiProgressTimerRef.current);
    aiProgressTimerRef.current = setInterval(() => {
      setAiProgress(prev => {
        if (prev >= 90) { clearInterval(aiProgressTimerRef.current!); return prev; }
        return prev + Math.random() * 4;
      });
    }, 400);

    const systemPrompt = FILE_SYSTEM_PROMPTS[activeFile] || `你是 AI Agent 配置专家，请优化 ${activeFile} 文件。`;
    const msg = `${systemPrompt}

【当前内容】
\`\`\`
${activeContent || '（文件为空，请生成初始内容）'}
\`\`\`

【用户要求】${aiPrompt}

直接输出优化后的完整 Markdown，不要解释，不要重复其他文件的内容，直接从标题开始。`;

    try {
      await sendChat(msg, chatId);
    } catch (e: any) {
      setAiError(e.message || '发送失败');
      setAiStreaming(false);
      setAiProgress(0);
      if (aiProgressTimerRef.current) clearInterval(aiProgressTimerRef.current);
      setContents(prev => applyPersonaContent(prev, aiFileRef.current, aiOriginalContentRef.current));
    }
  }, [aiPrompt, activeContent, activeFile, aiStreaming, originalContent]);

  const handleAiUndo = useCallback(() => {
    if (originalContent !== null) {
      setContents(prev => applyPersonaContent(prev, aiFileRef.current, aiOriginalContentRef.current));
      setOriginalContent(null);
      setAiProgress(0);
    }
  }, [originalContent]);

  const toggleAiPanel = useCallback(() => {
    setShowAiPanel(v => !v);
    setAiError('');
    if (!showAiPanel) {
      // focus textarea after open
      setTimeout(() => aiPromptRef.current?.focus(), 80);
    }
  }, [showAiPanel]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 size={22} className="animate-spin text-muted-foreground" />
      </div>
    );
  }

  const meta = FILE_META[activeFile];

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className="border-b border-border py-4 pl-6 pr-16 flex items-center justify-between shrink-0">
        <div className="flex items-center gap-3">
          <User size={19} className="text-rust" />
          <div>
            <h1 className="text-base font-semibold">{t('persona.title')}</h1>
            <p className="text-[11px] text-muted-foreground">{t('persona.subtitle')}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button onClick={fetchFiles} className="p-2 rounded-lg hover:bg-accent text-muted-foreground" title={t('common.refresh')}>
            <RefreshCw size={14} />
          </button>
          <button
            onClick={() => { void handleSave(); }}
            disabled={!isDirty || saving}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
              saveStatus === 'saved' ? 'bg-[hsl(var(--brand-green)/0.10)] text-[hsl(var(--brand-green))] border border-[hsl(var(--brand-green)/0.28)]'
              : saveStatus === 'error' ? 'bg-destructive/10 text-destructive border border-destructive/30'
              : 'bg-[hsl(var(--brand-green)/0.12)] text-[hsl(var(--brand-green))] hover:bg-[hsl(var(--brand-green-strong))] border border-[hsl(var(--brand-green)/0.28)]'
            }`}
          >
            {saving ? <Loader2 size={12} className="animate-spin" /> : <Save size={12} />}
            {saveStatus === 'saved' ? t('settings.configSaved') : saveStatus === 'error' ? t('common.error') : t('common.save')}
          </button>
        </div>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* File sidebar */}
        <div className="w-[250px] border-r border-border shrink-0 overflow-y-auto py-3 px-2 space-y-1">
          {files.map(f => {
            const m = FILE_META[f.name];
            const dirty = (contents[f.name] ?? '') !== (savedContents[f.name] ?? '');
            const active = activeFile === f.name;
            return (
              <button
                key={f.name}
                onClick={() => handleFileSwitch(f.name)}
                className={`w-full text-left px-3 py-2.5 rounded-lg transition-colors ${
                  active ? 'bg-rust/10 border border-rust/25' : 'hover:bg-accent border border-transparent'
                }`}
              >
                <div className="flex items-center gap-2">
                  <FileText size={12} className={active ? 'text-rust' : 'text-muted-foreground'} />
                  <span className="text-sm font-medium flex-1 truncate">{f.name}</span>
                  {dirty && <span className="w-1.5 h-1.5 rounded-full bg-rust shrink-0" title={t('persona.unsaved')} />}
                </div>
                {m && <p className="text-[10px] text-muted-foreground mt-0.5 ml-[18px] truncate">{m.label}</p>}
                {!f.exists && <p className="text-[10px] text-muted-foreground/50 ml-[18px]">{t('persona.notCreated')}</p>}
              </button>
            );
          })}
        </div>

        {/* Editor + AI panel column — position:relative so overlay can cover it */}
        <div className="flex-1 flex flex-col overflow-hidden relative">
          {/* Sub-header */}
          {meta && (
            <div className="px-5 py-2.5 border-b border-border shrink-0 flex items-center gap-2 min-h-0">
              <span className="text-sm font-semibold">{meta.label}</span>
              <span className="text-xs text-muted-foreground truncate">— {meta.desc}</span>
              <div className="ml-auto shrink-0 flex items-center gap-2">
                {isDirty && (
                  <span className="text-[10px] px-2 py-0.5 rounded-full bg-rust/10 text-rust border border-rust/20">
                    {t('persona.unsaved')} · ⌘S
                  </span>
                )}
                {/* AI optimise toggle button */}
                <button
                  onClick={toggleAiPanel}
                  disabled={aiStreaming}
                  className={`flex items-center gap-1.5 px-2.5 py-1 text-[11px] font-medium rounded-lg border transition-colors disabled:opacity-50 ${
                    showAiPanel
                      ? 'bg-[hsl(var(--brand-green)/0.12)] border-[hsl(var(--brand-green)/0.30)] text-[hsl(var(--brand-green))]'
                      : 'bg-[hsl(var(--brand-green)/0.06)] border-[hsl(var(--brand-green)/0.18)] text-[hsl(var(--brand-green))] hover:bg-[hsl(var(--brand-green)/0.10)]'
                  }`}
                >
                  <Sparkles size={11} />
                  {t('persona.aiOptimize')}
                  {showAiPanel ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
                </button>
              </div>
            </div>
          )}

          {/* Editor area */}
          <div className="flex-1 relative overflow-hidden" style={{ minHeight: 0 }}>
            <textarea
              value={activeContent}
              onChange={e => setContents(prev => ({ ...prev, [activeFile]: e.target.value }))}
              placeholder={meta?.placeholder}
              disabled={aiStreaming}
              className="absolute inset-0 m-4 px-4 py-3 text-sm font-mono bg-muted/20 border border-border rounded-xl resize-none focus:outline-none focus:ring-1 focus:ring-rust/40 leading-relaxed"
              spellCheck={false}
            />
          </div>

          {/* Status bar */}
          <div className="px-5 py-1.5 border-t border-border shrink-0 flex items-center gap-4 text-[10px] text-muted-foreground">
            <span>{activeFile}</span>
            <span>{activeContent.length} {t('persona.chars')}</span>
            <span>{activeContent.split('\n').length} {t('persona.lines')}</span>
            {!files.find(f => f.name === activeFile)?.exists && (
              <span className="text-yellow-500">{t('persona.willCreate')}</span>
            )}
            {/* Undo after AI edit */}
            {originalContent !== null && !aiStreaming && (
              <button
                onClick={handleAiUndo}
                className="ml-auto flex items-center gap-1 text-[10px] text-muted-foreground hover:text-rust transition-colors"
                title={t('persona.undoAi')}
              >
                <RotateCcw size={10} />{t('persona.undoAi')}
              </button>
            )}
          </div>

          {/* ── AI streaming overlay — covers the entire editor column ── */}
          {aiStreaming && (
            <div className="absolute inset-0 z-20 flex flex-col items-center justify-center bg-black/10">
              <div className="bg-card border border-[hsl(var(--brand-green)/0.24)] rounded-2xl shadow-2xl px-12 py-8 flex flex-col items-center gap-4 w-1/2 max-w-none">
                <div className="flex items-center gap-2">
                  <span className="w-2 h-2 rounded-full bg-[hsl(var(--brand-green))] animate-pulse" />
                  <span className="text-sm text-[hsl(var(--brand-green))] font-semibold">{t('persona.aiOptimizing')}</span>
                </div>
                <div className="w-full space-y-1.5 px-1">
                  <div className="h-1.5 bg-border rounded-full overflow-hidden">
                    <div
                      className="h-full bg-[hsl(var(--brand-green))] rounded-full transition-all duration-300"
                      style={{ width: `${Math.min(aiProgress, 100)}%` }}
                    />
                  </div>
                  <p className="text-[11px] text-center text-muted-foreground">{t('persona.generating')}</p>
                </div>
              </div>
            </div>
          )}

          {/* ── AI Optimization Panel (inline, below editor) ── */}
          {showAiPanel && (
            <div className="border-t border-[hsl(var(--brand-green)/0.18)] bg-[hsl(var(--brand-green)/0.04)] px-4 py-3 space-y-2.5 shrink-0">
              {/* Quick prompts */}
              <div className="flex flex-wrap gap-1.5">
                {(FILE_QUICK_PROMPTS[activeFile] || DEFAULT_QUICK_PROMPTS).map(q => (
                  <button
                    key={q}
                    onClick={() => setAiPrompt(q)}
                    disabled={aiStreaming}
                    className={`text-[11px] px-2 py-1 rounded-md border transition-colors disabled:opacity-50 ${
                      aiPrompt === q
                        ? 'bg-[hsl(var(--brand-green)/0.12)] border-[hsl(var(--brand-green)/0.28)] text-[hsl(var(--brand-green))]'
                        : 'bg-muted/40 border-border hover:bg-accent text-muted-foreground'
                    }`}
                  >
                    {q}
                  </button>
                ))}
              </div>

              {/* Input row */}
              <div className="flex gap-2 items-end">
                <textarea
                  ref={aiPromptRef}
                  value={aiPrompt}
                  onChange={e => setAiPrompt(e.target.value)}
                  onKeyDown={e => { if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') handleAiGenerate(); }}
                  placeholder={t('persona.aiPlaceholder')}
                  disabled={aiStreaming}
                  rows={3}
                  className="flex-1 px-3 py-2 text-xs bg-background border border-border rounded-lg resize-none focus:outline-none focus:ring-1 focus:ring-[hsl(var(--brand-green)/0.35)] disabled:opacity-50"
                  style={{ minHeight: '4.5rem' }}
                />
                <button
                  onClick={handleAiGenerate}
                  disabled={!aiPrompt.trim() || aiStreaming}
                  className="flex items-center gap-1.5 px-3 py-2 text-xs font-medium rounded-lg bg-[hsl(var(--brand-green))] text-white hover:bg-[hsl(var(--brand-green-strong))] disabled:opacity-40 disabled:cursor-not-allowed transition-colors shrink-0"
                >
                  {aiStreaming
                    ? <Loader2 size={13} className="animate-spin" />
                    : <Send size={13} />}
                  {aiStreaming ? t('persona.generating2') : t('persona.generate')}
                </button>
              </div>

              {/* Error */}
              {aiError && (
                <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-destructive/10 text-destructive text-xs">
                  <AlertTriangle size={12} />{aiError}
                </div>
              )}

              {/* Hint: prompt is sent with current content */}
              {!aiStreaming && aiPrompt.trim() && (
                <p className="text-[10px] text-muted-foreground/60">
                  {t('persona.aiHint')}
                </p>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Unsaved changes dialog */}
      {pendingSwitch && (
        <UnsavedDialog
          fileName={activeFile}
          saving={saving}
          t={t}
          onCancel={() => setPendingSwitch(null)}
          onDiscard={() => {
            // revert unsaved changes for current file then switch
            setContents(prev => ({ ...prev, [activeFile]: savedContents[activeFile] ?? '' }));
            setActiveFile(pendingSwitch);
            setPendingSwitch(null);
            setShowAiPanel(false);
            setAiPrompt('');
            setAiError('');
            setOriginalContent(null);
          }}
          onSave={() => confirmSwitch(true)}
        />
      )}
    </div>
  );
}
