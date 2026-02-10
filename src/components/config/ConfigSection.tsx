"use client";

import { AgentIdentity } from "./AgentIdentity";
import { ParameterWizard } from "./ParameterWizard";
import { SystemPromptEditor } from "./SystemPromptEditor";
import { CapabilitiesPanel } from "./CapabilitiesPanel";

export function ConfigSection() {
  return (
    <>
      <AgentIdentity />
      <div className="px-8 border-b border-slate-200 dark:border-border-dark flex-none">
        <div className="flex gap-8">
          <button className="pb-3 border-b-2 border-primary text-slate-900 dark:text-white text-sm font-bold tracking-wide">
            Guided Setup
          </button>
          <button className="pb-3 border-b-2 border-transparent text-slate-400 dark:text-gray-500 hover:text-slate-600 dark:hover:text-gray-300 text-sm font-bold tracking-wide transition-colors">
            Memory &amp; Context
          </button>
        </div>
      </div>
      <div className="p-8 space-y-10">
        <div className="max-w-6xl mx-auto space-y-10">
          <ParameterWizard />
          <div className="grid grid-cols-12 gap-8">
            <div className="col-span-12 lg:col-span-7 flex flex-col gap-8">
              <SystemPromptEditor />
            </div>
            <div className="col-span-12 lg:col-span-5 flex flex-col gap-8">
              <CapabilitiesPanel />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
