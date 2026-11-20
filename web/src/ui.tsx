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

/** A card's rip, kept between tries: a half tear goes on from where it stopped. See `[data-rip]` in styles.css. */
interface Rip {
  down: boolean;
  /** The side the rip starts from: 0 at the left (top) end, 1 at the right (bottom). */
  start: number;
  /** Per point along the perforation: its place, then the leaving and held halves' torn depth and fibre width. */
  at: number[];
  off: number[];
  held: number[];
  offFibre: number[];
  heldFibre: number[];
  /** How far the rip has run, 0 to 1. */
  front: number;
  animations: Animation[];
}

const rips = new WeakMap<HTMLElement, Rip>();

/** `card`'s rip, drawn the first time: it wanders off the line, frays and nicks; no two alike. */
function ripOf(card: HTMLElement, down: boolean): Rip {
  let rip = rips.get(card);
  if (rip) return rip;
  const n = 72;
  const r = Math.random;
  const [f1, f2, p1, p2] = [0.6 + r() * 0.8, 2 + r() * 2, r() * 7, r() * 7];
  const start = r() < 0.5 ? 0 : 1;
  rip = { down, start, at: [], off: [], held: [], offFibre: [], heldFibre: [], front: 0, animations: [] };
  for (let i = 0; i <= n; i++) {
    const u = i / n;
    rip.at.push(i === 0 || i === n ? u : u + (r() - 0.5) / n);
    const wander = 2.6 + 2 * Math.sin(2 * Math.PI * f1 * u + p1) + Math.sin(2 * Math.PI * f2 * u + p2);
    rip.off.push(Math.max(0, wander + r() * 1.4 + (r() < 0.08 ? 1.5 + r() * 2 : 0)));
    rip.held.push(r() * 1.8 + (r() < 0.06 ? 1.5 : 0));
    rip.offFibre.push(1 + r() * 1.4);
    rip.heldFibre.push(1 + r() * 1.4);
  }
  rips.set(card, rip);
  card.dataset.rip = down ? 'down' : 'across';
  return rip;
}

/**
 * The clip-paths of both torn edges and their fibre bands with the rip run `front` of the way: behind the front the
 * edges are ragged and open a little; ahead of it the perforation is whole.
 */
function ripAt(rip: Rip, front: number) {
  const px = (v: number) => `${v.toFixed(2)}px`;
  const pt = (along: number, into: string) => {
    const a = `${(along * 100).toFixed(2)}%`;
    return rip.down ? `${into} ${a}` : `${a} ${into}`;
  };
  const off: string[] = [];
  const offFibre: string[] = [];
  const held: string[] = [];
  const heldFibre: string[] = [];
  rip.at.forEach((along, i) => {
    const behind = front - Math.abs(along - rip.start);
    const torn = Math.min(1, Math.max(0, behind / 0.12));
    const gap = 4 * Math.min(1, Math.max(0, behind / 0.4));
    const y = torn * rip.off[i] + gap * 0.6;
    const b = torn * rip.held[i] + gap * 0.4;
    off.push(pt(along, px(y)));
    offFibre.push(pt(along, px(y + torn * rip.offFibre[i])));
    held.push(pt(along, `calc(100% - ${px(b)})`));
    heldFibre.push(pt(along, px(8 - b - torn * rip.heldFibre[i])));
  });
  const [o, end, farEnd, far] = [pt(0, '0px'), pt(1, '0px'), pt(1, '100%'), pt(0, '100%')];
  return {
    off: `polygon(${off.join(', ')}, ${farEnd}, ${far})`,
    offFibre: `polygon(${o}, ${end}, ${offFibre.reverse().join(', ')})`,
    held: `polygon(${o}, ${end}, ${held.reverse().join(', ')})`,
    heldFibre: `polygon(${heldFibre.join(', ')}, ${farEnd}, ${far})`,
  };
}

/** The held half (body or main column) and the leaving half (foot or stub) of a ticket or pass. */
const halves = (card: HTMLElement) =>
  [
    card.querySelector<HTMLElement>('.ticket-body, .pass-main')!,
    card.querySelector<HTMLElement>('.tear, .stub')!,
  ] as const;

/** Runs `card`'s rip on to `to`: quick at first, catching about halfway, then through. */
async function runRip(card: HTMLElement, rip: Rip, to: number, duration: number) {
  const from = rip.front;
  const [held, off] = halves(card);
  const easing = 'cubic-bezier(0.16, 1, 0.3, 1)';
  const pace = [
    [0, 0],
    [0.3, 0.45],
    [0.5, 0.52],
    [1, 1],
  ].map(([offset, p]) => ({ offset, edges: ripAt(rip, from + (to - from) * p) }));
  const run = (el: HTMLElement, edge: keyof ReturnType<typeof ripAt>, pseudoElement?: string) =>
    el.animate(
      pace.map(({ offset, edges }) => ({ offset, easing, clipPath: edges[edge] })),
      { duration, fill: 'forwards', pseudoElement },
    );
  rip.animations.forEach((a) => a.cancel());
  rip.animations = [
    run(off, 'off'),
    run(off, 'offFibre', '::before'),
    run(held, 'held'),
    run(held, 'heldFibre', '::after'),
  ];
  rip.front = to;
  await rip.animations[0].finished;
}

/**
 * Rips the foot or stub off a ticket or pass along its perforation, on from any half tear, and resolves once it has
 * come away; at once under reduced motion. `down` when the perforation runs down the card (a wide pass).
 */
export async function tearOff(card: HTMLElement, down = false) {
  if (reducedMotion()) return;
  const rip = ripOf(card, down);
  await runRip(card, rip, 1.15, 260);
  // Torn through: the foot comes away, and the held half, let go, springs back.
  const [held, off] = halves(card);
  const turn = rip.start === 0 ? 1 : -1;
  const away = down ? `translate(0.5rem, 0) rotate(${-turn}deg)` : `translate(0, 0.5rem) rotate(${turn}deg)`;
  const back = down ? 'translateX(-1.5px)' : 'translateY(-1.5px)';
  const gone = off.animate([{ transform: 'none' }, { transform: away, opacity: 0 }], {
    duration: 140,
    easing: 'cubic-bezier(0.5, 0, 0.75, 0)',
    fill: 'forwards',
  });
  rip.animations.push(gone, held.animate([{}, { transform: back, offset: 0.4 }, {}], { duration: 240 }));
  await gone.finished;
}

/** Tears `card` partway, for a refused submit: half the first time, a little further each time after. */
export function tearHalf(card: HTMLElement, down = false) {
  if (reducedMotion()) return;
  const rip = ripOf(card, down);
  void runRip(card, rip, rip.front ? Math.min(rip.front + 0.12, 0.8) : 0.5, 280);
}

/** Makes `card` whole again, for a form that stays to issue the next one. */
export function untear(card: HTMLElement) {
  rips.get(card)?.animations.forEach((a) => a.cancel());
  rips.delete(card);
  delete card.dataset.rip;
}

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
