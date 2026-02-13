import { create } from 'zustand';
import { tauriInvoke } from '@/lib/tauri';

interface SettingsState {
  theme: 'dark' | 'light';
  language: string;
  fontSize: number;
  settings: Record<string, string>;
  loaded: boolean;
  workingDirectory: string | null;
}

interface SettingsActions {
  loadSettings: () => Promise<void>;
  updateSetting: (key: string, value: string) => Promise<void>;
  toggleTheme: () => void;
  setLanguage: (lang: string) => void;
  setFontSize: (size: number) => void;
  selectWorkingDirectory: () => Promise<string | null>;
  loadWorkingDirectory: () => Promise<void>;
}

function applyThemeClass(theme: 'dark' | 'light') {
  if (typeof document !== 'undefined') {
    if (theme === 'dark') {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('theme', theme);
  }
}

export const useSettingsStore = create<SettingsState & SettingsActions>((set, get) => ({
  theme: 'dark',
  language: 'en',
  fontSize: 14,
  settings: {},
  loaded: false,
  workingDirectory: null,

  loadSettings: async () => {
    try {
      const settings = await tauriInvoke<Record<string, string>>('get_settings');
      const theme = (settings.theme === 'light' ? 'light' : 'dark') as 'dark' | 'light';
      const language = settings.language ?? 'en';
      const fontSize = settings.fontSize ? parseInt(settings.fontSize, 10) : 14;

      applyThemeClass(theme);

      set({
        settings,
        theme,
        language,
        fontSize: isNaN(fontSize) ? 14 : fontSize,
        loaded: true,
      });
    } catch (error) {
      console.error('Failed to load settings:', error);
      // Fallback: read theme from localStorage when Tauri backend is unavailable
      if (typeof localStorage !== 'undefined') {
        const stored = localStorage.getItem('theme');
        if (stored === 'light' || stored === 'dark') {
          applyThemeClass(stored);
          set({ theme: stored, loaded: true });
          return;
        }
      }
      set({ loaded: true });
    }
  },

  updateSetting: async (key, value) => {
    try {
      await tauriInvoke<void>('update_settings', { key, value });
      set((state) => ({
        settings: { ...state.settings, [key]: value },
      }));
    } catch (error) {
      console.error('Failed to update setting:', error);
      throw error;
    }
  },

  toggleTheme: () => {
    const current = get().theme;
    const next = current === 'dark' ? 'light' : 'dark';
    applyThemeClass(next);
    set({ theme: next });

    // Persist the theme change to the backend
    tauriInvoke<void>('update_settings', { key: 'theme', value: next }).catch((error) => {
      console.error('Failed to persist theme setting:', error);
    });
  },

  setLanguage: (lang) => {
    set({ language: lang });

    tauriInvoke<void>('update_settings', { key: 'language', value: lang }).catch((error) => {
      console.error('Failed to persist language setting:', error);
    });
  },

  setFontSize: (size) => {
    set({ fontSize: size });

    tauriInvoke<void>('update_settings', { key: 'fontSize', value: String(size) }).catch(
      (error) => {
        console.error('Failed to persist font size setting:', error);
      }
    );
  },

  selectWorkingDirectory: async () => {
    try {
      const path = await tauriInvoke<string | null>('select_working_directory');
      if (path) {
        set({ workingDirectory: path });
      }
      return path;
    } catch (error) {
      console.error('Failed to select working directory:', error);
      return null;
    }
  },

  loadWorkingDirectory: async () => {
    try {
      const path = await tauriInvoke<string | null>('get_working_directory');
      set({ workingDirectory: path ?? null });
    } catch (error) {
      console.error('Failed to load working directory:', error);
    }
  },
}));
