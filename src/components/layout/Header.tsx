"use client";

import { IconButton } from "@/components/ui/IconButton";
import { useState, useRef, useEffect } from "react";
import { useSettingsStore } from "@/stores/settingsStore";
import { Codicon } from "@/components/ui/Codicon";
import { RegistryPanel } from "@/components/layout/RegistryPanel";

export function Header() {
  const [userMenuOpen, setUserMenuOpen] = useState(false);
  const [registryOpen, setRegistryOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const registryBtnRef = useRef<HTMLButtonElement>(null);
  const { theme, toggleTheme } = useSettingsStore();

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setUserMenuOpen(false);
      }
    };
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, []);

  return (
    <header className="h-16 flex-none border-b border-slate-200 dark:border-border-dark bg-white dark:bg-[#0A0A10] px-6 flex items-center justify-between z-50">
      <div className="flex items-center gap-3">
        <div className="size-8 rounded-lg bg-primary/20 flex items-center justify-center text-primary">
          <Codicon name="hubot" className="text-[20px]" />
        </div>
        <h1 className="text-xl font-bold tracking-tight text-slate-900 dark:text-white">
          IAAgentHub
        </h1>
      </div>
      <div className="flex items-center gap-2">
        {/* ACP Registry */}
        <div className="relative">
          <IconButton
            ref={registryBtnRef}
            title="ACP Registry"
            onClick={() => setRegistryOpen(!registryOpen)}
          >
            <Codicon name="extensions" className="text-[20px]" />
          </IconButton>
          <RegistryPanel
            open={registryOpen}
            onClose={() => setRegistryOpen(false)}
            anchorRef={registryBtnRef}
          />
        </div>
        <IconButton title="Notifications">
          <Codicon name="bell" className="text-[20px]" />
        </IconButton>
        <IconButton title="Settings">
          <Codicon name="gear" className="text-[20px]" />
        </IconButton>
        <IconButton title="Toggle theme" onClick={toggleTheme}>
          {theme === 'dark' ? <Codicon name="color-mode" className="text-[20px]" /> : <Codicon name="lightbulb" className="text-[20px]" />}
        </IconButton>
        <IconButton title="Language">
          <Codicon name="globe" className="text-[20px]" />
        </IconButton>
        <IconButton title="Captions">
          <Codicon name="output" className="text-[20px]" />
        </IconButton>
        <div className="w-px h-6 bg-slate-200 dark:bg-border-dark mx-1" />
        <div className="relative" ref={menuRef}>
          <button
            onClick={() => setUserMenuOpen(!userMenuOpen)}
            className="size-9 rounded-full bg-slate-100 dark:bg-white/5 border-2 border-slate-300 dark:border-white/10 flex items-center justify-center text-slate-500 dark:text-gray-400 hover:border-primary/50 hover:text-primary transition-all relative"
          >
            <Codicon name="account" className="text-[20px]" />
            <div className="absolute -top-0.5 -right-0.5 size-3 rounded-full border-2 border-white dark:border-[#0A0A10] bg-rose-500 shadow-[0_0_8px_rgba(244,63,94,0.4)]" />
          </button>
          {userMenuOpen && (
            <div className="absolute top-full right-0 mt-2 w-56 bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl shadow-2xl z-[60] py-2">
              <div className="px-4 py-3 border-b border-slate-100 dark:border-border-dark/50 mb-1">
                <p className="text-[10px] font-bold text-rose-500 uppercase tracking-widest mb-1">
                  Account Required
                </p>
                <p className="text-xs text-slate-500 dark:text-gray-400 leading-tight">
                  Sign in to access saved agents and advanced features.
                </p>
              </div>
              <button className="w-full text-center px-4 py-2.5 text-sm font-bold text-white bg-primary/80 hover:bg-primary transition-colors">
                Sign In
              </button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
