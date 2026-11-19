// Small pieces every page shares: the split-flap figures, the plane mark, and the
// media queries the layout and the motion follow.

import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { errorMessage } from './i18n';

/** Whether `query` matches now, re-rendering when that changes. */
export function useMedia(query: string): boolean {
  return useSyncExternalStore(
    (onChange) => {
      const list = matchMedia(query);
      list.addEventListener('change', onChange);
      return () => list.removeEventListener('change', onChange);
    },
    () => matchMedia(query).matches,
  );
}

/** Two columns, the board and its pass, from this width up; below it the pass opens under its row. */
export const WIDE = '(min-width: 60rem)';

export const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

/**
 * A figure set as split-flap characters: each one flips in, staggered, when it first
 * shows or changes. The text stays one string to copy and to assistive technology.
 */
export function Flap({ text }: { text: string }) {
  // Every character in within 400ms: the last starts by 140ms and flips for 260ms.
  const step = Math.min(30, 140 / Math.max(1, text.length));
  return (
    <span className="flap">
      {[...text].map((ch, i) => (
        // Keyed by position and character, so only a changed character flips again.
        <span key={`${i}${ch}`} style={{ animationDelay: `${Math.round(i * step)}ms` }}>
          {ch}
        </span>
      ))}
    </span>
  );
}

/** The plane mark, drawn, nose to the right. */
export function Plane({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path
        fill="currentColor"
        d="M21.4 12c0-.8-.7-1.4-1.6-1.4h-5.1L9.9 3H7.8l2.5 7.6H5.2L3.6 8.3H2l1 3.7-1 3.7h1.6l1.6-2.3h5.1L7.8 21h2.1l4.8-7.6h5.1c.9 0 1.6-.6 1.6-1.4Z"
      />
    </svg>
  );
}

// ---------- Routing: the History API under /app, no reloads ----------

const routeListeners = new Set<() => void>();

/** Goes to `to` (a path under /app) without reloading; `replace` keeps Back where it was. */
export function navigate(to: string, { replace = false, keepScroll = false } = {}) {
  if (to === location.pathname + location.search + location.hash) return;
  history[replace ? 'replaceState' : 'pushState'](null, '', to);
  routeListeners.forEach((listener) => listener());
  if (!keepScroll) scrollTo(0, 0);
}

/** The current path and query, re-rendering on every navigation and on Back and Forward. */
export function useLocationKey(): string {
  return useSyncExternalStore(
    (onChange) => {
      routeListeners.add(onChange);
      addEventListener('popstate', onChange);
      return () => {
        routeListeners.delete(onChange);
        removeEventListener('popstate', onChange);
      };
    },
    () => location.pathname + location.search + location.hash,
  );
}

/** Follows plain clicks on links within /app with `navigate` instead of a page load. */
export function interceptLinks(e: MouseEvent) {
  const a = (e.target as Element).closest('a');
  if (!a || e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  if (a.target || a.hasAttribute('download') || a.origin !== location.origin || !a.pathname.startsWith('/app')) return;
  e.preventDefault();
  navigate(a.pathname + a.search + a.hash);
}

// ---------- Popovers and menus ----------

/**
 * Open state for a popover anchored in `ref`: closes on a click outside, on Escape
 * (returning focus to its button) and on navigation.
 */
export function usePopover<T extends HTMLElement>() {
  const [open, setOpen] = useState(false);
  const ref = useRef<T>(null);
  const where = useLocationKey();
  const [openedAt, setOpenedAt] = useState(where);
  if (open && openedAt !== where) {
    setOpen(false);
  }
  useEffect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      setOpen(false);
      ref.current?.querySelector<HTMLElement>('[aria-expanded]')?.focus();
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape);
    };
  }, [open]);
  const toggle = () => {
    setOpenedAt(where);
    setOpen((o) => !o);
  };
  return { open, setOpen, toggle, ref };
}

/** A menu button: its items in an ARIA menu, keyboard operable (arrows, Home, End, Escape, Tab). */
export function MoreMenu({
  label,
  items,
}: {
  label: string;
  items: { label: string; danger?: boolean; onSelect: () => void }[];
}) {
  const { open, setOpen, toggle, ref } = usePopover<HTMLDivElement>();
  const id = useId();
  const menu = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (open) menu.current?.querySelector<HTMLElement>('[role=menuitem]')?.focus();
  }, [open]);

  function onKeyDown(e: ReactKeyboardEvent) {
    const all = [...(menu.current?.querySelectorAll<HTMLElement>('[role=menuitem]') ?? [])];
    const at = all.indexOf(document.activeElement as HTMLElement);
    const to = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: all.length - 1 }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      all[(to + all.length) % all.length]?.focus();
    } else if (e.key === 'Tab') setOpen(false);
  }

  return (
    <div className="menu-wrap" ref={ref}>
      <button
        ref={button}
        className="small icon"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={toggle}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown' && !open) {
            e.preventDefault();
            toggle();
          }
        }}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false">
          <circle cx="3" cy="8" r="1.5" />
          <circle cx="8" cy="8" r="1.5" />
          <circle cx="13" cy="8" r="1.5" />
        </svg>
        <span className="vh">{label}</span>
      </button>
      {open && (
        <div id={id} ref={menu} role="menu" aria-label={label} className="popover menu" onKeyDown={onKeyDown}>
          {items.map((item) => (
            <button
              key={item.label}
              role="menuitem"
              tabIndex={-1}
              className={item.danger ? 'danger' : undefined}
              onClick={() => {
                setOpen(false);
                // Focus back on the menu's button, so a dialog the item opens returns it there.
                button.current?.focus();
                item.onSelect();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function Chevron() {
  return (
    <svg className="chevron" viewBox="0 0 12 8" aria-hidden="true" focusable="false">
      <path d="M1 1.5 6 6.5l5-5" fill="none" stroke="currentColor" strokeWidth="1.6" />
    </svg>
  );
}

// ---------- Toasts ----------

/** A toast at the bottom of the screen, read out politely; `show` replaces the one showing. */
export function useToast() {
  const [message, setMessage] = useState<{ text: string; n: number }>();
  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(() => setMessage(undefined), 3200);
    return () => clearTimeout(timer);
  }, [message]);
  const show = useCallback((text: string) => setMessage((m) => ({ text, n: (m?.n ?? 0) + 1 })), []);
  const node = (
    <div className="toasts" aria-live="polite">
      {message && (
        <p key={message.n} className="toast">
          {message.text}
        </p>
      )}
    </div>
  );
  return [node, show] as const;
}

// ---------- Confirming and failing ----------

/**
 * An in-app "Are you sure?": a modal `<dialog>` (focus kept inside, Escape cancels,
 * focus back on the button that asked). `ask` resolves to whether `action` was chosen.
 */
export function useConfirm() {
  const { t } = useTranslation();
  const id = useId();
  const ref = useRef<HTMLDialogElement>(null);
  const [asking, setAsking] = useState<{ message: ReactNode; action: string; done: (yes: boolean) => void }>();
  useEffect(() => {
    if (asking) ref.current?.showModal();
  }, [asking]);
  const ask = useCallback(
    (message: ReactNode, action: string) => new Promise<boolean>((done) => setAsking({ message, action, done })),
    [],
  );
  const node = asking && (
    <dialog
      ref={ref}
      className="confirm"
      aria-labelledby={id}
      onClose={(e) => {
        asking.done(e.currentTarget.returnValue === 'yes');
        setAsking(undefined);
      }}
    >
      <form method="dialog">
        <div id={id} className="confirm-message">
          {asking.message}
        </div>
        <div className="confirm-acts">
          <button value="no" autoFocus>
            {t('cancel')}
          </button>
          <button value="yes" className="danger">
            {asking.action}
          </button>
        </div>
      </form>
    </dialog>
  );
  return [node, ask] as const;
}

/** A page that failed to load: what went wrong, and a way to try again. */
export function PageError({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="page-error">
      <p role="alert">{errorMessage(t, error)}</p>
      <button className="small" onClick={onRetry}>
        {t('retry')}
      </button>
    </div>
  );
}
