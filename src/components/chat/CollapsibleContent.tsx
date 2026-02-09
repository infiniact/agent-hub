"use client";

import { useState, useRef, useEffect, type ReactNode } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";

interface CollapsibleContentProps {
  children: ReactNode;
  maxLines?: number;
  className?: string;
}

/**
 * Wraps content and collapses it to a max number of lines.
 * Shows a toggle button to expand/collapse when content overflows.
 */
export function CollapsibleContent({
  children,
  maxLines = 5,
  className,
}: CollapsibleContentProps) {
  const contentRef = useRef<HTMLDivElement>(null);
  const [isOverflowing, setIsOverflowing] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);

  // line-height for text-sm leading-relaxed ≈ 22.75px; use a generous estimate
  const lineHeight = 23;
  const collapsedHeight = maxLines * lineHeight;

  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;

    const check = () => {
      setIsOverflowing(el.scrollHeight > collapsedHeight + 4);
    };

    check();

    // Re-check when images/fonts load or content changes
    const observer = new ResizeObserver(check);
    observer.observe(el);
    return () => observer.disconnect();
  }, [children, collapsedHeight]);

  return (
    <div className={className}>
      <div
        ref={contentRef}
        className="overflow-hidden transition-[max-height] duration-200"
        style={{
          maxHeight: isOverflowing && !isExpanded ? `${collapsedHeight}px` : undefined,
        }}
      >
        {children}
      </div>
      {isOverflowing && (
        <button
          type="button"
          onClick={() => setIsExpanded((v) => !v)}
          className="flex items-center gap-1 mt-1 text-xs text-primary/70 hover:text-primary transition-colors cursor-pointer"
        >
          {isExpanded ? (
            <>
              <ChevronUp className="size-3" />
              <span>收起</span>
            </>
          ) : (
            <>
              <ChevronDown className="size-3" />
              <span>展开更多</span>
            </>
          )}
        </button>
      )}
    </div>
  );
}
