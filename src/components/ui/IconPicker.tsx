"use client";

import { useState, useRef, useMemo } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";

// ---------------------------------------------------------------------------
// Full list of codicon names (526 icons from @vscode/codicons)
// ---------------------------------------------------------------------------

const CODICON_NAMES: string[] = [
  "account","activate-breakpoints","add","agent","archive","arrow-both","arrow-circle-down",
  "arrow-circle-left","arrow-circle-right","arrow-circle-up","arrow-down","arrow-left",
  "arrow-right","arrow-small-down","arrow-small-left","arrow-small-right","arrow-small-up",
  "arrow-swap","arrow-up","attach","azure","azure-devops","beaker","beaker-stop","bell",
  "bell-dot","bell-slash","bell-slash-dot","blank","bold","book","bookmark","bracket-dot",
  "bracket-error","briefcase","broadcast","browser","bug","build","calendar","call-incoming",
  "call-outgoing","case-sensitive","chat-sparkle","chat-sparkle-error","chat-sparkle-warning",
  "check","check-all","checklist","chevron-down","chevron-left","chevron-right","chevron-up",
  "chip","chrome-close","chrome-maximize","chrome-minimize","chrome-restore","circle",
  "circle-filled","circle-large","circle-large-filled","circle-slash","circle-small",
  "circle-small-filled","circuit-board","clear-all","clippy","clockface","close","close-all",
  "cloud","cloud-download","cloud-small","cloud-upload","code","code-oss","code-review",
  "coffee","collapse-all","collection","collection-small","color-mode","combine","comment",
  "comment-discussion","comment-discussion-quote","comment-discussion-sparkle","comment-draft",
  "comment-unresolved","compass","compass-active","compass-dot","copilot","copilot-blocked",
  "copilot-error","copilot-in-progress","copilot-large","copilot-not-connected","copilot-snooze",
  "copilot-success","copilot-unavailable","copilot-warning","copilot-warning-large","copy",
  "coverage","credit-card","cursor","dash","dashboard","database","debug","debug-all",
  "debug-alt","debug-alt-small","debug-breakpoint-conditional",
  "debug-breakpoint-conditional-unverified","debug-breakpoint-data",
  "debug-breakpoint-data-unverified","debug-breakpoint-function",
  "debug-breakpoint-function-unverified","debug-breakpoint-log",
  "debug-breakpoint-log-unverified","debug-breakpoint-unsupported","debug-connected",
  "debug-console","debug-continue","debug-continue-small","debug-coverage","debug-disconnect",
  "debug-line-by-line","debug-pause","debug-rerun","debug-restart","debug-restart-frame",
  "debug-reverse-continue","debug-stackframe","debug-stackframe-active","debug-start",
  "debug-step-back","debug-step-into","debug-step-out","debug-step-over","debug-stop",
  "desktop-download","device-camera","device-camera-video","device-mobile","diff","diff-added",
  "diff-ignored","diff-modified","diff-multiple","diff-removed","diff-renamed","diff-single",
  "discard","download","edit","edit-code","edit-session","edit-sparkle","editor-layout",
  "ellipsis","empty-window","eraser","error","error-small","exclude","expand-all","export",
  "extensions","extensions-large","eye","eye-closed","feedback","file","file-binary","file-code",
  "file-media","file-pdf","file-submodule","file-symlink-directory","file-symlink-file",
  "file-text","file-zip","files","filter","filter-filled","flag","flame","fold","fold-down",
  "fold-up","folder","folder-active","folder-library","folder-opened","forward","game","gear",
  "gift","gist","gist-secret","git-branch","git-branch-changes","git-branch-conflicts",
  "git-branch-staged-changes","git-commit","git-compare","git-fetch","git-merge",
  "git-pull-request","git-pull-request-closed","git-pull-request-create",
  "git-pull-request-done","git-pull-request-draft","git-pull-request-go-to-changes",
  "git-pull-request-new-changes","git-stash","git-stash-apply","git-stash-pop","github",
  "github-action","github-alt","github-inverted","github-project","globe",
  "go-to-editing-session","go-to-file","go-to-search","grabber","graph","graph-left",
  "graph-line","graph-scatter","gripper","group-by-ref-type","heart","heart-filled","history",
  "home","horizontal-rule","hubot","inbox","indent","index-zero","info","insert","inspect",
  "issue-draft","issue-reopened","issues","italic","jersey","json","kebab-vertical","key",
  "keyboard-tab","keyboard-tab-above","keyboard-tab-below","law","layers","layers-active",
  "layers-dot","layout","layout-activitybar-left","layout-activitybar-right","layout-centered",
  "layout-menubar","layout-panel","layout-panel-center","layout-panel-dock",
  "layout-panel-justify","layout-panel-left","layout-panel-off","layout-panel-right",
  "layout-sidebar-left","layout-sidebar-left-dock","layout-sidebar-left-off",
  "layout-sidebar-right","layout-sidebar-right-dock","layout-sidebar-right-off",
  "layout-statusbar","library","lightbulb","lightbulb-autofix","lightbulb-empty",
  "lightbulb-sparkle","link","link-external","list-filter","list-flat","list-ordered",
  "list-selection","list-tree","list-unordered","live-share","loading","location","lock",
  "lock-small","magnet","mail","mail-read","map","map-filled","map-vertical",
  "map-vertical-filled","markdown","megaphone","mention","menu","merge","merge-into",
  "mic","mic-filled","milestone","mirror","mortar-board","move","multiple-windows","music",
  "mute","new-collection","new-file","new-folder","newline","no-newline","note","notebook",
  "notebook-template","octoface","open-in-product","open-preview","organization","output",
  "package","paintcan","pass","pass-filled","percentage","person","person-add","piano",
  "pie-chart","pin","pinned","pinned-dirty","play","play-circle","plug","preserve-case",
  "preview","primitive-square","project","pulse","python","question","quote","quotes",
  "radio-tower","reactions","record","record-keys","record-small","redo","references","refresh",
  "regex","remote","remote-explorer","remove","rename","replace","replace-all","reply","repo",
  "repo-clone","repo-fetch","repo-force-push","repo-forked","repo-pinned","repo-pull",
  "repo-push","repo-selected","report","robot","rocket","root-folder","root-folder-opened",
  "rss","ruby","run-above","run-all","run-all-coverage","run-below","run-coverage","run-errors",
  "run-with-deps","save","save-all","save-as","screen-full","screen-normal","search",
  "search-fuzzy","search-large","search-sparkle","search-stop","send","send-to-remote-agent",
  "server","server-environment","server-process","session-in-progress","settings","settings-gear",
  "share","shield","sign-in","sign-out","skip","smiley","snake","sort-precedence","sparkle",
  "sparkle-filled","split-horizontal","split-vertical","squirrel","star-empty","star-full",
  "star-half","stop-circle","strikethrough","surround-with","symbol-array","symbol-boolean",
  "symbol-class","symbol-color","symbol-constant","symbol-enum","symbol-enum-member",
  "symbol-event","symbol-field","symbol-interface","symbol-key","symbol-keyword","symbol-method",
  "symbol-method-arrow","symbol-misc","symbol-numeric","symbol-operator","symbol-parameter",
  "symbol-property","symbol-ruler","symbol-snippet","symbol-structure","symbol-variable","sync",
  "sync-ignored","table","tag","target","tasklist","telescope","terminal","terminal-bash",
  "terminal-cmd","terminal-debian","terminal-git-bash","terminal-linux","terminal-powershell",
  "terminal-tmux","terminal-ubuntu","text-size","thinking","three-bars","thumbsdown",
  "thumbsdown-filled","thumbsup","thumbsup-filled","tools","trash","triangle-down",
  "triangle-left","triangle-right","triangle-up","twitter","type-hierarchy","type-hierarchy-sub",
  "type-hierarchy-super","unarchive","unfold","ungroup-by-ref-type","unlock","unmute",
  "unverified","variable-group","verified","verified-filled","vm","vm-active","vm-connect",
  "vm-outline","vm-pending","vm-running","vm-small","vr","vscode","vscode-insiders","wand",
  "warning","watch","whitespace","whole-word","window-active","word-wrap","workspace-trusted",
  "workspace-unknown","workspace-untrusted","zoom-in","zoom-out",
];

// Popular icons shown by default without searching
const POPULAR_ICONS = [
  "folder","code","rocket","terminal","beaker","globe","heart","star-full",
  "home","bookmark","github","cloud","database","gear","play","bug",
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Resize an image file to 64x64 PNG data URL via canvas */
function resizeImageToDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const img = new Image();
      img.onload = () => {
        const canvas = document.createElement("canvas");
        canvas.width = 64;
        canvas.height = 64;
        const ctx = canvas.getContext("2d");
        if (!ctx) return reject(new Error("Canvas context unavailable"));
        ctx.drawImage(img, 0, 0, 64, 64);
        resolve(canvas.toDataURL("image/png"));
      };
      img.onerror = reject;
      img.src = reader.result as string;
    };
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface IconPickerProps {
  value: string;
  onChange: (icon: string) => void;
}

export function IconPicker({ value, onChange }: IconPickerProps) {
  const [search, setSearch] = useState("");
  const fileInputRef = useRef<HTMLInputElement>(null);

  const isImageValue = value.startsWith("data:");

  const searchResults = useMemo(() => {
    if (!search.trim()) return [];
    const q = search.toLowerCase();
    return CODICON_NAMES.filter((name) => name.includes(q));
  }, [search]);

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const dataUrl = await resizeImageToDataUrl(file);
      onChange(dataUrl);
    } catch (err) {
      console.error("[IconPicker] Failed to process image:", err);
    }
    // Reset input so re-selecting the same file triggers onChange
    e.target.value = "";
  };

  return (
    <div className="space-y-2">
      {/* Popular icons */}
      <div className="flex flex-wrap gap-1.5">
        {POPULAR_ICONS.map((name) => (
          <button
            key={name}
            type="button"
            onClick={() => onChange(name)}
            className={cn(
              "size-8 rounded-lg flex items-center justify-center transition-all",
              value === name
                ? "bg-primary text-background-dark shadow-[0_0_8px_rgba(0,229,255,0.2)]"
                : "bg-slate-100 dark:bg-white/5 text-slate-400 dark:text-gray-500 hover:text-primary hover:border-primary/50 border border-transparent"
            )}
            title={name}
          >
            <Codicon name={name} className="text-[14px]" />
          </button>
        ))}

        {/* Upload button */}
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          className={cn(
            "size-8 rounded-lg flex items-center justify-center transition-all",
            isImageValue
              ? "bg-primary text-background-dark shadow-[0_0_8px_rgba(0,229,255,0.2)]"
              : "bg-slate-100 dark:bg-white/5 text-slate-400 dark:text-gray-500 hover:text-primary hover:border-primary/50 border border-transparent"
          )}
          title="Upload custom image"
        >
          {isImageValue ? (
            <img
              src={value}
              alt="custom"
              className="size-5 rounded object-cover"
            />
          ) : (
            <Codicon name="device-camera" className="text-[14px]" />
          )}
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          className="hidden"
          onChange={handleUpload}
        />
      </div>

      {/* Search */}
      <div className="relative">
        <Codicon
          name="search"
          className="absolute left-2 top-1/2 -translate-y-1/2 text-[12px] text-slate-400 dark:text-gray-600"
        />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search 526 icons..."
          className="w-full pl-7 pr-3 py-1.5 rounded-lg bg-slate-50 dark:bg-white/5 border border-slate-200 dark:border-border-dark text-xs text-slate-900 dark:text-white placeholder-slate-400 dark:placeholder-gray-600 focus:outline-none focus:border-primary"
        />
      </div>

      {/* Search results grid */}
      {search.trim() && (
        <div className="max-h-36 overflow-y-auto">
          {searchResults.length === 0 ? (
            <p className="text-[10px] text-slate-400 dark:text-gray-600 py-2 text-center">
              No icons found
            </p>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {searchResults.map((name) => (
                <button
                  key={name}
                  type="button"
                  onClick={() => {
                    onChange(name);
                    setSearch("");
                  }}
                  className={cn(
                    "size-8 rounded-lg flex items-center justify-center transition-all",
                    value === name
                      ? "bg-primary text-background-dark shadow-[0_0_8px_rgba(0,229,255,0.2)]"
                      : "bg-slate-100 dark:bg-white/5 text-slate-400 dark:text-gray-500 hover:text-primary hover:border-primary/50 border border-transparent"
                  )}
                  title={name}
                >
                  <Codicon name={name} className="text-[14px]" />
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
