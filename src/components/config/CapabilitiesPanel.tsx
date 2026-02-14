"use client";

import { useState, useMemo, useEffect } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { useOrchestrationStore } from "@/stores/orchestrationStore";
import type { SkillDirEntry } from "@/types/agent";

export function CapabilitiesPanel() {
  const discoveredSkills = useOrchestrationStore((s) => s.discoveredSkills);
  const discoverWorkspaceSkills = useOrchestrationStore((s) => s.discoverWorkspaceSkills);

  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  // Auto-discover workspace skills on mount
  useEffect(() => {
    if (!discoveredSkills) {
      setIsLoading(true);
      discoverWorkspaceSkills().finally(() => setIsLoading(false));
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      await discoverWorkspaceSkills(true);
    } finally {
      setIsRefreshing(false);
    }
  };

  const skills = discoveredSkills?.skills ?? [];
  const projectSkills = useMemo(
    () => skills.filter((e) => e.location === "project"),
    [skills]
  );
  const userSkills = useMemo(
    () => skills.filter((e) => e.location === "user"),
    [skills]
  );

  const renderSkillCard = (entry: SkillDirEntry) => (
    <div
      key={entry.skill.id}
      className="rounded-lg border border-slate-200 dark:border-border-dark/50 bg-slate-50 dark:bg-white/[0.03] p-3"
    >
      <div className="flex items-center gap-2 mb-1.5">
        <span
          className={`px-1.5 py-0.5 rounded text-[9px] font-bold uppercase border ${
            entry.skill.skill_type === "mcp"
              ? "bg-cyan-500/10 text-cyan-400 border-cyan-500/30"
              : entry.skill.skill_type === "tool"
              ? "bg-amber-500/10 text-amber-400 border-amber-500/30"
              : "bg-purple-500/10 text-purple-400 border-purple-500/30"
          }`}
        >
          {entry.skill.skill_type}
        </span>
        <span className="text-xs font-medium text-slate-700 dark:text-gray-300">
          {entry.skill.name}
        </span>
        <span className="text-[10px] font-mono text-slate-400 dark:text-gray-600">
          {entry.skill.id}
        </span>
        {entry.skill.license && (
          <span className="px-1.5 py-0.5 rounded text-[9px] font-medium bg-emerald-500/10 text-emerald-500 border border-emerald-500/20">
            {entry.skill.license}
          </span>
        )}
      </div>
      {entry.skill.description && (
        <p className="text-[11px] text-slate-500 dark:text-gray-500 mb-1.5">
          {entry.skill.description}
        </p>
      )}
      {entry.skill.compatibility && (
        <p className="text-[10px] italic text-slate-400 dark:text-gray-600 mb-1.5">
          Compatibility: {entry.skill.compatibility}
        </p>
      )}
      {entry.skill.task_keywords.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-1">
          {entry.skill.task_keywords.map((kw) => (
            <span
              key={kw}
              className="px-1.5 py-0.5 rounded text-[9px] bg-blue-500/10 text-blue-400 border border-blue-500/20"
            >
              {kw}
            </span>
          ))}
        </div>
      )}
      {entry.skill.constraints.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-1">
          {entry.skill.constraints.map((c) => (
            <span
              key={c}
              className="px-1.5 py-0.5 rounded text-[9px] bg-amber-500/10 text-amber-500 border border-amber-500/20"
            >
              {c}
            </span>
          ))}
        </div>
      )}
      {(entry.has_scripts || entry.has_references || entry.has_assets) && (
        <div className="flex items-center gap-3 mb-1">
          {entry.has_scripts && (
            <span className="flex items-center gap-1 text-[9px] text-slate-400 dark:text-gray-600">
              <Codicon name="terminal" className="text-[10px]" />
              scripts
            </span>
          )}
          {entry.has_references && (
            <span className="flex items-center gap-1 text-[9px] text-slate-400 dark:text-gray-600">
              <Codicon name="book" className="text-[10px]" />
              references
            </span>
          )}
          {entry.has_assets && (
            <span className="flex items-center gap-1 text-[9px] text-slate-400 dark:text-gray-600">
              <Codicon name="file-media" className="text-[10px]" />
              assets
            </span>
          )}
        </div>
      )}
      {entry.skill.metadata && Object.keys(entry.skill.metadata).length > 0 && (
        <div className="flex flex-wrap gap-1 mb-1">
          {Object.entries(entry.skill.metadata).map(([k, v]) => (
            <span
              key={k}
              className="px-1.5 py-0.5 rounded text-[9px] bg-slate-200/60 dark:bg-white/5 text-slate-500 dark:text-gray-500 border border-slate-200 dark:border-border-dark/30"
            >
              {k}: {v}
            </span>
          ))}
        </div>
      )}
      <p className="text-[9px] font-mono text-slate-300 dark:text-gray-700 truncate" title={entry.dir_path}>
        {entry.dir_path}
      </p>
    </div>
  );

  return (
    <div className="flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest">
            Skills
          </h3>
          {skills.length > 0 && (
            <span className="text-[10px] font-mono text-slate-400 dark:text-gray-600 bg-slate-100 dark:bg-white/5 px-1.5 py-0.5 rounded">
              {skills.length}
            </span>
          )}
        </div>
        <button
          onClick={handleRefresh}
          disabled={isRefreshing}
          className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-medium text-slate-500 dark:text-gray-500 hover:text-primary hover:bg-primary/5 border border-transparent hover:border-primary/20 transition-all disabled:opacity-50"
        >
          <Codicon
            name="refresh"
            className={`text-[12px] ${isRefreshing ? "animate-spin" : ""}`}
          />
          {isRefreshing ? "Scanning..." : "Rescan"}
        </button>
      </div>

      {/* Scanned directories */}
      {discoveredSkills && discoveredSkills.scanned_directories.length > 0 && (
        <div className="flex flex-col gap-1">
          {discoveredSkills.scanned_directories.map((dir) => (
            <div key={dir} className="flex items-center gap-1.5 text-[10px] text-slate-400 dark:text-gray-600">
              <Codicon name="folder-opened" className="text-[12px]" />
              <span className="font-mono truncate">{dir}</span>
            </div>
          ))}
        </div>
      )}

      {/* Loading state */}
      {isLoading && !discoveredSkills && (
        <div className="flex items-center justify-center gap-2 py-8 text-xs text-slate-400 dark:text-gray-600">
          <span className="inline-block size-3.5 border-2 border-slate-300 dark:border-gray-600 border-t-primary rounded-full animate-spin" />
          Scanning skills directories...
        </div>
      )}

      {/* Empty state */}
      {!isLoading && skills.length === 0 && (
        <div className="flex flex-col items-center gap-2 py-8 text-center">
          <Codicon name="search" className="text-[24px] text-slate-300 dark:text-gray-700" />
          <p className="text-xs text-slate-400 dark:text-gray-600">
            No skills detected.
          </p>
          <p className="text-[10px] text-slate-400 dark:text-gray-600 max-w-[260px]">
            Create skill directories under <code className="bg-slate-100 dark:bg-white/5 px-1 rounded">skills/</code> in
            your working directory or <code className="bg-slate-100 dark:bg-white/5 px-1 rounded">~/.iaagenthub/skills/</code>.
            Each subdirectory should contain a <code className="bg-slate-100 dark:bg-white/5 px-1 rounded">SKILL.md</code> with YAML frontmatter.
          </p>
        </div>
      )}

      {/* Project Skills */}
      {projectSkills.length > 0 && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <span className="text-[10px] font-bold text-emerald-500 uppercase tracking-widest">
              Project
            </span>
            <span className="text-[10px] font-mono text-slate-400 dark:text-gray-600">
              {projectSkills.length}
            </span>
          </div>
          {projectSkills.map(renderSkillCard)}
        </div>
      )}

      {/* User Skills */}
      {userSkills.length > 0 && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <span className="text-[10px] font-bold text-sky-500 uppercase tracking-widest">
              User
            </span>
            <span className="text-[10px] font-mono text-slate-400 dark:text-gray-600">
              {userSkills.length}
            </span>
          </div>
          {userSkills.map(renderSkillCard)}
        </div>
      )}
    </div>
  );
}
