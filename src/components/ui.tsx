import { useEffect, useLayoutEffect, useRef, type ReactNode, type TextareaHTMLAttributes } from "react";
import { createPortal } from "react-dom";
import { X } from "@phosphor-icons/react";
import { useFlow } from "../store";

type ModalProps = {
  kicker?: string;
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  large?: boolean;
};

export function Modal({
  kicker,
  title,
  onClose,
  children,
  footer,
  wide = false,
  large = false,
}: ModalProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const overflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const focusTimer = window.setTimeout(() => {
      const target = panelRef.current?.querySelector<HTMLElement>(
        "input:not([type='hidden']), textarea, select, button.primary-focus",
      );
      (target ?? panelRef.current)?.focus();
    }, 30);

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);

    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = overflow;
      previous?.focus();
    };
  }, [onClose]);

  return createPortal(
    <div
      className="overlay"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        ref={panelRef}
        className={`modal${wide ? " wide" : ""}${large ? " large" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        data-testid="modal"
        tabIndex={-1}
      >
        <div className="modal-head">
          <div>
            {kicker ? <div className="modal-kicker">{kicker}</div> : null}
            <h2 className="modal-title">{title}</h2>
          </div>
          <button
            type="button"
            className="icon-btn"
            aria-label="Close"
            data-testid="modal-close"
            onClick={onClose}
          >
            <X size={16} />
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer ? <div className="modal-foot">{footer}</div> : null}
      </div>
    </div>,
    document.body,
  );
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <label className="field">
      <span className="field-label">
        <span>{label}</span>
        {hint ? <span className="field-hint">{hint}</span> : null}
      </span>
      {children}
    </label>
  );
}

export function AutoGrowTextarea({
  className,
  value,
  ...props
}: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  const ref = useRef<HTMLTextAreaElement>(null);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }
    element.style.height = "auto";
    const border = element.offsetHeight - element.clientHeight;
    element.style.height = `${element.scrollHeight + border}px`;
  }, [value]);

  useEffect(() => {
    const onResize = () => {
      const element = ref.current;
      if (!element) {
        return;
      }
      element.style.height = "auto";
      const border = element.offsetHeight - element.clientHeight;
      element.style.height = `${element.scrollHeight + border}px`;
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return (
    <textarea
      ref={ref}
      value={value}
      className={["text-area", "auto-grow", className].filter(Boolean).join(" ")}
      {...props}
    />
  );
}

export function EmptyState({
  title,
  icon,
  body,
  children,
}: {
  title: string;
  icon?: ReactNode;
  body?: string;
  children?: ReactNode;
}) {
  return (
    <div className="empty-state">
      {icon ? <span className="empty-icon">{icon}</span> : null}
      <strong>{title}</strong>
      {body ? <p className="empty-body">{body}</p> : null}
      {children}
    </div>
  );
}

export function Toasts() {
  const toasts = useFlow((state) => state.toasts);
  const dismiss = useFlow((state) => state.dismissToast);

  return createPortal(
    <div className="toast-stack" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className="toast"
          data-tone={toast.tone}
          data-testid="toast"
        >
          <div className="toast-body">
            <div className="toast-title">{toast.title}</div>
            {toast.detail ? (
              <div className="toast-text">{toast.detail}</div>
            ) : null}
          </div>
          <button
            type="button"
            className="toast-close"
            aria-label="Dismiss"
            onClick={() => dismiss(toast.id)}
          >
            <X size={13} />
          </button>
        </div>
      ))}
    </div>,
    document.body,
  );
}

export function ProgressThread() {
  const busy = useFlow((state) => state.busy);
  return (
    <div
      className="progress-thread"
      style={{ width: busy ? "38%" : "100%", opacity: busy ? 1 : 0 }}
    />
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <span className="kbd">{children}</span>;
}

