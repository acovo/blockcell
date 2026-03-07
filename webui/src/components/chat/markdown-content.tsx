import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { useState } from 'react';
import { Copy, Check } from 'lucide-react';
import { mediaFileUrl } from '@/lib/api';
import { useAgentStore } from '@/lib/store';

export function MarkdownContent({ content }: { content: string }) {
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  return (
    <div className="prose prose-sm dark:prose-invert max-w-none prose-blockcell">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          h1({ children }) {
            return <h1 className="mt-1 mb-3 text-base font-semibold text-rust tracking-tight">{children}</h1>;
          },
          h2({ children }) {
            return <h2 className="mt-4 mb-2 text-[0.95rem] font-semibold text-cyber tracking-tight">{children}</h2>;
          },
          h3({ children }) {
            return <h3 className="mt-3 mb-2 text-sm font-semibold text-foreground">{children}</h3>;
          },
          p({ children }) {
            return <p className="my-2 leading-7 text-foreground/95">{children}</p>;
          },
          ul({ children }) {
            return <ul className="my-2 space-y-1">{children}</ul>;
          },
          ol({ children }) {
            return <ol className="my-2 space-y-1">{children}</ol>;
          },
          li({ children }) {
            return <li className="marker:text-rust">{children}</li>;
          },
          blockquote({ children }) {
            return (
              <blockquote className="my-3 rounded-r-md border-l-4 border-cyber/50 bg-cyber/5 px-4 py-2 text-foreground/90">
                {children}
              </blockquote>
            );
          },
          hr() {
            return <hr className="my-4 border-border/80" />;
          },
          strong({ children }) {
            return <strong className="font-semibold text-rust">{children}</strong>;
          },
          code({ node, className, children, ...props }) {
            const match = /language-(\w+)/.exec(className || '');
            const codeStr = String(children).replace(/\n$/, '');

            if (match) {
              return <CodeBlock language={match[1]} code={codeStr} />;
            }
            return (
              <code className="bg-muted px-1.5 py-0.5 rounded text-xs font-mono" {...props}>
                {children}
              </code>
            );
          },
          a({ href, children }) {
            return (
              <a href={href} target="_blank" rel="noopener noreferrer" className="text-rust hover:text-rust-light underline">
                {children}
              </a>
            );
          },
          img({ src, alt }) {
            // Route local file paths through the serve endpoint
            const resolvedSrc = src && src.startsWith('/') ? mediaFileUrl(src, selectedAgentId) : src;
            return (
              <img
                src={resolvedSrc}
                alt={alt || ''}
                className="max-w-full max-h-[300px] object-contain rounded-lg border border-border my-2"
                loading="lazy"
              />
            );
          },
          table({ children }) {
            return (
              <div className="my-3 overflow-x-auto rounded-lg border border-border/70">
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
      <div className="flex items-center justify-between bg-muted/80 px-3 py-1 rounded-t-lg text-xs text-muted-foreground">
        <span>{language}</span>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 hover:text-foreground transition-colors"
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
