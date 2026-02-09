<!DOCTYPE html>
<html class="dark" lang="en"><head>
<meta charset="utf-8"/>
<meta content="width=device-width, initial-scale=1.0" name="viewport"/>
<title>AgentOS Desktop</title>
<link href="https://fonts.googleapis.com" rel="preconnect"/>
<link crossorigin="" href="https://fonts.gstatic.com" rel="preconnect"/>
<link href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@300;400;500;600;700&amp;family=Noto+Sans:wght@400;500;600&amp;display=swap" rel="stylesheet"/>
<link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:wght,FILL@100..700,0..1&amp;display=swap" rel="stylesheet"/>
<script src="https://cdn.tailwindcss.com?plugins=forms,container-queries"></script>
<script id="tailwind-config">
        tailwind.config = {
            darkMode: "class",
            theme: {
                extend: {
                    colors: {
                        "primary": "#00E5FF",
                        "background-light": "#f0fdfa",
                        "background-dark": "#0A0A10",
                        "surface-dark": "#12121A",
                        "border-dark": "#1E1E2E",
                    },
                    fontFamily: {
                        "display": ["Space Grotesk", "sans-serif"],
                        "body": ["Noto Sans", "sans-serif"],
                    },
                    borderRadius: { "DEFAULT": "0.25rem", "lg": "0.5rem", "xl": "0.75rem", "2xl": "1rem", "full": "9999px" },
                },
            },
        }
    </script>
<style type="text/tailwindcss">
        ::-webkit-scrollbar {
            width: 8px;
            height: 8px;
        }
        ::-webkit-scrollbar-track {
            background: #0A0A10;
        }
        ::-webkit-scrollbar-thumb {
            background: #1E1E2E;
            border-radius: 4px;
        }
        ::-webkit-scrollbar-thumb:hover {
            background: #2D2D3F;
        }
        .material-symbols-outlined {
            font-variation-settings:
            'FILL' 0,
            'wght' 400,
            'GRAD' 0,
            'opsz' 24
        }
        .icon-filled {
            font-variation-settings:
            'FILL' 1,
            'wght' 400,
            'GRAD' 0,
            'opsz' 24
        }
        #config-section.collapsed {
            display: none;
        }
        #chat-section.expanded {
            height: 100% !important;
            flex: 1 1 0% !important;
        }
        .collapse-btn-active .material-symbols-outlined {
            transform: rotate(180deg);
        }
        .icon-dropdown, .user-dropdown, .custom-select-dropdown {
            display: none;
        }
        .icon-dropdown.active, .user-dropdown.active, .custom-select-dropdown.active {
            display: block;
        }
        .execution-card.active {
            @apply border-primary bg-primary/5 ring-1 ring-primary/50;
        }
        .capability-tag {
            @apply px-2 py-1 rounded text-[10px] font-bold uppercase tracking-wider transition-all cursor-default;
        }
        .capability-tag.mcp {
            @apply bg-cyan-500/10 text-cyan-400 border border-cyan-500/30;
        }
        .capability-tag.skill {
            @apply bg-purple-500/10 text-purple-400 border border-purple-500/30;
        }
        .wizard-step.active {
            @apply bg-primary/10 border-primary/40 text-primary;
        }
        .wizard-step.active .step-icon {
            @apply text-primary;
        }
        .wizard-content:not(.active) {
            display: none;
        }
        .status-dot {
            @apply absolute -top-0.5 -right-0.5 size-2 rounded-full border border-background-dark bg-primary;
            box-shadow: 0 0 4px var(--primary);
        }
        .login-dot {
            @apply absolute -top-0.5 -right-0.5 size-3 rounded-full border-2 border-white dark:border-[#0A0A10] bg-rose-500;
            box-shadow: 0 0 8px rgba(244, 63, 94, 0.4);
        }
    </style>
</head>
<body class="bg-background-light dark:bg-background-dark text-slate-900 dark:text-gray-100 font-display h-screen flex flex-col overflow-hidden selection:bg-primary/30">
<header class="h-16 flex-none border-b border-border-dark bg-white dark:bg-[#0A0A10] px-6 flex items-center justify-between z-50">
<div class="flex items-center gap-3">
<div class="size-8 rounded-lg bg-primary/20 flex items-center justify-center text-primary">
<span class="material-symbols-outlined icon-filled">smart_toy</span>
</div>
<h1 class="text-xl font-bold tracking-tight text-slate-900 dark:text-white">AgentOS</h1>
</div>
<div class="flex items-center gap-2">
<button class="size-9 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-500 dark:text-gray-400 hover:text-slate-900 dark:hover:text-white transition-colors">
<span class="material-symbols-outlined text-[20px]">notifications</span>
</button>
<button class="size-9 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-500 dark:text-gray-400 hover:text-slate-900 dark:hover:text-white transition-colors">
<span class="material-symbols-outlined text-[20px]">settings</span>
</button>
<button class="size-9 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-500 dark:text-gray-400 hover:text-slate-900 dark:hover:text-white transition-colors">
<span class="material-symbols-outlined text-[20px]">dark_mode</span>
</button>
<button class="size-9 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-500 dark:text-gray-400 hover:text-slate-900 dark:hover:text-white transition-colors">
<span class="material-symbols-outlined text-[20px]">language</span>
</button>
<button class="size-9 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-500 dark:text-gray-400 hover:text-slate-900 dark:hover:text-white transition-colors">
<span class="material-symbols-outlined text-[20px]">closed_caption</span>
</button>
<div class="w-px h-6 bg-slate-200 dark:bg-border-dark mx-1"></div>
<div class="relative">
<button class="size-9 rounded-full bg-slate-100 dark:bg-white/5 border-2 border-slate-300 dark:border-white/10 flex items-center justify-center text-slate-500 dark:text-gray-400 hover:border-primary/50 hover:text-primary transition-all relative" id="user-avatar-btn" onclick="toggleUserDropdown()">
<span class="material-symbols-outlined text-[22px]">person</span>
<div class="login-dot"></div>
</button>
<div class="user-dropdown absolute top-full right-0 mt-2 w-56 bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl shadow-2xl z-[60] py-2" id="user-menu-dropdown">
<div class="px-4 py-3 border-b border-slate-100 dark:border-border-dark/50 mb-1">
<p class="text-[10px] font-bold text-rose-500 uppercase tracking-widest mb-1">Account Required</p>
<p class="text-xs text-slate-500 dark:text-gray-400 leading-tight">Please sign in to access your saved agents and advanced features.</p>
</div>
<button class="w-full text-left px-4 py-2.5 text-sm font-bold text-white bg-primary/80 hover:bg-primary mx-0 my-1 transition-colors flex items-center justify-center gap-2">
                    Sign In
                </button>
<div class="h-px bg-slate-100 dark:bg-border-dark/50 my-1"></div>
<button class="w-full text-left px-4 py-2 text-sm text-slate-600 dark:text-gray-300 hover:bg-slate-50 dark:hover:bg-white/5 flex items-center gap-2">
<span class="material-symbols-outlined text-lg">help_outline</span> Help Center
                </button>
</div>
</div>
</div>
</header>
<div class="flex-none h-12 bg-white dark:bg-[#07070C] border-b border-slate-200 dark:border-border-dark flex items-center px-6 justify-center z-40">
<div class="flex items-center gap-3">
<div class="relative group cursor-pointer">
<div class="size-8 rounded-lg bg-primary flex items-center justify-center text-background-dark shadow-[0_0_10px_rgba(0,229,255,0.3)] transition-all transform hover:scale-105">
<span class="material-symbols-outlined text-[20px]" id="sidebar-main-icon">code</span>
</div>
<div class="status-dot"></div>
</div>
<div class="relative group cursor-pointer">
<div class="size-8 rounded-lg bg-white dark:bg-surface-dark border border-slate-200 dark:border-white/5 hover:border-primary/50 hover:bg-slate-50 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-400 hover:text-primary transition-all">
<span class="material-symbols-outlined text-[20px]">manage_search</span>
</div>
<div class="status-dot opacity-0 group-hover:opacity-100 transition-opacity"></div>
</div>
<div class="relative group cursor-pointer">
<div class="size-8 rounded-lg bg-white dark:bg-surface-dark border border-slate-200 dark:border-white/5 hover:border-primary/50 hover:bg-slate-50 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-400 hover:text-primary transition-all">
<span class="material-symbols-outlined text-[20px]">terminal</span>
</div>
<div class="status-dot opacity-0 group-hover:opacity-100 transition-opacity"></div>
</div>
<div class="relative group cursor-pointer">
<div class="size-8 rounded-lg bg-white dark:bg-surface-dark border border-slate-200 dark:border-white/5 hover:border-primary/50 hover:bg-slate-50 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-400 hover:text-primary transition-all">
<span class="material-symbols-outlined text-[20px]">psychology</span>
</div>
<div class="status-dot opacity-0 group-hover:opacity-100 transition-opacity"></div>
</div>
<div class="relative group cursor-pointer">
<div class="size-8 rounded-lg bg-white dark:bg-surface-dark border border-slate-200 dark:border-white/5 hover:border-primary/50 hover:bg-slate-50 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-400 hover:text-primary transition-all">
<span class="material-symbols-outlined text-[20px]">shield</span>
</div>
<div class="status-dot opacity-0 group-hover:opacity-100 transition-opacity"></div>
</div>
<div class="w-px h-5 bg-slate-200 dark:bg-border-dark/60 mx-1"></div>
<button class="size-8 rounded-lg border border-slate-300 dark:border-border-dark bg-white dark:bg-surface-dark hover:border-primary hover:text-primary text-slate-400 dark:text-gray-500 flex items-center justify-center transition-colors">
<span class="material-symbols-outlined text-[18px]">add</span>
</button>
</div>
</div>
<div class="flex flex-1 overflow-hidden">
<main class="flex-1 flex flex-col relative bg-white dark:bg-background-dark">
<div class="flex-1 flex flex-col min-h-0 overflow-y-auto" id="config-section">
<div class="flex items-center justify-between px-8 py-5 flex-none">
<div class="flex items-center gap-4">
<div class="relative">
<button class="size-12 rounded-lg bg-slate-100 dark:bg-surface-dark border-2 border-slate-200 dark:border-border-dark flex items-center justify-center text-primary hover:border-primary transition-all group overflow-hidden" id="agent-icon-btn" onclick="toggleIconSelector()">
<span class="material-symbols-outlined text-[24px]" id="current-agent-icon">code</span>
<div class="absolute inset-0 bg-primary/10 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity">
<span class="material-symbols-outlined text-xs text-primary">edit</span>
</div>
</button>
<div class="icon-dropdown absolute top-full left-0 mt-2 p-3 bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl shadow-2xl z-50 w-48" id="icon-selector-dropdown">
<p class="text-[10px] font-bold text-slate-400 dark:text-gray-500 uppercase tracking-wider mb-3 px-1">Choose Identity</p>
<div class="grid grid-cols-4 gap-2">
<button class="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors" onclick="changeAgentIcon('code')"><span class="material-symbols-outlined text-xl">code</span></button>
<button class="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors" onclick="changeAgentIcon('terminal')"><span class="material-symbols-outlined text-xl">terminal</span></button>
<button class="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors" onclick="changeAgentIcon('psychology')"><span class="material-symbols-outlined text-xl">psychology</span></button>
<button class="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors" onclick="changeAgentIcon('rocket_launch')"><span class="material-symbols-outlined text-xl">rocket_launch</span></button>
<button class="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors" onclick="changeAgentIcon('database')"><span class="material-symbols-outlined text-xl">database</span></button>
<button class="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors" onclick="changeAgentIcon('shield')"><span class="material-symbols-outlined text-xl">shield</span></button>
<button class="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors" onclick="changeAgentIcon('auto_awesome')"><span class="material-symbols-outlined text-xl">auto_awesome</span></button>
<button class="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors" onclick="changeAgentIcon('architecture')"><span class="material-symbols-outlined text-xl">architecture</span></button>
</div>
</div>
</div>
<div>
<div class="flex items-center gap-3 mb-1">
<h2 class="text-2xl font-bold text-slate-900 dark:text-white tracking-tight">Coder Bot v1.2</h2>
<span class="px-2 py-0.5 rounded text-[11px] font-bold bg-primary/20 text-primary border border-primary/30">ACTIVE</span>
</div>
<p class="text-sm text-slate-500 dark:text-gray-400">Specialized in TypeScript refactoring and Next.js architecture.</p>
</div>
</div>
<div class="flex gap-3">
<button class="flex items-center gap-2 px-4 py-2 rounded-lg bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark text-sm font-medium text-slate-600 dark:text-gray-300 hover:text-slate-900 dark:hover:text-white hover:border-slate-400 dark:hover:border-gray-600 transition-colors">
<span class="material-symbols-outlined text-[18px]">history</span> Logs
                        </button>
<button class="flex items-center gap-2 px-4 py-2 rounded-lg bg-primary hover:bg-cyan-400 text-sm font-bold text-background-dark transition-colors shadow-lg shadow-primary/10">
<span class="material-symbols-outlined text-[18px]">play_arrow</span> Run Diagnostics
                        </button>
</div>
</div>
<div class="px-8 border-b border-slate-200 dark:border-border-dark flex-none">
<div class="flex gap-8">
<button class="pb-3 border-b-2 border-primary text-slate-900 dark:text-white text-sm font-bold tracking-wide">Guided Setup</button>
<button class="pb-3 border-b-2 border-transparent text-slate-400 dark:text-gray-500 hover:text-slate-600 dark:hover:text-gray-300 text-sm font-bold tracking-wide transition-colors">Memory &amp; Context</button>
</div>
</div>
<div class="p-8 space-y-10">
<div class="max-w-6xl mx-auto space-y-10">
<div class="space-y-4">
<h3 class="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest">Execution Mode</h3>
<div class="grid grid-cols-3 gap-4">
<div class="execution-card active group cursor-pointer p-4 rounded-xl border border-slate-200 dark:border-border-dark bg-slate-50 dark:bg-surface-dark hover:border-primary/50 transition-all flex flex-col gap-3">
<div class="size-10 rounded-lg bg-primary/10 flex items-center justify-center text-primary">
<span class="material-symbols-outlined">play_circle</span>
</div>
<div>
<p class="font-bold text-sm text-slate-900 dark:text-white">Run Now</p>
<p class="text-[11px] text-slate-500 dark:text-gray-400 mt-1">Manual execution on demand.</p>
</div>
</div>
<div class="execution-card group cursor-pointer p-4 rounded-xl border border-slate-200 dark:border-border-dark bg-slate-50 dark:bg-surface-dark hover:border-primary/50 transition-all flex flex-col gap-3">
<div class="size-10 rounded-lg bg-emerald-500/10 flex items-center justify-center text-emerald-400">
<span class="material-symbols-outlined">calendar_today</span>
</div>
<div>
<p class="font-bold text-sm text-slate-900 dark:text-white">Schedule</p>
<p class="text-[11px] text-slate-500 dark:text-gray-400 mt-1">Execute at specific times.</p>
</div>
</div>
<div class="execution-card group cursor-pointer p-4 rounded-xl border border-slate-200 dark:border-border-dark bg-slate-50 dark:bg-surface-dark hover:border-primary/50 transition-all flex flex-col gap-3">
<div class="size-10 rounded-lg bg-purple-500/10 flex items-center justify-center text-purple-400">
<span class="material-symbols-outlined">sync</span>
</div>
<div>
<p class="font-bold text-sm text-slate-900 dark:text-white">Automate</p>
<p class="text-[11px] text-slate-500 dark:text-gray-400 mt-1">Loop based on triggers.</p>
</div>
</div>
</div>
</div>
<div class="space-y-4">
<h3 class="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest">Parameter Wizard</h3>
<div class="bg-slate-50 dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl p-4">
<div class="flex flex-col lg:flex-row gap-6 items-center">
<div class="flex flex-none gap-1 bg-white dark:bg-background-dark p-1 rounded-lg border border-slate-200 dark:border-border-dark">
<button class="wizard-step active flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-bold transition-all border border-transparent" id="step-btn-1" onclick="showWizardStep(1)">
<span class="material-symbols-outlined text-[16px] step-icon">person_outline</span>
                                            IDENTITY
                                        </button>
<button class="wizard-step flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-bold transition-all border border-transparent text-slate-500" id="step-btn-2" onclick="showWizardStep(2)">
<span class="material-symbols-outlined text-[16px] step-icon">model_training</span>
                                            MODEL
                                        </button>
<button class="wizard-step flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-bold transition-all border border-transparent text-slate-500" id="step-btn-3" onclick="showWizardStep(3)">
<span class="material-symbols-outlined text-[16px] step-icon">thermostat</span>
                                            TEMP
                                        </button>
<button class="wizard-step flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-bold transition-all border border-transparent text-slate-500" id="step-btn-4" onclick="showWizardStep(4)">
<span class="material-symbols-outlined text-[16px] step-icon">token</span>
                                            LIMIT
                                        </button>
</div>
<div class="flex-1 w-full">
<div class="wizard-content active flex items-center gap-4" id="wizard-step-1">
<span class="text-xs font-medium text-slate-400 whitespace-nowrap">Choose Identity:</span>
<div class="relative w-full max-w-sm">
<button class="w-full h-9 flex items-center justify-between px-3 bg-white dark:bg-background-dark border border-slate-200 dark:border-border-dark rounded-lg text-sm text-slate-900 dark:text-white" onclick="toggleCustomSelect('agent-select')">
<span id="selected-agent">Codex</span>
<span class="material-symbols-outlined text-slate-400">unfold_more</span>
</button>
<div class="custom-select-dropdown absolute top-[calc(100%+4px)] left-0 w-full bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-lg shadow-xl z-50 py-1" id="agent-select">
<button class="w-full text-left px-4 py-2 text-sm text-slate-600 dark:text-gray-300 hover:bg-primary/10 hover:text-primary" onclick="selectOption('Codex', 'selected-agent', 'agent-select')">Codex</button>
<button class="w-full text-left px-4 py-2 text-sm text-slate-600 dark:text-gray-300 hover:bg-primary/10 hover:text-primary" onclick="selectOption('Claude Code', 'selected-agent', 'agent-select')">Claude Code</button>
<button class="w-full text-left px-4 py-2 text-sm text-slate-600 dark:text-gray-300 hover:bg-primary/10 hover:text-primary" onclick="selectOption('OpenCode', 'selected-agent', 'agent-select')">OpenCode</button>
</div>
</div>
</div>
<div class="wizard-content flex items-center gap-4" id="wizard-step-2">
<span class="text-xs font-medium text-slate-400 whitespace-nowrap">LLM Model:</span>
<div class="relative w-full max-w-sm">
<button class="w-full h-9 flex items-center justify-between px-3 bg-white dark:bg-background-dark border border-slate-200 dark:border-border-dark rounded-lg text-sm text-slate-900 dark:text-white" onclick="toggleCustomSelect('model-select')">
<span id="selected-model">GPT-4 Turbo</span>
<span class="material-symbols-outlined text-slate-400">unfold_more</span>
</button>
<div class="custom-select-dropdown absolute top-[calc(100%+4px)] left-0 w-full bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-lg shadow-xl z-50 py-1" id="model-select">
<button class="w-full text-left px-4 py-2 text-sm text-slate-600 dark:text-gray-300 hover:bg-primary/10 hover:text-primary" onclick="selectOption('GPT-4 Turbo', 'selected-model', 'model-select')">GPT-4 Turbo</button>
<button class="w-full text-left px-4 py-2 text-sm text-slate-600 dark:text-gray-300 hover:bg-primary/10 hover:text-primary" onclick="selectOption('Claude 3.5 Sonnet', 'selected-model', 'model-select')">Claude 3.5 Sonnet</button>
<button class="w-full text-left px-4 py-2 text-sm text-slate-600 dark:text-gray-300 hover:bg-primary/10 hover:text-primary" onclick="selectOption('Llama 3 70B', 'selected-model', 'model-select')">Llama 3 70B</button>
</div>
</div>
</div>
<div class="wizard-content flex items-center gap-6" id="wizard-step-3">
<div class="flex-1 flex flex-col gap-1 max-w-md">
<div class="flex justify-between items-center">
<span class="text-xs font-medium text-slate-400">Temperature</span>
<span class="text-xs font-mono text-primary">0.7</span>
</div>
<input class="w-full h-1 bg-slate-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary" max="1" min="0" step="0.1" type="range" value="0.7"/>
</div>
</div>
<div class="wizard-content flex items-center gap-6" id="wizard-step-4">
<div class="flex-1 flex flex-col gap-1 max-w-md">
<div class="flex justify-between items-center">
<span class="text-xs font-medium text-slate-400">Max Tokens</span>
<span class="text-xs font-mono text-primary">4096</span>
</div>
<input class="w-full h-1 bg-slate-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary" max="8192" min="256" step="256" type="range" value="4096"/>
</div>
</div>
</div>
</div>
</div>
</div>
<div class="grid grid-cols-12 gap-8">
<div class="col-span-12 lg:col-span-7 flex flex-col gap-8">
<div class="flex flex-col gap-3">
<span class="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest flex justify-between">
                                        System Prompt
                                        <span class="text-[10px] font-normal lowercase tracking-normal text-slate-500">Controls the agent's persona and rules</span>
</span>
<textarea class="w-full h-80 bg-slate-50 dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl p-4 text-slate-700 dark:text-gray-300 text-sm font-mono leading-relaxed focus:ring-1 focus:ring-primary focus:border-primary outline-none resize-none shadow-sm" spellcheck="false">You are an expert TypeScript developer with a focus on clean architecture and performance optimization.
When analyzing code:
1. Prioritize type safety.
2. Suggest immutable patterns where possible.
3. Keep component render cycles efficient.
Always explain your reasoning before providing the refactored code block.</textarea>
</div>
</div>
<div class="col-span-12 lg:col-span-5 flex flex-col gap-8">
<div class="flex flex-col gap-6">
<div class="flex items-center justify-between">
<h3 class="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest">Capabilities</h3>
<div class="flex gap-2">
<span class="px-2 py-0.5 rounded text-[10px] font-bold bg-cyan-500/10 text-cyan-400 border border-cyan-500/30">MCP</span>
<span class="px-2 py-0.5 rounded text-[10px] font-bold bg-purple-500/10 text-purple-400 border border-purple-500/30">SKILL</span>
</div>
</div>
<div class="relative">
<span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 dark:text-gray-500 text-xl">search</span>
<input class="w-full h-11 pl-10 pr-4 bg-slate-50 dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-lg text-sm text-slate-900 dark:text-white focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all placeholder:text-slate-400 dark:placeholder:text-gray-600" placeholder="Search and add capabilities..." type="text"/>
</div>
<div class="flex flex-wrap gap-2 min-h-[100px] p-4 rounded-xl border border-dashed border-slate-200 dark:border-border-dark bg-slate-50/50 dark:bg-surface-dark/50">
<span class="capability-tag mcp flex items-center gap-1.5">Enterprise Retrieve <span class="material-symbols-outlined text-[12px] cursor-pointer">close</span></span>
<span class="capability-tag skill flex items-center gap-1.5">Web Search <span class="material-symbols-outlined text-[12px] cursor-pointer">close</span></span>
<span class="capability-tag skill flex items-center gap-1.5">Code Refactor <span class="material-symbols-outlined text-[12px] cursor-pointer">close</span></span>
<span class="capability-tag mcp flex items-center gap-1.5">SQL Analytics <span class="material-symbols-outlined text-[12px] cursor-pointer">close</span></span>
</div>
</div>
</div>
</div>
</div>
</div>
</div>
<div class="relative h-px bg-slate-200 dark:bg-border-dark flex items-center justify-center flex-none">
<button class="absolute z-30 size-8 rounded-full bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark flex items-center justify-center text-slate-400 dark:text-gray-400 hover:text-primary transition-all shadow-md group" id="collapse-btn" onclick="toggleConfig()">
<span class="material-symbols-outlined text-[20px] transition-transform duration-300">keyboard_double_arrow_up</span>
</button>
</div>
<div class="h-[35%] min-h-[200px] flex flex-col bg-slate-50 dark:bg-[#07070C] relative shadow-[0_-10px_20px_-10px_rgba(0,0,0,0.15)]" id="chat-section">
<div class="flex-1 overflow-y-auto px-8 py-6 flex flex-col gap-6">
<div class="flex justify-end gap-4 pl-12">
<div class="flex flex-col items-end gap-1 max-w-[80%]">
<div class="bg-slate-200 dark:bg-[#1E1E2E] text-slate-900 dark:text-white px-5 py-3 rounded-2xl rounded-tr-sm border border-slate-300 dark:border-white/5 shadow-sm">
<p class="text-sm leading-relaxed font-body">Can you check the <code class="bg-black/10 dark:bg-black/30 px-1 py-0.5 rounded text-xs font-mono">/src/components/auth</code> folder?</p>
</div>
</div>
<div class="size-8 rounded-full overflow-hidden border border-slate-300 dark:border-border-dark flex-none bg-slate-200 dark:bg-surface-dark flex items-center justify-center">
<span class="material-symbols-outlined text-lg text-slate-500">person</span>
</div>
</div>
</div>
<div class="pb-8 pt-2 px-8 bg-slate-50 dark:bg-[#07070C]">
<div class="max-w-6xl mx-auto">
<div class="relative bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl shadow-2xl focus-within:ring-1 focus-within:ring-primary/50 transition-all">
<textarea class="w-full bg-transparent border-none text-slate-900 dark:text-white px-4 py-4 min-h-[100px] text-sm focus:ring-0 placeholder:text-slate-400 font-body resize-none" placeholder="Type your message to Coder Bot..."></textarea>
<div class="flex items-center justify-between px-4 py-3 bg-slate-50/50 dark:bg-white/[0.02] rounded-b-xl border-t border-slate-100 dark:border-border-dark/30">
<div class="flex items-center gap-2">
<button class="size-9 rounded-lg hover:bg-slate-200/50 dark:hover:bg-white/10 flex items-center justify-center text-slate-400 transition-colors" title="Attach image"><span class="material-symbols-outlined text-[20px]">image</span></button>
<button class="size-9 rounded-lg hover:bg-slate-200/50 dark:hover:bg-white/10 flex items-center justify-center text-slate-400 transition-colors" title="Upload file"><span class="material-symbols-outlined text-[20px]">description</span></button>
<button class="size-9 rounded-lg hover:bg-slate-200/50 dark:hover:bg-white/10 flex items-center justify-center text-slate-400 transition-colors" title="Voice input"><span class="material-symbols-outlined text-[20px]">mic</span></button>
</div>
<div class="flex items-center gap-3">
<span class="text-[10px] text-slate-400 dark:text-gray-500 font-medium">⌘ + Enter to send</span>
<button class="h-9 px-5 bg-primary hover:bg-cyan-400 text-background-dark rounded-lg flex items-center gap-2 font-bold text-xs transition-all shadow-lg shadow-primary/20">
                            Send <span class="material-symbols-outlined text-[16px]">send</span>
</button>
</div>
</div>
</div>
</div>
</div>
</div>
</main>
</div>
<script>
    function showWizardStep(stepNumber) {
        document.querySelectorAll('.wizard-content').forEach(content => {
            content.classList.remove('active');
        });
        document.querySelectorAll('.wizard-step').forEach(btn => {
            btn.classList.remove('active');
            btn.classList.add('text-slate-500');
        });
        document.getElementById('wizard-step-' + stepNumber).classList.add('active');
        const activeBtn = document.getElementById('step-btn-' + stepNumber);
        activeBtn.classList.add('active');
        activeBtn.classList.remove('text-slate-500');
    }
    function toggleConfig() {
        const config = document.getElementById('config-section');
        const chat = document.getElementById('chat-section');
        const btn = document.getElementById('collapse-btn');
        config.classList.toggle('collapsed');
        chat.classList.toggle('expanded');
        btn.classList.toggle('collapse-btn-active');
        const icon = btn.querySelector('.material-symbols-outlined');
        if (config.classList.contains('collapsed')) {
            icon.innerText = 'keyboard_double_arrow_down';
        } else {
            icon.innerText = 'keyboard_double_arrow_up';
        }
    }
    function toggleIconSelector() {
        const dropdown = document.getElementById('icon-selector-dropdown');
        dropdown.classList.toggle('active');
    }
    function toggleUserDropdown() {
        const dropdown = document.getElementById('user-menu-dropdown');
        dropdown.classList.toggle('active');
    }
    function toggleCustomSelect(id) {
        const dropdown = document.getElementById(id);
        const isActive = dropdown.classList.contains('active');
        document.querySelectorAll('.custom-select-dropdown').forEach(d => d.classList.remove('active'));
        if (!isActive) dropdown.classList.add('active');
    }
    function selectOption(val, displayId, dropdownId) {
        document.getElementById(displayId).innerText = val;
        document.getElementById(dropdownId).classList.remove('active');
    }
    function changeAgentIcon(iconName) {
        document.getElementById('current-agent-icon').innerText = iconName;
        document.getElementById('sidebar-main-icon').innerText = iconName;
        const chatIcons = document.querySelectorAll('.chat-agent-icon');
        chatIcons.forEach(icon => icon.innerText = iconName);
        document.getElementById('icon-selector-dropdown').classList.remove('active');
    }
    document.querySelectorAll('.execution-card').forEach(card => {
        card.addEventListener('click', () => {
            document.querySelectorAll('.execution-card').forEach(c => c.classList.remove('active'));
            card.classList.add('active');
        });
    });
    document.addEventListener('click', (e) => {
        if (!e.target.closest('.relative') && !e.target.closest('#user-avatar-btn')) {
            document.querySelectorAll('.custom-select-dropdown, .icon-dropdown, .user-dropdown').forEach(d => d.classList.remove('active'));
        }
    });
</script>

</body></html>
