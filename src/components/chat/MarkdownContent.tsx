"use client";

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { memo } from "react";
import type { Components } from "react-markdown";
import { tauriInvoke } from "@/lib/tauri";

/** Check if text looks like an absolute file path. */
function isFilePath(text: string): boolean {
  const trimmed = text.trim();
  // Absolute paths: /Users/..., /home/..., /tmp/..., C:\..., ~/...
  if (!/^(\/|~\/|[A-Z]:\\)/.test(trimmed)) return false;
  // Must have a file extension
  const lastSegment = trimmed.split(/[/\\]/).pop() || "";
  return /\.\w{1,10}$/.test(lastSegment);
}

function handleOpenFile(path: string) {
  tauriInvoke("open_file_with_default_app", { path: path.trim() }).catch((err) => {
    console.error("[MarkdownContent] Failed to open file:", err);
  });
}

const components: Components = {
  // Add language label to code blocks
  pre({ children, ...props }) {
    return <pre {...props}>{children}</pre>;
  },
  code({ className, children, ...props }) {
    const match = /language-(\w+)/.exec(className || "");
    const isInline = !className;

    if (isInline) {
      // Check if the content is a file path
      const text = typeof children === "string" ? children : String(children ?? "");
      if (isFilePath(text)) {
        return (
          <code
            className={`${className || ""} cursor-pointer hover:text-primary hover:underline transition-colors`}
            onClick={() => handleOpenFile(text)}
            title={`Open ${text.trim()}`}
            role="button"
            {...props}
          >
            {children}
          </code>
        );
      }

      return (
        <code className={className} {...props}>
          {children}
        </code>
      );
    }

    return (
      <>
        {match && <span className="code-lang">{match[1]}</span>}
        <code className={className} {...props}>
          {children}
        </code>
      </>
    );
  },
  // Open links in external browser
  a({ href, children, ...props }) {
    return (
      <a href={href} target="_blank" rel="noopener noreferrer" {...props}>
        {children}
      </a>
    );
  },
  // Make tables scrollable
  table({ children, ...props }) {
    return (
      <div className="overflow-x-auto">
        <table {...props}>{children}</table>
      </div>
    );
  },
};

interface MarkdownContentProps {
  content: string;
  className?: string;
}

export const MarkdownContent = memo(function MarkdownContent({
  content,
  className,
}: MarkdownContentProps) {
  return (
    <div
      className={`markdown-body prose prose-sm dark:prose-invert max-w-none break-words ${className || ""}`}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
        components={components}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
});
