"use client";

import { cn } from "@/lib/cn";
import { ButtonHTMLAttributes, forwardRef } from "react";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = "secondary", size = "md", ...props }, ref) => {
    return (
      <button
        ref={ref}
        className={cn(
          "inline-flex items-center justify-center gap-2 font-bold transition-colors rounded-lg",
          variant === "primary" &&
            "bg-primary hover:bg-cyan-400 text-background-dark shadow-lg shadow-primary/10",
          variant === "secondary" &&
            "bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark text-slate-600 dark:text-gray-300 hover:text-slate-900 dark:hover:text-white hover:border-slate-400 dark:hover:border-gray-600",
          variant === "ghost" &&
            "hover:bg-slate-100 dark:hover:bg-white/5 text-slate-500 dark:text-gray-400 hover:text-slate-900 dark:hover:text-white",
          size === "sm" && "h-8 px-3 text-xs",
          size === "md" && "h-9 px-4 text-sm",
          size === "lg" && "h-10 px-5 text-sm",
          className
        )}
        {...props}
      />
    );
  }
);
Button.displayName = "Button";
