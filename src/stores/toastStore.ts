import { create } from 'zustand';

export interface Toast {
  id: string;
  type: 'error' | 'warning' | 'info' | 'success';
  title: string;
  message?: string;
  duration?: number; // ms, default 5000; 0 = manual close only
}

interface ToastState {
  toasts: Toast[];
}

interface ToastActions {
  addToast: (toast: Omit<Toast, 'id'>) => void;
  removeToast: (id: string) => void;
}

export const useToastStore = create<ToastState & ToastActions>((set) => ({
  toasts: [],

  addToast: (toast) => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    set((state) => ({
      toasts: [...state.toasts, { ...toast, id }],
    }));
  },

  removeToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    }));
  },
}));

// ---------------------------------------------------------------------------
// Convenience helpers — callable from any store (no React hooks needed)
// ---------------------------------------------------------------------------

export function showError(title: string, message?: unknown) {
  useToastStore.getState().addToast({
    type: 'error',
    title,
    message: formatMessage(message),
  });
}

export function showWarning(title: string, message?: unknown) {
  useToastStore.getState().addToast({
    type: 'warning',
    title,
    message: formatMessage(message),
  });
}

export function showInfo(title: string, message?: unknown) {
  useToastStore.getState().addToast({
    type: 'info',
    title,
    message: formatMessage(message),
  });
}

export function showSuccess(title: string, message?: unknown) {
  useToastStore.getState().addToast({
    type: 'success',
    title,
    message: formatMessage(message),
  });
}

function formatMessage(value: unknown): string | undefined {
  if (value === undefined || value === null) return undefined;
  if (value instanceof Error) return value.message;
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
