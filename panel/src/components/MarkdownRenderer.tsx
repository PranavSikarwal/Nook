import { Check, Copy } from 'lucide-react'
import { useState } from 'react'
import ReactMarkdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'

interface MarkdownRendererProps {
  readonly content: string
}

interface CodeBlockProps {
  readonly code: string
}

function CodeBlock({ code }: CodeBlockProps) {
  const [copied, setCopied] = useState(false)

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // Fallback if clipboard API is restricted
    }
  }

  return (
    <div className="relative group my-3 rounded-lg overflow-hidden border border-white/10 bg-black/40">
      <div className="flex items-center justify-between px-3 py-1.5 bg-white/5 border-b border-white/10 text-xs text-zinc-400">
        <span className="font-mono text-[11px] uppercase tracking-wider">code</span>
        <button
          onClick={handleCopy}
          type="button"
          className="flex items-center gap-1 hover:text-white transition-colors px-1.5 py-0.5 rounded text-[11px] bg-white/5 hover:bg-white/10"
        >
          {copied ? (
            <>
              <Check className="size-3 text-emerald-400" />
              <span className="text-emerald-400">Copied</span>
            </>
          ) : (
            <>
              <Copy className="size-3" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>
      <pre className="p-3 text-[13px] font-mono leading-relaxed overflow-x-auto text-zinc-200">
        <code>{code}</code>
      </pre>
    </div>
  )
}

const markdownComponents: Components = {
  code({ className, children, ...props }) {
    const isInline = !className && !String(children).includes('\n')
    if (isInline) {
      return (
        <code
          className="bg-white/10 text-purple-300 px-1.5 py-0.5 rounded text-[13px] font-mono"
          {...props}
        >
          {children}
        </code>
      )
    }
    return <CodeBlock code={String(children).replace(/\n$/, '')} />
  },
  p({ children }) {
    return <p className="mb-2 last:mb-0 leading-normal">{children}</p>
  },
  ul({ children }) {
    return <ul className="list-disc pl-5 my-2 space-y-1">{children}</ul>
  },
  ol({ children }) {
    return <ol className="list-decimal pl-5 my-2 space-y-1">{children}</ol>
  },
  table({ children }) {
    return (
      <div className="overflow-x-auto my-3">
        <table className="min-w-full text-left border-collapse text-xs border border-white/10">
          {children}
        </table>
      </div>
    )
  },
  th({ children }) {
    return (
      <th className="border border-white/10 bg-white/5 px-2.5 py-1.5 font-medium text-zinc-300">
        {children}
      </th>
    )
  },
  td({ children }) {
    return (
      <td className="border border-white/10 px-2.5 py-1 text-zinc-300">
        {children}
      </td>
    )
  },
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  return (
    <div className="prose prose-invert prose-sm max-w-none text-zinc-200 leading-relaxed space-y-2">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
        {content}
      </ReactMarkdown>
    </div>
  )
}
