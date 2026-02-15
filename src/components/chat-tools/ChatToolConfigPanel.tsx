"use client";

import { useState, useEffect } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/cn";
import { useChatToolStore } from "@/stores/chatToolStore";
import type { ChatTool, ChatToolMessage, ChatToolContact } from "@/types/chatTool";

interface Props {
  tool: ChatTool;
}

type Tab = "config" | "messages" | "contacts";

export function ChatToolConfigPanel({ tool }: Props) {
  const [activeTab, setActiveTab] = useState<Tab>("config");
  const startChatTool = useChatToolStore((s) => s.startChatTool);
  const stopChatTool = useChatToolStore((s) => s.stopChatTool);
  const logoutChatTool = useChatToolStore((s) => s.logoutChatTool);
  const updateChatTool = useChatToolStore((s) => s.updateChatTool);
  const messages = useChatToolStore((s) => s.messages);
  const contacts = useChatToolStore((s) => s.contacts);
  const qrCodeImage = useChatToolStore((s) => s.qrCodeImage);
  const qrCodeUrl = useChatToolStore((s) => s.qrCodeUrl);
  const setContactBlocked = useChatToolStore((s) => s.setContactBlocked);

  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [switchingAccount, setSwitchingAccount] = useState(false);

  const isRunning = tool.status === "running";
  const isStarting = tool.status === "starting" || tool.status === "waiting_for_login";
  const needsLogin = tool.status === "login_required";
  const isError = tool.status === "error";

  // Auto-refresh chat tool list every 30s while tool is running (to keep last_active_at fresh)
  const fetchChatTools = useChatToolStore((s) => s.fetchChatTools);
  useEffect(() => {
    if (!isRunning) return;
    const interval = setInterval(() => {
      fetchChatTools();
    }, 30000);
    return () => clearInterval(interval);
  }, [isRunning, fetchChatTools]);

  // Fetch QR code from backend cache when tool enters login-required state
  const getQrCode = useChatToolStore((s) => s.getQrCode);
  useEffect(() => {
    if (needsLogin || isStarting) {
      getQrCode(tool.id);
    }
  }, [tool.id, tool.status, needsLogin, isStarting, getQrCode]);

  const handleStart = async () => {
    setStarting(true);
    try {
      await startChatTool(tool.id);
    } catch (e) {
      console.error("[ChatToolConfig] Start failed:", e);
    }
    setStarting(false);
  };

  const handleStop = async () => {
    setStopping(true);
    try {
      await stopChatTool(tool.id);
    } catch (e) {
      console.error("[ChatToolConfig] Stop failed:", e);
    }
    setStopping(false);
  };

  const handleSwitchAccount = async () => {
    setSwitchingAccount(true);
    try {
      await logoutChatTool(tool.id);
    } catch (e) {
      console.error("[ChatToolConfig] Logout failed:", e);
    }
    setSwitchingAccount(false);
  };

  return (
    <div className="p-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <div
            className={cn(
              "size-10 rounded-xl flex items-center justify-center",
              isRunning
                ? "bg-emerald-500/10 text-emerald-500"
                : isError
                  ? "bg-rose-500/10 text-rose-500"
                  : "bg-slate-100 dark:bg-surface-dark text-slate-400"
            )}
          >
            <Codicon name="comment-discussion" className="text-[20px]" />
          </div>
          <div>
            <h2 className="text-base font-bold text-slate-800 dark:text-white">
              {tool.name}
            </h2>
            <div className="flex items-center gap-2 text-xs text-slate-400 dark:text-gray-500">
              <span className="capitalize">{tool.plugin_type}</span>
              {tool.status_message && (
                <>
                  <span className="text-slate-300 dark:text-gray-600">|</span>
                  <span>{tool.status_message}</span>
                </>
              )}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {(isRunning || isStarting || needsLogin) ? (
            <>
              {isRunning && (
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={handleSwitchAccount}
                  disabled={switchingAccount}
                >
                  <Codicon name="person-add" className="text-[14px] text-amber-500" />
                  {switchingAccount ? "Logging out..." : "Switch Account"}
                </Button>
              )}
              <Button
                variant="secondary"
                size="sm"
                onClick={handleStop}
                disabled={stopping}
              >
                <Codicon name="debug-stop" className="text-[14px] text-rose-500" />
                {stopping ? "Stopping..." : "Stop"}
              </Button>
            </>
          ) : (
            <Button
              variant="primary"
              size="sm"
              onClick={handleStart}
              disabled={starting}
            >
              <Codicon name="play" className="text-[14px]" />
              {starting ? "Starting..." : "Start"}
            </Button>
          )}
        </div>
      </div>

      {/* QR Code Login */}
      {(needsLogin || isStarting) && (qrCodeImage || qrCodeUrl) && (
        <div className="mb-6 p-5 rounded-xl border border-amber-200 dark:border-amber-500/30 bg-amber-50 dark:bg-amber-500/5">
          <div className="flex items-center gap-2 mb-4">
            <Codicon name="device-mobile" className="text-amber-500" />
            <span className="text-sm font-semibold text-amber-700 dark:text-amber-400">
              Scan QR Code with WeChat to Login
            </span>
          </div>
          <div className="flex justify-center">
            {qrCodeImage ? (
              <img
                src={
                  qrCodeImage.startsWith("data:")
                    ? qrCodeImage
                    : `data:image/png;base64,${qrCodeImage}`
                }
                alt="QR Code"
                className="w-56 h-56 rounded-lg bg-white p-2"
              />
            ) : qrCodeUrl ? (
              <img
                src={qrCodeUrl}
                alt="QR Code"
                className="w-56 h-56 rounded-lg bg-white p-2"
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = 'none';
                }}
              />
            ) : null}
          </div>
        </div>
      )}

      {/* Error display */}
      {isError && tool.status_message && (
        <div className="mb-6 p-3 rounded-lg border border-rose-200 dark:border-rose-500/30 bg-rose-50 dark:bg-rose-500/5">
          <div className="flex items-center gap-2">
            <Codicon name="error" className="text-rose-500" />
            <span className="text-xs text-rose-600 dark:text-rose-400">
              {tool.status_message}
            </span>
          </div>
        </div>
      )}

      {/* Stats */}
      <div className="grid grid-cols-3 gap-3 mb-6">
        <StatCard
          label="Received"
          value={tool.messages_received}
          icon="arrow-down"
          color="text-blue-500"
        />
        <StatCard
          label="Sent"
          value={tool.messages_sent}
          icon="arrow-up"
          color="text-emerald-500"
        />
        <StatCard
          label="Last Active"
          value={tool.last_active_at ? formatRelativeTime(tool.last_active_at) : "Never"}
          icon="clock"
          color="text-slate-400"
          isText
          healthStatus={isRunning ? getHealthStatus(tool.last_active_at) : undefined}
        />
      </div>

      {/* Tabs */}
      <div className="flex border-b border-slate-200 dark:border-border-dark mb-4">
        {(["config", "messages", "contacts"] as Tab[]).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={cn(
              "px-4 py-2 text-xs font-semibold capitalize transition-colors border-b-2 -mb-px",
              activeTab === tab
                ? "border-primary text-primary"
                : "border-transparent text-slate-400 dark:text-gray-500 hover:text-slate-600 dark:hover:text-gray-300"
            )}
          >
            {tab}
          </button>
        ))}
      </div>

      {/* Tab content */}
      {activeTab === "config" && (
        <ConfigTab tool={tool} onUpdate={updateChatTool} />
      )}
      {activeTab === "messages" && <MessageLog messages={messages} />}
      {activeTab === "contacts" && (
        <ContactList contacts={contacts} onToggleBlocked={setContactBlocked} />
      )}
    </div>
  );
}

function StatCard({
  label,
  value,
  icon,
  color,
  isText,
  healthStatus,
}: {
  label: string;
  value: number | string;
  icon: string;
  color: string;
  isText?: boolean;
  healthStatus?: "healthy" | "delayed" | "disconnected";
}) {
  const healthDotColor =
    healthStatus === "healthy"
      ? "bg-emerald-500"
      : healthStatus === "delayed"
        ? "bg-amber-500"
        : healthStatus === "disconnected"
          ? "bg-rose-500"
          : null;

  return (
    <div className="p-3 rounded-lg border border-slate-200 dark:border-border-dark bg-slate-50 dark:bg-surface-dark">
      <div className="flex items-center gap-1.5 mb-1">
        <Codicon name={icon} className={cn("text-[12px]", color)} />
        <span className="text-[10px] font-medium text-slate-400 dark:text-gray-500 uppercase">
          {label}
        </span>
        {healthDotColor && (
          <span
            className={cn("size-2 rounded-full ml-auto shrink-0", healthDotColor)}
            title={healthStatus}
          />
        )}
      </div>
      <div
        className={cn(
          "font-bold text-slate-700 dark:text-gray-200",
          isText ? "text-xs" : "text-lg"
        )}
      >
        {value}
      </div>
    </div>
  );
}

function ConfigTab({
  tool,
  onUpdate,
}: {
  tool: ChatTool;
  onUpdate: (id: string, req: any) => Promise<any>;
}) {
  return (
    <div className="space-y-4">
      {/* Message Routing */}
      <div className="p-3 rounded-lg border border-blue-200 dark:border-blue-500/30 bg-blue-50 dark:bg-blue-500/5">
        <div className="flex items-center gap-2">
          <Codicon name="rocket" className="text-blue-500 text-[14px]" />
          <span className="text-xs font-semibold text-blue-700 dark:text-blue-400">
            Messages routed via Control Hub
          </span>
        </div>
        <p className="text-[10px] text-blue-600 dark:text-blue-400/70 mt-1">
          Incoming messages are automatically forwarded to the workspace Control Hub agent for processing.
        </p>
      </div>

      {/* Auto Reply Mode */}
      <div>
        <label className="block text-xs font-semibold text-slate-600 dark:text-gray-300 mb-1">
          Auto Reply Mode
        </label>
        <select
          value={tool.auto_reply_mode}
          onChange={(e) => onUpdate(tool.id, { auto_reply_mode: e.target.value })}
          className="w-full h-9 px-3 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-800 dark:text-white focus:outline-none focus:border-primary"
        >
          <option value="all">All messages</option>
          <option value="contacts_only">Contacts only</option>
          <option value="none">Disabled</option>
        </select>
      </div>

      {/* Info */}
      <div className="pt-2 space-y-1 text-[10px] text-slate-400 dark:text-gray-500">
        <div>
          <span className="font-semibold">Plugin:</span> {tool.plugin_type}
        </div>
        <div>
          <span className="font-semibold">Created:</span> {tool.created_at}
        </div>
        <div>
          <span className="font-semibold">ID:</span> {tool.id}
        </div>
      </div>
    </div>
  );
}

function MessageLog({ messages }: { messages: ChatToolMessage[] }) {
  if (messages.length === 0) {
    return (
      <div className="text-center py-12 text-slate-400 dark:text-gray-500">
        <Codicon name="mail" className="text-2xl mb-2 block" />
        <p className="text-xs">No messages yet</p>
      </div>
    );
  }

  return (
    <div className="space-y-2 max-h-[400px] overflow-y-auto">
      {messages.map((msg) => (
        <div
          key={msg.id}
          className={cn(
            "p-3 rounded-lg text-xs",
            msg.direction === "incoming"
              ? "bg-blue-50 dark:bg-blue-500/5 border border-blue-100 dark:border-blue-500/20"
              : "bg-emerald-50 dark:bg-emerald-500/5 border border-emerald-100 dark:border-emerald-500/20"
          )}
        >
          <div className="flex items-center justify-between mb-1">
            <span className="font-semibold text-slate-600 dark:text-gray-300">
              {msg.direction === "incoming" ? (
                <>
                  <Codicon name="arrow-down" className="text-blue-500 mr-1" />
                  {msg.external_sender_name || msg.external_sender_id || "Unknown"}
                </>
              ) : (
                <>
                  <Codicon name="arrow-up" className="text-emerald-500 mr-1" />
                  Bot Reply
                </>
              )}
            </span>
            <span className="text-[10px] text-slate-400 dark:text-gray-500">
              {formatRelativeTime(msg.created_at)}
            </span>
          </div>
          <p className="text-slate-700 dark:text-gray-200 whitespace-pre-wrap break-words">
            {msg.content}
          </p>
          {msg.agent_response && (
            <div className="mt-2 pt-2 border-t border-slate-200 dark:border-border-dark">
              <span className="text-[10px] font-semibold text-primary mb-1 block">
                Agent Reply:
              </span>
              <p className="text-slate-600 dark:text-gray-300 whitespace-pre-wrap">
                {msg.agent_response}
              </p>
            </div>
          )}
          {msg.error_message && (
            <div className="mt-1 text-[10px] text-rose-500">
              Error: {msg.error_message}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

function ContactList({
  contacts,
  onToggleBlocked,
}: {
  contacts: ChatToolContact[];
  onToggleBlocked: (contactId: string, blocked: boolean) => Promise<void>;
}) {
  const [search, setSearch] = useState("");

  const filtered = contacts.filter(
    (c) =>
      c.name.toLowerCase().includes(search.toLowerCase()) ||
      c.external_id.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div>
      <div className="mb-3">
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search contacts..."
          className="w-full h-8 px-3 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-xs text-slate-800 dark:text-white focus:outline-none focus:border-primary"
        />
      </div>
      {filtered.length === 0 ? (
        <div className="text-center py-8 text-slate-400 dark:text-gray-500">
          <Codicon name="person" className="text-2xl mb-2 block" />
          <p className="text-xs">
            {contacts.length === 0
              ? "No contacts synced yet"
              : "No contacts match your search"}
          </p>
        </div>
      ) : (
        <div className="space-y-1 max-h-[400px] overflow-y-auto">
          {filtered.map((contact) => (
            <div
              key={contact.id}
              className="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-slate-50 dark:hover:bg-white/5"
            >
              <div className="flex items-center gap-2">
                <div className="size-7 rounded-full bg-slate-200 dark:bg-surface-dark flex items-center justify-center text-slate-400 dark:text-gray-500">
                  <Codicon name="person" className="text-[12px]" />
                </div>
                <div>
                  <div className="text-xs font-medium text-slate-700 dark:text-gray-200">
                    {contact.name}
                  </div>
                  <div className="text-[10px] text-slate-400 dark:text-gray-500">
                    {contact.contact_type}
                  </div>
                </div>
              </div>
              <button
                onClick={() => onToggleBlocked(contact.id, !contact.is_blocked)}
                className={cn(
                  "text-[10px] px-2 py-0.5 rounded font-medium transition-colors",
                  contact.is_blocked
                    ? "bg-rose-100 dark:bg-rose-500/10 text-rose-500 hover:bg-rose-200"
                    : "bg-slate-100 dark:bg-surface-dark text-slate-400 hover:bg-slate-200"
                )}
              >
                {contact.is_blocked ? "Blocked" : "Block"}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function formatRelativeTime(dateStr: string): string {
  try {
    const date = new Date(dateStr + "Z"); // Assume UTC from DB
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSec = Math.floor(diffMs / 1000);

    if (diffSec < 60) return "just now";
    if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
    if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
    return `${Math.floor(diffSec / 86400)}d ago`;
  } catch {
    return dateStr;
  }
}

function getHealthStatus(
  lastActiveAt: string | null
): "healthy" | "delayed" | "disconnected" {
  if (!lastActiveAt) return "disconnected";
  try {
    const date = new Date(lastActiveAt + "Z");
    const diffSec = (Date.now() - date.getTime()) / 1000;
    if (diffSec <= 60) return "healthy";
    if (diffSec <= 120) return "delayed";
    return "disconnected";
  } catch {
    return "disconnected";
  }
}
