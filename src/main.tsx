import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { RefreshCw, Pin, X, CircleAlert, LoaderCircle, Github, Layers2 } from "lucide-react";
import "./styles.css";
import "./overrides.css";

type WindowData = { used_percent: number; remaining_percent: number; reset_after_seconds: number; reset_at?: number | string | null };
type Usage = { primary: WindowData; secondary: WindowData; plan_type: string; plan_multiplier?: string | null; reset_credits?: number | null; reset_credit_expires_at?: number | string | null; credit_balance?: number | null; has_credits: boolean; fetched_at: string };
type ImmersiveState = { active: boolean };
const isDetailWindow = getCurrentWindow().label === "panel";

const compactTime = (seconds: number) => {
  const minutes = Math.max(0, Math.floor(seconds / 60));
  return minutes >= 60 ? `${Math.floor(minutes / 60)}h ${minutes % 60}m` : `${minutes}m`;
};
const resetDate = (window: WindowData) => {
  if (window.reset_at) {
    const date = new Date(typeof window.reset_at === "number" ? window.reset_at * 1000 : window.reset_at);
    if (!Number.isNaN(date.getTime())) return date;
  }
  return null;
};
const shortResetText = (window: WindowData) => {
  const date = resetDate(window); if (!date) return `${compactTime(window.reset_after_seconds)} 后重置`;
  const today = new Date(); today.setHours(0, 0, 0, 0); const tomorrow = new Date(today); tomorrow.setDate(tomorrow.getDate() + 1);
  const time = new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false }).format(date);
  if (date >= today && date < tomorrow) return `今天 ${time} 重置`;
  const dayAfter = new Date(tomorrow); dayAfter.setDate(dayAfter.getDate() + 1);
  if (date >= tomorrow && date < dayAfter) return `明天 ${time} 重置`;
  return `将于 ${new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false }).format(date)} 重置`;
};
const weeklyResetText = (window: WindowData) => {
  const date = resetDate(window); if (!date) return `${compactTime(window.reset_after_seconds)} 后重置`;
  return `将于 ${new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false }).format(date)} 重置`;
};
const planLabel = (plan: string) => ({ plus: "Plus", pro: "Pro", business: "Business", team: "Team", enterprise: "Enterprise" }[plan.toLowerCase()] ?? plan.replace(/(^|[_-])(\w)/g, (_, __, char) => char.toUpperCase()));
const expiryText = (value?: number | string | null) => { if (!value) return null; const date = new Date(typeof value === "number" ? value * 1000 : value); return Number.isNaN(date.getTime()) ? null : `最早到期 ${new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(date)}`; };

function Ring({ label, window, tone, period }: { label: string; window: WindowData; tone: "mint" | "amber"; period: "short" | "weekly" }) {
  const progress = Math.max(0, Math.min(100, window.remaining_percent));
  return <section className="ring-block">
    <div className={`ring ring--${tone}`} style={{ "--progress": `${progress * 3.6}deg` } as React.CSSProperties}>
      <div className="ring__inside"><span>{label}</span><strong>{Math.round(progress)}%</strong><small>剩余</small></div>
    </div>
    <p>{period === "short" ? shortResetText(window) : weeklyResetText(window)}</p>
  </section>;
}

function App() {
  const [usage, setUsage] = useState<Usage | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [pinned, setPinned] = useState(() => {
    // Restore hover-first behavior once for existing installs that used the former pin default.
    if (localStorage.getItem("codex-island-hover-contract-v2") !== "1") {
      localStorage.setItem("codex-island-hover-contract-v2", "1");
      localStorage.setItem("quota-island-pinned", "false");
      return false;
    }
    return localStorage.getItem("quota-island-pinned") === "true";
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [opacity, setOpacity] = useState(() => Number(localStorage.getItem("codex-island-opacity") ?? "100"));
  const [closing, setClosing] = useState(false);
  const [stale, setStale] = useState(false);
  const [exitConfirmOpen, setExitConfirmOpen] = useState(false);
  const [immersive, setImmersive] = useState(false);
  const [panelClosing, setPanelClosing] = useState(false);
  const failures = useRef(0);
  const collapseTimer = useRef<number | null>(null);
  const shrinkTimer = useRef<number | null>(null);
  const settingsTimer = useRef<number | null>(null);
  const immersiveTimer = useRef<number | null>(null);
  const immersiveCandidate = useRef<boolean | null>(null);
  const lastWindowBounds = useRef("");
  const islandRef = useRef<HTMLElement>(null);
  const barRef = useRef<HTMLButtonElement>(null);
  const dragOrigin = useRef<{ x: number; y: number } | null>(null);
  const dragging = useRef(false);

  const refresh = async () => {
    setLoading(true);
    try { setUsage(await invoke<Usage>("fetch_usage")); setError(null); setStale(false); failures.current = 0; }
    catch (e) { failures.current += 1; setError(e instanceof Error ? e.message : String(e)); setStale(Boolean(usage)); }
    finally { setLoading(false); }
  };
  useEffect(() => { void refresh(); }, []);
  useEffect(() => { const ms = failures.current === 0 ? 60_000 : Math.min(30 * 60_000, 30_000 * 2 ** (failures.current - 1)); const timer = window.setTimeout(refresh, ms); return () => window.clearTimeout(timer); }, [usage, stale]);
  useEffect(() => () => { if (collapseTimer.current) window.clearTimeout(collapseTimer.current); if (shrinkTimer.current) window.clearTimeout(shrinkTimer.current); if (settingsTimer.current) window.clearTimeout(settingsTimer.current); if (immersiveTimer.current) window.clearTimeout(immersiveTimer.current); }, []);
  // Pinning only controls auto-collapse. The island itself stays above other apps.
  useEffect(() => { localStorage.setItem("quota-island-pinned", String(pinned)); }, [pinned]);
  useEffect(() => { localStorage.setItem("codex-island-opacity", String(opacity)); document.documentElement.style.setProperty("--island-opacity", String(opacity / 100)); }, [opacity]);
  useLayoutEffect(() => {
    if (isDetailWindow) return;
    const target = expanded ? islandRef.current : barRef.current;
    if (!target) return;
    let frame = 0;
    const syncWindowBounds = () => {
      const bounds = target.getBoundingClientRect();
      const nextBounds = `${expanded}:${immersive}:${Math.ceil(bounds.width)}:${Math.ceil(bounds.height)}`;
      if (lastWindowBounds.current === nextBounds) return;
      lastWindowBounds.current = nextBounds;
      void invoke("set_expanded", { expanded, immersive, contentWidth: Math.ceil(bounds.width), contentHeight: Math.ceil(bounds.height) }).catch(() => undefined);
    };
    frame = window.requestAnimationFrame(syncWindowBounds);
    const observer = new ResizeObserver(syncWindowBounds);
    observer.observe(target);
    return () => { window.cancelAnimationFrame(frame); observer.disconnect(); };
  }, [expanded, immersive]);
  useEffect(() => {
    if (isDetailWindow) return;
    const schedule = (active: boolean) => {
      if (immersiveCandidate.current === active) return;
      immersiveCandidate.current = active;
      if (immersiveTimer.current) window.clearTimeout(immersiveTimer.current);
      immersiveTimer.current = window.setTimeout(() => {
        setImmersive(active);
        if (active) { setExpanded(false); setSettingsOpen(false); }
      }, active ? 260 : 160);
    };
    const check = () => { void invoke<ImmersiveState>("get_immersive_state").then((state) => schedule(state.active)).catch(() => schedule(false)); };
    check(); const timer = window.setInterval(check, 260);
    return () => window.clearInterval(timer);
  }, []);
  useEffect(() => {
    if (isDetailWindow) return;
    if (expanded && !immersive) {
      void emit("codex-island-detail-closing", false);
      void invoke("show_detail_panel").catch(() => undefined);
    } else {
      void invoke("hide_detail_panel").catch(() => undefined);
    }
  }, [expanded, immersive]);
  useEffect(() => {
    if (!isDetailWindow) return;
    let dispose: (() => void) | undefined;
    void listen<boolean>("codex-island-detail-closing", (event) => setPanelClosing(event.payload)).then((unlisten) => { dispose = unlisten; });
    return () => dispose?.();
  }, []);
  useEffect(() => {
    if (isDetailWindow) return;
    let disposeHover: (() => void) | undefined;
    let disposePin: (() => void) | undefined;
    void Promise.all([
      listen<boolean>("codex-island-detail-hover", (event) => { if (event.payload) openIsland(); else closeIslandLater(); }),
      listen<boolean>("codex-island-detail-pin", (event) => setPinned(event.payload)),
    ]).then(([hover, pin]) => { disposeHover = hover; disposePin = pin; });
    return () => { disposeHover?.(); disposePin?.(); };
  }, []);
  useEffect(() => {
    if (isDetailWindow || !expanded || pinned || immersive) return;
    const checkCursor = () => {
      void invoke<boolean>("is_cursor_over_island").then((overIsland) => {
        if (overIsland) { if (collapseTimer.current) window.clearTimeout(collapseTimer.current); collapseTimer.current = null; }
        else closeIslandLater();
      }).catch(() => undefined);
    };
    checkCursor();
    const timer = window.setInterval(checkCursor, 90);
    return () => window.clearInterval(timer);
  }, [expanded, pinned, immersive]);
  const topText = useMemo(() => usage ? `${Math.round(usage.primary.remaining_percent)}% · ${compactTime(usage.primary.reset_after_seconds)}` : "—", [usage]);
  const quit = () => invoke("exit_app").catch(() => undefined);
  const openExternal = (url: string) => { void openUrl(url).catch(() => undefined); };

  const openIsland = () => { if (immersive) return; if (collapseTimer.current) window.clearTimeout(collapseTimer.current); if (shrinkTimer.current) window.clearTimeout(shrinkTimer.current); setClosing(false); void emit("codex-island-detail-closing", false); setExpanded(true); };
  const closeIslandLater = () => {
    if (pinned || collapseTimer.current || shrinkTimer.current) return;
    collapseTimer.current = window.setTimeout(() => {
      void invoke<boolean>("is_cursor_over_island").then((overIsland) => {
        collapseTimer.current = null;
        if (overIsland) return;
        setClosing(true); void emit("codex-island-detail-closing", true);
        shrinkTimer.current = window.setTimeout(() => { setExpanded(false); setClosing(false); shrinkTimer.current = null; }, 185);
      }).catch(() => { collapseTimer.current = null; });
    }, 220);
  };
  const keepSettingsOpen = () => { if (settingsTimer.current) window.clearTimeout(settingsTimer.current); };
  const hideSettingsLater = () => { settingsTimer.current = window.setTimeout(() => { setSettingsOpen(false); settingsTimer.current = null; }, 480); };
  const beginPotentialDrag = (event: React.MouseEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    if (event.button !== 0 || target.closest("input, button:not(.island-bar), .confirm-dialog")) return;
    dragOrigin.current = { x: event.clientX, y: event.clientY };
    dragging.current = false;
  };
  const continuePotentialDrag = (event: React.MouseEvent<HTMLElement>) => {
    const origin = dragOrigin.current;
    if (!origin || dragging.current || (event.buttons & 1) === 0) return;
    if (Math.hypot(event.clientX - origin.x, event.clientY - origin.y) < 4) return;
    dragging.current = true;
    void invoke("start_window_drag");
  };
  const finishPotentialDrag = () => {
    if (dragging.current) void invoke("save_window_position").catch(() => undefined);
    window.setTimeout(() => { dragOrigin.current = null; dragging.current = false; }, 0);
  };
  const detail = <article className={`island-panel ${isDetailWindow ? "island-panel--window" : ""} ${closing || panelClosing ? "island-panel--closing" : ""}`}>
      <header><div className="panel-brand"><span className="brand-orbit" /><strong>Codex Island</strong><span className="plan-label">{usage ? planLabel(usage.plan_type) : "—"}{usage?.plan_multiplier ? ` · ${usage.plan_multiplier}` : ""}</span></div><div className="controls">
        <button className="icon-button icon-button--external" onClick={() => openExternal("https://github.com/s840207702/codex-island")} title="在 GitHub 查看 Codex Island" aria-label="在 GitHub 查看 Codex Island"><Github size={16} /></button><button className="icon-button icon-button--avatar" onClick={() => openExternal("https://www.feige177.com")} title="打开非哥工具箱" aria-label="打开非哥工具箱"><img src="/feige-toolbox-avatar.png" alt="" /></button><span className="control-divider" aria-hidden="true" /><button className={`icon-button ${pinned ? "icon-button--selected" : ""}`} onClick={() => setPinned(v => { const next = !v; if (isDetailWindow) void emit("codex-island-detail-pin", next); return next; })} title={pinned ? "取消常驻" : "锁定常驻"}><Pin size={16} /></button><button className={`icon-button icon-button--opacity ${settingsOpen ? "icon-button--selected" : ""}`} onPointerEnter={keepSettingsOpen} onPointerLeave={hideSettingsLater} onClick={() => setSettingsOpen(v => !v)} title="窗口透明度" aria-label="窗口透明度"><Layers2 size={17} strokeWidth={1.8} /></button>
      </div></header>
      {settingsOpen && <section onPointerEnter={keepSettingsOpen} onPointerLeave={hideSettingsLater} className="settings-popover" aria-label="窗口透明度"><span>{opacity}%</span><input aria-label="窗口透明度" type="range" min="65" max="100" value={opacity} onChange={(event) => setOpacity(Number(event.target.value))} /></section>}
      {error && !usage ? <div className="error-state"><CircleAlert size={18} /><div><b>暂时无法同步</b><span>{error}</span></div><button onClick={refresh}>重试</button></div> : usage && <div className="overview"><Ring label="5 小时" window={usage.primary} tone="mint" period="short" /><Ring label="本周" window={usage.secondary} tone="amber" period="weekly" /></div>}
      <footer className="status-rail"><span className="status-source">{loading ? <LoaderCircle className="spinning sync-spinner" size={15} /> : <i className={`live-dot ${stale ? "live-dot--error" : ""}`} />}{stale ? "上次成功数据 · 重试中" : !loading && "OpenAI · 刚刚同步"}</span>{usage?.reset_credits != null && <em className="reset-credit">重置 {usage.reset_credits}{expiryText(usage.reset_credit_expires_at) ? ` · ${expiryText(usage.reset_credit_expires_at)?.replace("最早到期 ", "")}` : ""}</em>}<div><button className="icon-button" onClick={refresh} title="立即刷新"><RefreshCw size={16} className={loading ? "spinning" : ""} /></button><button className="icon-button" onClick={() => setExitConfirmOpen(true)} title="退出 Codex Island" aria-label="退出 Codex Island"><X size={16} /></button></div></footer>
      {exitConfirmOpen && <section className="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="exit-dialog-title"><div className="confirm-dialog__card"><span className="confirm-dialog__eyebrow">退出确认</span><strong id="exit-dialog-title">要完全退出 Codex Island 吗？</strong><p>退出后将停止额度同步和桌面悬浮显示。</p><div><button className="text-action" onClick={() => setExitConfirmOpen(false)}>取消</button><button className="confirm-dialog__exit" onClick={quit}>退出应用</button></div></div></section>}
    </article>;
  if (isDetailWindow) return <main className="detail-hitbox" onPointerEnter={() => void emit("codex-island-detail-hover", true)} onPointerLeave={() => void emit("codex-island-detail-hover", false)}>{detail}</main>;
  return <main ref={islandRef} className={`island-shell ${expanded ? "island-shell--active" : ""} ${immersive ? "island-shell--immersive" : ""}`} onPointerEnter={openIsland} onPointerLeave={closeIslandLater} onMouseDownCapture={beginPotentialDrag} onMouseMoveCapture={continuePotentialDrag} onMouseUpCapture={finishPotentialDrag}>
    <button ref={barRef} className="island-bar" onClick={() => { if (!immersive && !dragging.current) setExpanded(v => !v); }} aria-label={immersive ? "沉浸模式额度" : "展开 Codex 额度"}>
      <span className="bar-identity"><i className={`live-dot ${error ? "live-dot--error" : ""}`} /><span className="brand-orbit" aria-hidden="true" /><b>Codex</b></span><span className="bar-summary">{loading ? <LoaderCircle className="spinning sync-spinner" size={15} /> : <span className="bar-summary__value">{topText}</span>}</span>
    </button>
  </main>;
}
createRoot(document.getElementById("root")!).render(<App />);
