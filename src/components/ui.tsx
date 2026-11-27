import { useEffect, useRef, useState, type ReactNode, type ButtonHTMLAttributes } from "react";
import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import { openDialog, useUi } from "../store/ui";

type Variant = "primary" | "secondary" | "ghost" | "danger";

export function Button({ variant = "secondary", className = "", ...p }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  const v: Record<Variant, string> = {
    primary: "bg-accent text-accent-fg hover:brightness-110 border-transparent",
    secondary: "bg-panel-2 text-fg hover:bg-hover border-line",
    ghost: "bg-transparent text-dim hover:text-fg hover:bg-hover border-transparent",
    danger: "bg-err/90 text-white hover:bg-err border-transparent",
  };
  return (
    <button
      {...p}
      className={`inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12.5px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${v[variant]} ${className}`}
    />
  );
}

export function IconButton({ title, active, className = "", ...p }: ButtonHTMLAttributes<HTMLButtonElement> & { title: string; active?: boolean }) {
  return (
    <button
      {...p}
      title={title}
      aria-label={title}
      className={`inline-flex h-7 w-7 items-center justify-center rounded-md transition-colors disabled:opacity-40 ${
        active ? "bg-accent-soft text-accent" : "text-dim hover:bg-hover hover:text-fg"
      } ${className}`}
    />
  );
}

export function Modal({ title, onClose, children, footer, width = 520 }: { title: ReactNode; onClose: () => void; children: ReactNode; footer?: ReactNode; width?: number }) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose]);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="flex max-h-[90vh] w-full flex-col rounded-xl border border-line-strong bg-panel shadow-2xl" style={{ maxWidth: width }} role="dialog" aria-modal>
        <div className="flex items-center justify-between border-b border-line px-4 py-3">
          <div className="text-[14px] font-semibold">{title}</div>
          <IconButton title="Close" onClick={onClose}>
            <X size={16} />
          </IconButton>
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-4 py-4">{children}</div>
        {footer && <div className="flex justify-end gap-2 border-t border-line px-4 py-3">{footer}</div>}
      </div>
    </div>
  );
}

export function confirmDialog(opts: { title: string; message: ReactNode; confirmText?: string; danger?: boolean }): Promise<boolean> {
  return openDialog<boolean>((close) => (
    <Modal
      title={opts.title}
      onClose={() => close(false)}
      footer={
        <>
          <Button onClick={() => close(false)}>Cancel</Button>
          <Button variant={opts.danger ? "danger" : "primary"} onClick={() => close(true)} autoFocus>
            {opts.confirmText ?? "OK"}
          </Button>
        </>
      }
    >
      <div className="text-[13px] leading-relaxed text-fg">{opts.message}</div>
    </Modal>
  ));
}

function PromptBody({ label, initial, secret, close, checkbox }: { label: string; initial: string; secret?: boolean; checkbox?: string; close: (v: { value: string; checked: boolean } | null) => void }) {
  const [v, setV] = useState(initial);
  const [checked, setChecked] = useState(false);
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => ref.current?.select(), []);
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        close({ value: v, checked });
      }}
    >
      <label className="label">{label}</label>
      <input ref={ref} autoFocus className="field" type={secret ? "password" : "text"} value={v} onChange={(e) => setV(e.target.value)} />
      {checkbox && (
        <label className="mt-3 flex items-center gap-2 text-[12.5px] text-dim">
          <input type="checkbox" checked={checked} onChange={(e) => setChecked(e.target.checked)} /> {checkbox}
        </label>
      )}
      <div className="mt-4 flex justify-end gap-2">
        <Button type="button" onClick={() => close(null)}>
          Cancel
        </Button>
        <Button variant="primary" type="submit">
          OK
        </Button>
      </div>
    </form>
  );
}

export function promptDialog(opts: { title: string; label: string; initial?: string; secret?: boolean; checkbox?: string }) {
  return openDialog<{ value: string; checked: boolean } | null>((close) => (
    <Modal title={opts.title} onClose={() => close(null)} width={440}>
      <PromptBody label={opts.label} initial={opts.initial ?? ""} secret={opts.secret} checkbox={opts.checkbox} close={close} />
    </Modal>
  ));
}

export function DialogHost() {
  const dialogs = useUi((s) => s.dialogs);
  return (
    <>
      {dialogs.map((d) => (
        <div key={d.id}>{d.node()}</div>
      ))}
    </>
  );
}

export function Toasts() {
  const { toasts, dismiss } = useUi();
  const icon = { info: <Info size={16} className="text-accent" />, success: <CheckCircle2 size={16} className="text-ok" />, error: <XCircle size={16} className="text-err" />, warn: <AlertTriangle size={16} className="text-warn" /> };
  return (
    <div className="pointer-events-none fixed right-4 bottom-10 z-[60] flex w-[360px] flex-col gap-2">
      {toasts.map((t) => (
        <div key={t.id} className="pointer-events-auto flex items-start gap-2 rounded-lg border border-line-strong bg-panel px-3 py-2.5 shadow-xl">
          <div className="mt-0.5">{icon[t.kind]}</div>
          <div className="flex-1 text-[12.5px] leading-snug break-words select-text">{t.text}</div>
          <button className="text-faint hover:text-fg" onClick={() => dismiss(t.id)} aria-label="Dismiss">
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  onClick: () => void;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
}

export function ContextMenu({ x, y, items, onClose }: { x: number; y: number; items: MenuItem[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  useEffect(() => {
    const el = ref.current;
    if (el) {
      const r = el.getBoundingClientRect();
      setPos({ x: Math.min(x, window.innerWidth - r.width - 8), y: Math.min(y, window.innerHeight - r.height - 8) });
    }
    const h = () => onClose();
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", h);
    window.addEventListener("keydown", k);
    window.addEventListener("blur", h);
    return () => {
      window.removeEventListener("mousedown", h);
      window.removeEventListener("keydown", k);
      window.removeEventListener("blur", h);
    };
  }, [x, y, onClose]);
  return (
    <div ref={ref} className="fixed z-[70] min-w-[190px] rounded-lg border border-line-strong bg-panel py-1 shadow-2xl" style={{ left: pos.x, top: pos.y }} onMouseDown={(e) => e.stopPropagation()}>
      {items.map((it, i) =>
        it.separator ? (
          <div key={i} className="my-1 border-t border-line" />
        ) : (
          <button
            key={i}
            disabled={it.disabled}
            onClick={() => {
              onClose();
              it.onClick();
            }}
            className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-[12.5px] hover:bg-hover disabled:opacity-40 ${it.danger ? "text-err" : "text-fg"}`}
          >
            <span className="w-4 text-dim">{it.icon}</span>
            {it.label}
          </button>
        ),
      )}
    </div>
  );
}

export function useContextMenu() {
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const node = menu ? <ContextMenu {...menu} onClose={() => setMenu(null)} /> : null;
  return { open: (e: { clientX: number; clientY: number; preventDefault: () => void }, items: MenuItem[]) => (e.preventDefault(), setMenu({ x: e.clientX, y: e.clientY, items })), node };
}

export function EmptyState({ icon, title, children }: { icon: ReactNode; title: string; children?: ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
      <div className="text-faint">{icon}</div>
      <div className="text-[14px] font-semibold">{title}</div>
      {children && <div className="max-w-md text-[12.5px] text-dim">{children}</div>}
    </div>
  );
}

export function Field({ label, children, hint, error }: { label: string; children: ReactNode; hint?: ReactNode; error?: string | null }) {
  return (
    <div className={error ? "[&_.field]:!border-err" : undefined}>
      <label className="label">{label}</label>
      {children}
      {error ? (
        <div className="mt-1 text-[11.5px] text-err" role="alert">
          {error}
        </div>
      ) : (
        hint && <div className="mt-1 text-[11px] text-faint">{hint}</div>
      )}
    </div>
  );
}

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "ok" | "warn" | "err" | "accent" }) {
  const t = { neutral: "bg-panel-2 text-dim", ok: "bg-ok/15 text-ok", warn: "bg-warn/15 text-warn", err: "bg-err/15 text-err", accent: "bg-accent-soft text-accent" };
  return <span className={`inline-flex items-center rounded-md px-1.5 py-0.5 text-[10.5px] font-medium ${t[tone]}`}>{children}</span>;
}
