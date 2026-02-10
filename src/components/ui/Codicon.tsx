"use client";

import { cn } from "@/lib/cn";
import { McpIcon } from "@/components/icons/McpIcon";

interface CodiconProps extends React.HTMLAttributes<HTMLElement> {
  /** Codicon icon name, e.g. "add", "search", "close" */
  name: string;
}

/**
 * Renders a VS Code codicon.
 * Default size is 16px (inherited from codicon CSS).
 * Override with `text-[20px]` etc. for larger/smaller icons.
 *
 * Custom icons (not in the codicon font) are rendered as inline SVGs:
 *   - "mcp" → MCP protocol icon
 */
export function Codicon({ name, className, ...rest }: CodiconProps) {
  if (name === "mcp") {
    return <McpIcon className={cn("inline-block size-[1em]", className)} />;
  }
  return <i className={cn("codicon", `codicon-${name}`, className)} {...rest} />;
}
