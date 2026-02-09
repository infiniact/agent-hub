"use client";

import { cn } from "@/lib/cn";
import { ButtonHTMLAttributes, forwardRef } from "react";

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  size?: "sm" | "md";
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ className, size = "md", ...props }, ref) => {
    return (
      <button
        ref={ref}
        className={cn(
          "rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-500 dark:text-gray-400 hover:text-slate-900 dark:hover:text-white transition-colors",
          size === "sm" && "size-8",
          size === "md" && "size-9",
          className
        )}
        {...props}
      />
    );
  }
);
IconButton.displayName = "IconButton";
