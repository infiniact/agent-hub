"use client";

import { useState, useCallback } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { MarkdownContent } from "@/components/chat/MarkdownContent";
import { tauriInvoke } from "@/lib/tauri";

interface GeneratedFileBlockProps {
  path: string;
  content: string;
}

/** Detect if a file path refers to a Markdown file. */
function isMarkdownFile(path: string): boolean {
  const lower = path.toLowerCase();
  return lower.endsWith(".md") || lower.endsWith(".mdx") || lower.endsWith(".markdown");
}

/** Get file extension for code block language hint. */
function getLanguageFromPath(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() || "";
  const map: Record<string, string> = {
    ts: "typescript",
    tsx: "tsx",
    js: "javascript",
    jsx: "jsx",
    py: "python",
    rs: "rust",
    json: "json",
    yaml: "yaml",
    yml: "yaml",
    toml: "toml",
    css: "css",
    html: "html",
    sh: "bash",
    bash: "bash",
    sql: "sql",
    xml: "xml",
    go: "go",
    java: "java",
    kt: "kotlin",
    swift: "swift",
    c: "c",
    cpp: "cpp",
    h: "c",
    hpp: "cpp",
  };
  return map[ext] || ext;
}

/** Get just the file name from a full path. */
function getFileName(path: string): string {
  return path.split("/").pop() || path;
}

export function GeneratedFileBlock({ path, content }: GeneratedFileBlockProps) {
  const [expanded, setExpanded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editContent, setEditContent] = useState(content);
  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "success" | "error">("idle");

  const handleSave = useCallback(async () => {
    setSaving(true);
    setSaveStatus("idle");
    try {
      await tauriInvoke("save_generated_file", { path, content: editContent });
      setSaveStatus("success");
      setEditing(false);
      setTimeout(() => setSaveStatus("idle"), 2000);
    } catch (err) {
      console.error("[GeneratedFileBlock] Save failed:", err);
      setSaveStatus("error");
      setTimeout(() => setSaveStatus("idle"), 3000);
    } finally {
      setSaving(false);
    }
  }, [path, editContent]);

  const handleCancelEdit = useCallback(() => {
    setEditContent(content);
    setEditing(false);
  }, [content]);

  const handleOpenFile = useCallback(() => {
    tauriInvoke("open_file_with_default_app", { path }).catch((err) => {
      console.error("[GeneratedFileBlock] Open file failed:", err);
    });
  }, [path]);

  const isMd = isMarkdownFile(path);
  const lang = getLanguageFromPath(path);
  const fileName = getFileName(path);

  return (
    <div className="bg-slate-50 dark:bg-[#0D0D15] border border-slate-200 dark:border-border-dark rounded-lg overflow-hidden text-xs">
      {/* Collapsed header */}
      <div className="flex items-center gap-2 w-full px-3 py-2 hover:bg-slate-100 dark:hover:bg-white/5 transition-colors">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-2 flex-1 min-w-0 text-left"
        >
          <Codicon name="file" className="text-[14px] text-primary flex-none" />
          <span className="font-mono font-medium text-slate-700 dark:text-gray-300 truncate flex-1" title={path}>
            {fileName}
          </span>
          <span className="text-[10px] text-slate-400 dark:text-gray-500 truncate max-w-[200px] hidden sm:inline" title={path}>
            {path}
          </span>
        </button>
        <button
          onClick={handleOpenFile}
          className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] text-slate-500 dark:text-gray-400 hover:text-primary hover:bg-primary/10 transition-colors flex-none"
          title="Open with default app"
        >
          <Codicon name="link-external" className="text-[12px]" />
        </button>
        {saveStatus === "success" && (
          <Codicon name="pass-filled" className="text-[12px] text-emerald-400 flex-none" />
        )}
        {saveStatus === "error" && (
          <Codicon name="error" className="text-[12px] text-rose-400 flex-none" />
        )}
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex-none p-0.5"
        >
          <Codicon
            name={expanded ? "chevron-down" : "chevron-right"}
            className="text-[12px] text-slate-400"
          />
        </button>
      </div>

      {/* Expanded content */}
      {expanded && (
        <div className="border-t border-slate-200 dark:border-border-dark">
          {/* Toolbar */}
          <div className="flex items-center gap-1 px-3 py-1.5 bg-slate-100/50 dark:bg-white/[0.02]">
            <button
              onClick={() => {
                if (editing) {
                  handleCancelEdit();
                } else {
                  setEditContent(content);
                  setEditing(true);
                }
              }}
              className={`flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium transition-colors ${
                editing
                  ? "text-amber-500 hover:bg-amber-500/10"
                  : "text-slate-500 dark:text-gray-400 hover:bg-slate-200 dark:hover:bg-white/5"
              }`}
            >
              <Codicon name={editing ? "eye" : "edit"} className="text-[12px]" />
              {editing ? "Preview" : "Edit"}
            </button>

            {editing && (
              <>
                <button
                  onClick={handleSave}
                  disabled={saving}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium text-primary hover:bg-primary/10 transition-colors disabled:opacity-50"
                >
                  <Codicon name={saving ? "loading" : "save"} className={`text-[12px] ${saving ? "codicon-modifier-spin" : ""}`} />
                  Save
                </button>
                <button
                  onClick={handleCancelEdit}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium text-slate-400 hover:bg-slate-200 dark:hover:bg-white/5 transition-colors"
                >
                  Cancel
                </button>
              </>
            )}
          </div>

          {/* Content area */}
          <div className="max-h-80 overflow-y-auto">
            {editing ? (
              <textarea
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                className="w-full min-h-[200px] px-3 py-2 bg-white dark:bg-[#0A0A10] text-slate-700 dark:text-gray-300 font-mono text-[11px] leading-relaxed resize-y border-none outline-none focus:ring-0"
                spellCheck={false}
              />
            ) : isMd ? (
              <div className="px-3 py-2">
                <MarkdownContent content={content} className="text-xs" />
              </div>
            ) : (
              <pre className="px-3 py-2 text-slate-600 dark:text-gray-400 font-mono text-[11px] leading-relaxed whitespace-pre-wrap overflow-x-auto">
                <code className={lang ? `language-${lang}` : undefined}>
                  {content}
                </code>
              </pre>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
