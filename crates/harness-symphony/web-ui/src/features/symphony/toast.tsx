import React from "react";
import { createPortal } from "react-dom";
import { CheckCircle2, XCircle, Info, X } from "lucide-react";
import { cn } from "../../lib/utils";

// ─── Types ───────────────────────────────────────────────────────────────────
export type ToastKind = "success" | "error" | "info";

export type Toast = {
  id: number;
  kind: ToastKind;
  title: string;
  description?: string;
  durationMs?: number;
};

type ToastAction =
  | { type: "ADD"; toast: Omit<Toast, "id"> }
  | { type: "REMOVE"; id: number };

// ─── Context ─────────────────────────────────────────────────────────────────
const ToastContext = React.createContext<{
  add: (toast: Omit<Toast, "id">) => void;
} | null>(null);

// ─── Hook ─────────────────────────────────────────────────────────────────────
export function useToast() {
  const ctx = React.useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be inside <ToastProvider>");
  return ctx;
}

// ─── Provider ─────────────────────────────────────────────────────────────────
let nextId = 1;

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, dispatch] = React.useReducer(
    (state: Toast[], action: ToastAction): Toast[] => {
      if (action.type === "ADD") {
        return [...state.slice(-4), { ...action.toast, id: nextId++ }];
      }
      if (action.type === "REMOVE") {
        return state.filter((t) => t.id !== action.id);
      }
      return state;
    },
    []
  );

  const add = React.useCallback((toast: Omit<Toast, "id">) => {
    dispatch({ type: "ADD", toast });
  }, []);

  const remove = React.useCallback((id: number) => {
    dispatch({ type: "REMOVE", id });
  }, []);

  return (
    <ToastContext.Provider value={{ add }}>
      {children}
      {createPortal(
        <div
          className="toast-host"
          aria-live="polite"
          aria-atomic="false"
          role="region"
          aria-label="Notifications"
        >
          {toasts.map((t) => (
            <ToastItem key={t.id} toast={t} onDismiss={() => remove(t.id)} />
          ))}
        </div>,
        document.body
      )}
    </ToastContext.Provider>
  );
}

// ─── Toast Item ───────────────────────────────────────────────────────────────
function ToastItem({
  toast,
  onDismiss,
}: {
  toast: Toast;
  onDismiss: () => void;
}) {
  const [dismissing, setDismissing] = React.useState(false);
  const durationMs = toast.durationMs ?? 4000;
  const prefersReducedMotion = usePrefersReducedMotion();

  const dismiss = React.useCallback(() => {
    if (dismissing) return;
    setDismissing(true);
    if (prefersReducedMotion) {
      onDismiss();
    } else {
      setTimeout(onDismiss, 200);
    }
  }, [dismissing, onDismiss, prefersReducedMotion]);

  React.useEffect(() => {
    const timer = window.setTimeout(dismiss, durationMs);
    return () => window.clearTimeout(timer);
  }, [dismiss, durationMs]);

  const iconClass = "toast__icon";

  return (
    <div
      role="alert"
      aria-live="assertive"
      className={cn("toast relative", `toast--${toast.kind}`, dismissing && "toast--dismiss")}
    >
      {toast.kind === "success" ? (
        <CheckCircle2 className={cn(iconClass, "text-[--state-done]")} />
      ) : toast.kind === "error" ? (
        <XCircle className={cn(iconClass, "text-destructive")} />
      ) : (
        <Info className={cn(iconClass, "text-[--state-progress]")} />
      )}
      <div className="toast__body">
        <p className="toast__title">{toast.title}</p>
        {toast.description ? (
          <p className="toast__desc">{toast.description}</p>
        ) : null}
      </div>
      <button
        type="button"
        className="toast__close"
        onClick={dismiss}
        aria-label="Dismiss notification"
      >
        <X size={14} />
      </button>
      {!prefersReducedMotion ? (
        <div
          className="toast__progress"
          style={{ animationDuration: `${durationMs}ms` }}
        />
      ) : null}
    </div>
  );
}

function usePrefersReducedMotion() {
  const [prefers, setPrefers] = React.useState(() =>
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
  React.useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const sync = () => setPrefers(mq.matches);
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  }, []);
  return prefers;
}
