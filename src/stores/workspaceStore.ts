import { create } from 'zustand';
import { tauriInvoke, isTauri } from '@/lib/tauri';
import type {
  Workspace,
  CreateWorkspaceRequest,
  UpdateWorkspaceRequest,
} from '@/types/workspace';

interface WorkspaceState {
  workspaces: Workspace[];
  activeWorkspaceId: string | null;
  loading: boolean;
  sidebarExpanded: boolean;
}

interface WorkspaceActions {
  fetchWorkspaces: () => Promise<void>;
  setActiveWorkspace: (id: string) => Promise<void>;
  createWorkspace: (req: CreateWorkspaceRequest) => Promise<Workspace>;
  updateWorkspace: (id: string, req: UpdateWorkspaceRequest) => Promise<Workspace>;
  deleteWorkspace: (id: string) => Promise<void>;
  selectWorkspaceDirectory: (workspaceId: string) => Promise<string | null>;
  toggleSidebar: () => void;
}

export const useWorkspaceStore = create<WorkspaceState & WorkspaceActions>(
  (set, get) => ({
    workspaces: [],
    activeWorkspaceId: null,
    loading: false,
    sidebarExpanded: false,

    fetchWorkspaces: async () => {
      if (!isTauri()) return;
      set({ loading: true });
      try {
        const workspaces = await tauriInvoke<Workspace[]>('list_workspaces');
        // Get active workspace from settings
        const settings = await tauriInvoke<Record<string, string>>('get_settings');
        const activeId = settings?.active_workspace_id || workspaces[0]?.id || null;

        set({ workspaces, activeWorkspaceId: activeId, loading: false });
      } catch (error) {
        console.error('[WorkspaceStore] Failed to fetch workspaces:', error);
        set({ loading: false });
      }
    },

    setActiveWorkspace: async (id: string) => {
      set({ activeWorkspaceId: id });
      try {
        await tauriInvoke('update_settings', { key: 'active_workspace_id', value: id });
      } catch (error) {
        console.error('[WorkspaceStore] Failed to persist active workspace:', error);
      }
    },

    createWorkspace: async (req: CreateWorkspaceRequest) => {
      const workspace = await tauriInvoke<Workspace>('create_workspace', { request: req });
      set((state) => ({ workspaces: [...state.workspaces, workspace] }));
      return workspace;
    },

    updateWorkspace: async (id: string, req: UpdateWorkspaceRequest) => {
      const workspace = await tauriInvoke<Workspace>('update_workspace', { id, request: req });
      set((state) => ({
        workspaces: state.workspaces.map((w) => (w.id === id ? workspace : w)),
      }));
      return workspace;
    },

    deleteWorkspace: async (id: string) => {
      await tauriInvoke('delete_workspace', { id });
      set((state) => {
        const remaining = state.workspaces.filter((w) => w.id !== id);
        const needsNewActive = state.activeWorkspaceId === id;
        return {
          workspaces: remaining,
          activeWorkspaceId: needsNewActive ? remaining[0]?.id ?? null : state.activeWorkspaceId,
        };
      });
      // If the deleted workspace was active, persist the new active
      const { activeWorkspaceId } = get();
      if (activeWorkspaceId) {
        await tauriInvoke('update_settings', {
          key: 'active_workspace_id',
          value: activeWorkspaceId,
        });
      }
    },

    selectWorkspaceDirectory: async (workspaceId: string) => {
      const path = await tauriInvoke<string | null>('select_workspace_directory', {
        workspaceId,
      });
      if (path) {
        set((state) => ({
          workspaces: state.workspaces.map((w) =>
            w.id === workspaceId ? { ...w, working_directory: path } : w
          ),
        }));
      }
      return path;
    },

    toggleSidebar: () => {
      set((state) => ({ sidebarExpanded: !state.sidebarExpanded }));
    },
  })
);
