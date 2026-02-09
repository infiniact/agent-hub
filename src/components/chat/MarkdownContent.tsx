"use client";

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { memo, useCallback } from "react";
import type { Components } from "react-markdown";

const components: Components = {
  // Add language label to code blocks
  pre({ children, ...props }) {
    return <pre {...props}>{children}</pre>;
  },
  code({ className, children, ...props }) {
    const match = /language-(\w+)/.exec(className || "");
    const isInline = !className;

    if (isInline) {
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
