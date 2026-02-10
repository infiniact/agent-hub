import type { SVGProps } from "react";

/**
 * MCP (Model Context Protocol) logo icon.
 * Interlinked chain-link / braid pattern matching the official MCP visual.
 */
export function McpIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      {/* Upper-left loop: goes over at the top crossing, under at the bottom crossing */}
      <path d="M 8 3 C 5 3 3.5 5 3.5 7.5 C 3.5 10 5 11.5 7 12" />
      <path d="M 11 13 C 13 13.5 14.5 12 14.5 9.5 C 14.5 7 13 5 10 3.5" />

      {/* Lower-right loop: goes over at the bottom crossing, under at the top crossing */}
      <path d="M 16 21 C 19 21 20.5 19 20.5 16.5 C 20.5 14 19 12.5 17 12" />
      <path d="M 13 11 C 11 10.5 9.5 12 9.5 14.5 C 9.5 17 11 19 14 20.5" />
    </svg>
  );
}
