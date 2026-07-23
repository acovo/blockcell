import { memo, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { Copy, Check } from 'lucide-react';
import { useAgentStore } from '@/lib/store';
import { useAuthenticatedFileUrl } from '@/lib/use-authenticated-file-url';

function LocalMarkdownImage({ path, alt, agentId }: { path: string; alt: string; agentId?: string }) {
  const url = useAuthenticatedFileUrl(path, agentId);
  if (!url) return null;
  return <img src={url} alt={alt} className="my-2 max-h-[300px] max-w-full rounded-xl border border-border/70 object-contain shadow-sm" loading="lazy" />;
}

function CodeBlock({ language, code }: { language: string; code: string }) {
  const [copied, setCopied] = useState(false);

  function handleCopy() {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="relative group not-prose">
      <div className="flex items-center justify-between rounded-t-xl border border-b-0 border-border/70 bg-muted/75 px-3 py-1.5 text-[11px] text-muted-foreground shadow-sm">
        <span className="font-medium tracking-wide text-foreground/70">{language}</span>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 rounded-md px-1.5 py-0.5 hover:bg-background/60 hover:text-foreground transition-colors"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
          <span>{copied ? 'Copied' : 'Copy'}</span>
        </button>
      </div>
      <SyntaxHighlighter
        language={language}
        style={oneDark}
        customStyle={{
          margin: 0,
          borderTopLeftRadius: 0,
          borderTopRightRadius: 0,
          fontSize: '0.8rem',
        }}
      >
        {code}
      </SyntaxHighlighter>
    </div>
  );
}

export const MarkdownContent = memo(function MarkdownContent({ content }: { content: string }) {
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  return (
    <div className="prose prose-sm dark:prose-invert max-w-none prose-blockcell chat-markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          h1({ children }) {
            return <h1 className="mt-1 mb-3 text-base font-semibold text-foreground tracking-tight">{children}</h1>;
          },
          h2({ children }) {
            return <h2 className="mt-4 mb-2 text-[0.95rem] font-semibold text-foreground/95 tracking-tight">{children}</h2>;
          },
          h3({ children }) {
            return <h3 className="mt-3 mb-2 text-sm font-semibold text-foreground/90">{children}</h3>;
          },
          p({ children }) {
            return <p className="my-2 leading-7 text-foreground/90">{children}</p>;
          },
          ul({ children }) {
            return <ul className="my-2 space-y-1">{children}</ul>;
          },
          ol({ children }) {
            return <ol className="my-2 space-y-1">{children}</ol>;
          },
          li({ children }) {
            return <li className="marker:text-primary/80">{children}</li>;
          },
          blockquote({ children }) {
            return (
              <blockquote className="my-3 rounded-r-lg border-l-4 border-primary/35 bg-muted/50 px-4 py-2.5 text-foreground/85 shadow-sm">
                {children}
              </blockquote>
            );
          },
          hr() {
            return <hr className="my-4 border-border/80" />;
          },
          strong({ children }) {
            return <strong className="font-semibold text-foreground">{children}</strong>;
          },
          code({ node, className, children, ...props }) {
            const match = /language-(\w+)/.exec(className || '');
            const codeStr = String(children).replace(/\n$/, '');

            if (match) {
              return <CodeBlock language={match[1]} code={codeStr} />;
            }
            return (
              <code className="rounded-md border border-border/60 bg-muted/70 px-1.5 py-0.5 text-[0.8em] font-mono text-primary/90" {...props}>
                {children}
              </code>
            );
          },
          a({ href, children }) {
            return (
              <a href={href} target="_blank" rel="noopener noreferrer" className="font-medium text-primary hover:text-primary/80 underline decoration-primary/35 underline-offset-4 transition-colors">
                {children}
              </a>
            );
          },
          img({ src, alt }) {
            // Route any local file path (relative or absolute) through the serve
            // endpoint; only http(s)/data/blob URLs are passed through unchanged.
            const isRemoteUrl = src && /^(https?:|data:|blob:)/i.test(src);
            if (!isRemoteUrl) {
              return <LocalMarkdownImage path={src ?? ''} alt={alt || ''} agentId={selectedAgentId} />;
            }
            return (
              <img
                src={src}
                alt={alt || ''}
                className="my-2 max-h-[300px] max-w-full rounded-xl border border-border/70 object-contain shadow-sm"
                loading="lazy"
              />
            );
          },
          table({ children }) {
            return (
              <div className="my-3 overflow-x-auto rounded-xl border border-border/70 bg-card/40 shadow-sm">
                <table>{children}</table>
              </div>
            );
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
});
