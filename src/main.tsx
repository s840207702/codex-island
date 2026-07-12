import React, { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { RefreshCw, Pin, Eye, EyeOff, X, CircleAlert, SlidersHorizontal } from "lucide-react";
import "./styles.css";
import "./overrides.css";

type WindowData = { used_percent: number; remaining_percent: number; reset_after_seconds: number; reset_at?: number | string | null };
type Usage = { primary: WindowData; secondary: WindowData; plan_type: string; plan_multiplier?: string | null; reset_credits?: number | null; reset_credit_expires_at?: number | string | null; credit_balance?: number | null; has_credits: boolean; fetched_at: string };
type Style = "overview" | "focus";

const compactTime = (seconds: number) => {
  const minutes = Math.max(0, Math.floor(seconds / 60));
  return minutes >= 60 ? `${Math.floor(minutes / 60)}h ${minutes % 60}m` : `${minutes}m`;
};
const resetText = (window: WindowData) => {
  if (window.reset_at) {
    const date = new Date(typeof window.reset_at === "number" ? window.reset_at * 1000 : window.reset_at);
    if (!Number.isNaN(date.getTime())) return `重置于 ${new Intl.DateTimeFormat("zh-CN", { weekday: "short", hour: "2-digit", minute: "2-digit" }).format(date)}`;
  }
  return `${compactTime(window.reset_after_seconds)} 后重置`;
};
const planLabel = (plan: string) => ({ plus: "Plus", pro: "Pro", business: "Business", team: "Team", enterprise: "Enterprise" }[plan.toLowerCase()] ?? plan.replace(/(^|[_-])(\w)/g, (_, __, char) => char.toUpperCase()));
const expiryText = (value?: number | string | null) => { if (!value) return null; const date = new Date(typeof value === "number" ? value * 1000 : value); return Number.isNaN(date.getTime()) ? null : `最早到期 ${new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(date)}`; };

function Ring({ label, window, tone, primary = false }: { label: string; window: WindowData; tone: "mint" | "amber" | "blue"; primary?: boolean }) {
  const progress = Math.max(0, Math.min(100, window.remaining_percent));
  return <section className={`ring-block ${primary ? "ring-block--hero" : ""}`}>
    <div className={`ring ring--${tone}`} style={{ "--progress": `${progress * 3.6}deg` } as React.CSSProperties}>
      <div className="ring__inside"><span>{label}</span><strong>{Math.round(progress)}%</strong><small>剩余</small></div>
    </div>
    <p>{resetText(window)}</p>
  </section>;
}

function App() {
  const [usage, setUsage] = useState<Usage | null>(null);
  const [style, setStyle] = useState<Style>(() => (localStorage.getItem("quota-island-style") as Style) || "overview");
  const [expanded, setExpanded] = useState(false);
  const [pinned, setPinned] = useState(() => localStorage.getItem("quota-island-pinned") === "true");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [opacity, setOpacity] = useState(() => Number(localStorage.getItem("codex-island-opacity") ?? "100"));
  const [closing, setClosing] = useState(false);
  const [stale, setStale] = useState(false);
  const failures = useRef(0);
  const collapseTimer = useRef<number | null>(null);
  const shrinkTimer = useRef<number | null>(null);

  const refresh = async () => {
    setLoading(true);
    try { setUsage(await invoke<Usage>("fetch_usage")); setError(null); setStale(false); failures.current = 0; }
    catch (e) { failures.current += 1; setError(e instanceof Error ? e.message : String(e)); setStale(Boolean(usage)); }
    finally { setLoading(false); }
  };
  useEffect(() => { void refresh(); }, []);
  useEffect(() => { const ms = failures.current === 0 ? 60_000 : Math.min(30 * 60_000, 30_000 * 2 ** (failures.current - 1)); const timer = window.setTimeout(refresh, ms); return () => window.clearTimeout(timer); }, [usage, stale]);
  useEffect(() => () => { if (collapseTimer.current) window.clearTimeout(collapseTimer.current); if (shrinkTimer.current) window.clearTimeout(shrinkTimer.current); }, []);
  useEffect(() => { localStorage.setItem("quota-island-style", style); }, [style]);
  // Pinning only controls auto-collapse. The island itself stays above other apps.
  useEffect(() => { localStorage.setItem("quota-island-pinned", String(pinned)); }, [pinned]);
  useEffect(() => { localStorage.setItem("codex-island-opacity", String(opacity)); document.documentElement.style.setProperty("--island-opacity", String(opacity / 100)); }, [opacity]);
  useEffect(() => { invoke("set_expanded", { expanded }).catch(() => undefined); }, [expanded]);
  const topText = useMemo(() => usage ? `${Math.round(usage.primary.remaining_percent)}% · ${compactTime(usage.primary.reset_after_seconds)}` : "正在同步", [usage]);
  const close = () => invoke("hide_window").catch(() => undefined);

  const openIsland = () => { if (collapseTimer.current) window.clearTimeout(collapseTimer.current); if (shrinkTimer.current) window.clearTimeout(shrinkTimer.current); setClosing(false); setExpanded(true); };
  const closeIslandLater = () => { if (!pinned) collapseTimer.current = window.setTimeout(() => { setClosing(true); shrinkTimer.current = window.setTimeout(() => { setExpanded(false); setClosing(false); shrinkTimer.current = null; }, 165); collapseTimer.current = null; }, 120); };
  return <main className={`island-shell ${expanded ? "island-shell--expanded" : ""}`} onPointerLeave={closeIslandLater} onPointerEnter={openIsland}>
    <button className="island-bar" onMouseDown={(event) => { if (event.button === 0) getCurrentWindow().startDragging(); }} onClick={() => setExpanded(v => !v)} aria-label="展开 Codex 额度">
      <i className={`live-dot ${error ? "live-dot--error" : ""}`} />
      <span className="brand-orbit" aria-hidden="true" />
      <b>Codex</b><span className="bar-summary">{topText}</span>
    </button>
    {expanded && <article className={`island-panel ${closing ? "island-panel--closing" : ""}`}>
      <header><div className="panel-brand"><span className="brand-orbit" /><strong>Codex Island</strong><span className="plan-label">{usage ? planLabel(usage.plan_type) : "—"}{usage?.plan_multiplier ? ` · ${usage.plan_multiplier}` : ""}</span></div><div className="controls">
        <div className="style-switch" role="group" aria-label="显示风格"><button className={style === "overview" ? "selected" : ""} onClick={() => setStyle("overview")}>概览</button><button className={style === "focus" ? "selected" : ""} onClick={() => setStyle("focus")}>专注</button></div>
        <button className={`icon-button ${pinned ? "icon-button--selected" : ""}`} onClick={() => setPinned(v => !v)} title={pinned ? "取消常驻" : "锁定常驻"}><Pin size={16} /></button><button className={`icon-button ${settingsOpen ? "icon-button--selected" : ""}`} onClick={() => setSettingsOpen(v => !v)} title="显示设置"><SlidersHorizontal size={16} /></button>
      </div></header>
      {settingsOpen && <section className="settings-popover"><div><b>窗口透明度</b><span>{opacity}%</span></div><input aria-label="窗口透明度" type="range" min="65" max="100" value={opacity} onChange={(event) => setOpacity(Number(event.target.value))} /><p>仅改变 Codex Island 的原生窗口，不影响桌面其他应用。</p></section>}
      {error && !usage ? <div className="error-state"><CircleAlert size={18} /><div><b>暂时无法同步</b><span>{error}</span></div><button onClick={refresh}>重试</button></div> : usage && (style === "overview" ?
        <div className="overview"><Ring label="5 小时" window={usage.primary} tone="mint" /><Ring label="本周" window={usage.secondary} tone="amber" /></div> :
        <div className="focus"><Ring label="5 小时额度" window={usage.primary} tone="blue" primary /><div className="weekly-line"><span className="weekly-line__dot" /><b>周额度</b><strong>{Math.round(usage.secondary.remaining_percent)}%</strong><em>{resetText(usage.secondary)}</em></div></div>)}
      <footer><span><i className={`live-dot ${stale ? "live-dot--error" : ""}`} />{loading ? "正在同步服务端…" : stale ? "显示上次成功数据 · 正在重试" : "刚刚同步 · 来源：OpenAI"}{usage?.reset_credits != null && <em className="reset-credit">重置机会 {usage.reset_credits}{expiryText(usage.reset_credit_expires_at) ? ` · ${expiryText(usage.reset_credit_expires_at)}` : ""}</em>}</span><div><button className="text-action" onClick={() => setPinned(v => !v)}>{pinned ? <><Eye size={14} /> 常驻</> : <><EyeOff size={14} /> 自动收起</>}</button><button className="icon-button" onClick={refresh} title="立即刷新"><RefreshCw size={16} className={loading ? "spinning" : ""} /></button><button className="icon-button" onClick={close} title="隐藏"><X size={16} /></button></div></footer>
    </article>}
  </main>;
}
createRoot(document.getElementById("root")!).render(<App />);
