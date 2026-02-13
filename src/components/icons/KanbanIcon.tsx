"use client";

import { cn } from "@/lib/cn";

interface KanbanIconProps extends React.HTMLAttributes<SVGSVGElement> {}

/**
 * Custom Kanban board icon (three columns with cards).
 */
export function KanbanIcon({ className, ...rest }: KanbanIconProps) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="currentColor"
      className={cn("inline-block size-[1em]", className)}
      {...rest}
    >
      <rect x="1" y="2" width="4" height="12" rx="1" opacity="0.9" />
      <rect x="6" y="2" width="4" height="8" rx="1" opacity="0.7" />
      <rect x="11" y="2" width="4" height="10" rx="1" opacity="0.5" />
    </svg>
  );
}
