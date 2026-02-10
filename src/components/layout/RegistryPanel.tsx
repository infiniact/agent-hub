"use client";

import { useAcpStore } from "@/stores/acpStore";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";
import { useState, useRef, useEffect } from "react";

interface RegistryPanelProps {
  open: boolean;
  onClose: () => void;
  anchorRef: React.RefObject<HTMLElement | null>;
}

type AgentStatus = "installed" | "installable" | "unavailable";

function getAgentStatus(d: {
  available: boolean;
  source_path: string;
}): AgentStatus {
  if (d.available) return "installed";
  // "installable:" prefix means it can be installed on demand
  if (d.source_path.startsWith("installable:")) return "installable";
  return "unavailable";
}

export function RegistryPanel({ open, onClose, anchorRef }: RegistryPanelProps) {
  const discoveredAgents = useAcpStore((s) => s.discoveredAgents);
  const scanning = useAcpStore((s) => s.scanning);
  const scanForAgents = useAcpStore((s) => s.scanForAgents);
  const installAgent = useAcpStore((s) => s.installAgent);
  const uninstallAgent = useAcpStore((s) => s.uninstallAgent);
  const [busyId, setBusyId] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  // Only registry agents
  const registryAgents = discoveredAgents.filter((d) => d.registry_id);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (
        panelRef.current &&
        !panelRef.current.contains(e.target as Node) &&
        anchorRef.current &&
        !anchorRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open, onClose, anchorRef]);

  if (!open) return null;

  const handleInstall = async (registryId: string) => {
    if (busyId) return;
    setBusyId(registryId);
    try {
      await installAgent(registryId);
    } catch (e) {
      console.error("[RegistryPanel] install error:", e);
    } finally {
      setBusyId(null);
    }
  };

  const handleUninstall = async (registryId: string) => {
    if (busyId) return;
    setBusyId(registryId);
    try {
      await uninstallAgent(registryId);
    } catch (e) {
      console.error("[RegistryPanel] uninstall error:", e);
    } finally {
      setBusyId(null);
    }
  };

  const handleRefresh = async (registryId: string) => {
    if (busyId) return;
    setBusyId(registryId);
    try {
      await uninstallAgent(registryId);
      await installAgent(registryId);
    } catch (e) {
      console.error("[RegistryPanel] refresh error:", e);
    } finally {
      setBusyId(null);
    }
  };

  const installedCount = registryAgents.filter(
    (d) => getAgentStatus(d) === "installed"
  ).length;

  return (
    <div
      ref={panelRef}
      className="absolute top-full right-0 mt-2 w-[380px] max-h-[480px] bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl shadow-2xl z-[60] flex flex-col overflow-hidden"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-slate-100 dark:border-border-dark/50">
        <div className="flex items-center gap-2">
          <Codicon name="extensions" className="text-[16px] text-primary" />
          <span className="text-sm font-semibold text-slate-800 dark:text-white">
            ACP Registry
          </span>
          <span className="text-[10px] font-medium text-slate-400 dark:text-gray-500 bg-slate-100 dark:bg-white/5 rounded px-1.5 py-0.5">
            {installedCount}/{registryAgents.length}
          </span>
        </div>
        <button
          onClick={() => scanForAgents()}
          disabled={scanning}
          title="Refresh registry"
          className="size-7 rounded-md hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-500 hover:text-primary transition-colors disabled:opacity-40"
        >
          <Codicon
            name="refresh"
            className={cn("text-[14px]", scanning && "animate-spin")}
          />
        </button>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto py-1">
        {scanning && registryAgents.length === 0 && (
          <div className="flex items-center justify-center gap-2 py-8 text-xs text-gray-500">
            <Codicon name="loading" className="text-[16px] animate-spin" />
            Scanning registry...
          </div>
        )}

        {registryAgents.map((d) => {
          const status = getAgentStatus(d);
          const isBusy = busyId === d.registry_id;

          return (
            <div
              key={d.id}
              className="flex items-center gap-3 px-4 py-2.5 hover:bg-slate-50 dark:hover:bg-white/[0.02] transition-colors"
            >
              {/* Icon */}
              <div className="size-8 rounded-lg bg-slate-100 dark:bg-white/5 flex items-center justify-center shrink-0 overflow-hidden">
                {d.icon_url ? (
                  <img
                    src={d.icon_url}
                    alt=""
                    className="size-5 object-contain"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = "none";
                      (e.target as HTMLImageElement).parentElement!.innerHTML =
                        '<i class="codicon codicon-code text-[16px] text-slate-400"></i>';
                    }}
                  />
                ) : (
                  <Codicon
                    name="code"
                    className="text-[16px] text-slate-400 dark:text-gray-500"
                  />
                )}
              </div>

              {/* Info */}
              <div className="flex-1 min-w-0">
                <span className="text-[13px] font-medium text-slate-800 dark:text-gray-200 truncate block">
                  {d.name}
                </span>
                <p className="text-[11px] text-slate-400 dark:text-gray-500 truncate leading-tight mt-0.5">
                  {d.description || d.registry_id}
                </p>
              </div>

              {/* Status + Actions */}
              <div className="flex items-center gap-1.5 shrink-0">
                {isBusy ? (
                  <span className="flex items-center gap-1 text-[11px] text-primary">
                    <Codicon
                      name="loading"
                      className="text-[14px] animate-spin"
                    />
                  </span>
                ) : status === "installed" ? (
                  <>
                    <span className="flex items-center gap-1 text-[11px] font-medium text-emerald-500 bg-emerald-500/10 rounded px-1.5 py-0.5">
                      <span className="size-1.5 rounded-full bg-emerald-400 shadow-[0_0_4px_rgba(52,211,153,0.5)]" />
                      Installed
                    </span>
                    <button
                      onClick={() => handleRefresh(d.registry_id!)}
                      title="Reinstall"
                      className="size-6 rounded hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-500 hover:text-primary transition-colors"
                    >
                      <Codicon name="refresh" className="text-[13px]" />
                    </button>
                    <button
                      onClick={() => handleUninstall(d.registry_id!)}
                      title="Uninstall"
                      className="size-6 rounded hover:bg-rose-50 dark:hover:bg-rose-500/10 flex items-center justify-center text-slate-400 dark:text-gray-500 hover:text-rose-500 transition-colors"
                    >
                      <Codicon name="trash" className="text-[13px]" />
                    </button>
                  </>
                ) : status === "installable" ? (
                  <button
                    onClick={() => handleInstall(d.registry_id!)}
                    className="flex items-center gap-1 text-[11px] font-medium text-primary bg-primary/10 hover:bg-primary/20 rounded px-2 py-1 transition-colors"
                  >
                    <Codicon name="cloud-download" className="text-[13px]" />
                    Install
                  </button>
                ) : (
                  <span className="text-[11px] font-medium text-gray-400 dark:text-gray-600 bg-slate-100 dark:bg-white/5 rounded px-1.5 py-0.5">
                    N/A
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
