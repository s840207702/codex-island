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
type Usage = { weekly: WindowData; plan_type: string; plan_multiplier?: string | null; reset_credits?: number | null; reset_credit_expires_at?: number | string | null; credit_balance?: number | null; has_credits: boolean; fetched_at: string };
// Legacy dual-window fields, retained for a possible 5-hour quota restoration:
// type Usage = { primary: WindowData; secondary: WindowData; ... };
type ImmersiveState = { active: boolean };
type QuotaTemperature = { color: string; urgent: boolean; critical: boolean };
const supportedLocales = ["zh-CN", "zh-TW", "en", "ja", "ko", "es", "fr", "de", "pt-BR", "ru"] as const;
type Locale = typeof supportedLocales[number];
const isLocale = (value: unknown): value is Locale => typeof value === "string" && supportedLocales.includes(value as Locale);
type AppCopy = {
  resetAfter: (time: string) => string; todayReset: (time: string) => string; tomorrowReset: (time: string) => string; willReset: (time: string) => string; earliestExpiry: (time: string) => string;
  remaining: string; github: string; toolbox: string; pin: string; unpin: string; opacity: string; syncUnavailable: string; retry: string; fiveHours: string; thisWeek: string;
  staleRetry: string; synced: string; resetCredits: string; refresh: string; exit: string; exitConfirm: string; exitQuestion: string; exitBody: string; cancel: string; exitApp: string; immersiveQuota: string; openQuota: string;
};
const translations: Record<Locale, AppCopy> = {
  "zh-CN": { resetAfter: time => `${time} 后重置`, todayReset: time => `今天 ${time} 重置`, tomorrowReset: time => `明天 ${time} 重置`, willReset: time => `将于 ${time} 重置`, earliestExpiry: time => `最早到期 ${time}`, remaining: "剩余", github: "在 GitHub 查看 Codex Island", toolbox: "打开非哥工具箱", pin: "锁定常驻", unpin: "取消常驻", opacity: "窗口透明度", syncUnavailable: "暂时无法同步", retry: "重试", fiveHours: "5 小时", thisWeek: "本周", staleRetry: "上次成功数据 · 重试中", synced: "OpenAI · 刚刚同步", resetCredits: "重置", refresh: "立即刷新", exit: "退出 Codex Island", exitConfirm: "退出确认", exitQuestion: "要完全退出 Codex Island 吗？", exitBody: "退出后将停止额度同步和桌面悬浮显示。", cancel: "取消", exitApp: "退出应用", immersiveQuota: "沉浸模式额度", openQuota: "展开 Codex 额度" },
  en: { resetAfter: time => `Resets in ${time}`, todayReset: time => `Resets today at ${time}`, tomorrowReset: time => `Resets tomorrow at ${time}`, willReset: time => `Resets ${time}`, earliestExpiry: time => `Earliest expiry ${time}`, remaining: "left", github: "View Codex Island on GitHub", toolbox: "Open Feige Toolbox", pin: "Keep expanded", unpin: "Stop keeping expanded", opacity: "Window opacity", syncUnavailable: "Unable to sync", retry: "Retry", fiveHours: "5 hours", thisWeek: "This week", staleRetry: "Last successful data · retrying", synced: "OpenAI · synced just now", resetCredits: "Resets", refresh: "Refresh now", exit: "Quit Codex Island", exitConfirm: "Confirm quit", exitQuestion: "Quit Codex Island completely?", exitBody: "Quota syncing and the floating island will stop.", cancel: "Cancel", exitApp: "Quit app", immersiveQuota: "Immersive quota", openQuota: "Open Codex quota" },
  ja: { resetAfter: time => `${time}後にリセット`, todayReset: time => `今日 ${time} にリセット`, tomorrowReset: time => `明日 ${time} にリセット`, willReset: time => `${time} にリセット`, earliestExpiry: time => `最短有効期限 ${time}`, remaining: "残り", github: "GitHub で Codex Island を表示", toolbox: "非哥ツールボックスを開く", pin: "展開を固定", unpin: "固定を解除", opacity: "ウィンドウ透明度", syncUnavailable: "同期できません", retry: "再試行", fiveHours: "5時間", thisWeek: "今週", staleRetry: "前回のデータ · 再試行中", synced: "OpenAI · 同期済み", resetCredits: "リセット", refresh: "今すぐ更新", exit: "Codex Island を終了", exitConfirm: "終了の確認", exitQuestion: "Codex Island を完全に終了しますか？", exitBody: "使用量の同期とフローティング表示が停止します。", cancel: "キャンセル", exitApp: "アプリを終了", immersiveQuota: "没入モード使用量", openQuota: "Codex 使用量を開く" },
  ko: { resetAfter: time => `${time} 후 재설정`, todayReset: time => `오늘 ${time} 재설정`, tomorrowReset: time => `내일 ${time} 재설정`, willReset: time => `${time} 재설정`, earliestExpiry: time => `가장 빠른 만료 ${time}`, remaining: "남음", github: "GitHub에서 Codex Island 보기", toolbox: "非哥 도구 상자 열기", pin: "펼침 고정", unpin: "고정 해제", opacity: "창 투명도", syncUnavailable: "동기화할 수 없음", retry: "다시 시도", fiveHours: "5시간", thisWeek: "이번 주", staleRetry: "마지막 성공 데이터 · 재시도 중", synced: "OpenAI · 방금 동기화", resetCredits: "재설정", refresh: "지금 새로고침", exit: "Codex Island 종료", exitConfirm: "종료 확인", exitQuestion: "Codex Island를 완전히 종료할까요?", exitBody: "사용량 동기화와 플로팅 표시가 중지됩니다.", cancel: "취소", exitApp: "앱 종료", immersiveQuota: "몰입 모드 사용량", openQuota: "Codex 사용량 열기" },
  "zh-TW": { resetAfter: time => `${time} 後重置`, todayReset: time => `今天 ${time} 重置`, tomorrowReset: time => `明天 ${time} 重置`, willReset: time => `將於 ${time} 重置`, earliestExpiry: time => `最早到期 ${time}`, remaining: "剩餘", github: "在 GitHub 查看 Codex Island", toolbox: "開啟非哥工具箱", pin: "鎖定常駐", unpin: "取消常駐", opacity: "視窗透明度", syncUnavailable: "暫時無法同步", retry: "重試", fiveHours: "5 小時", thisWeek: "本週", staleRetry: "上次成功資料 · 重試中", synced: "OpenAI · 剛剛同步", resetCredits: "重置", refresh: "立即重新整理", exit: "結束 Codex Island", exitConfirm: "結束確認", exitQuestion: "要完全結束 Codex Island 嗎？", exitBody: "結束後將停止額度同步和桌面浮動顯示。", cancel: "取消", exitApp: "結束應用程式", immersiveQuota: "沉浸模式額度", openQuota: "展開 Codex 額度" },
  es: { resetAfter: time => `Se restablece en ${time}`, todayReset: time => `Se restablece hoy a las ${time}`, tomorrowReset: time => `Se restablece mañana a las ${time}`, willReset: time => `Se restablece el ${time}`, earliestExpiry: time => `Caduca primero el ${time}`, remaining: "restante", github: "Ver Codex Island en GitHub", toolbox: "Abrir Feige Toolbox", pin: "Mantener abierto", unpin: "Dejar de mantener abierto", opacity: "Opacidad de la ventana", syncUnavailable: "No se puede sincronizar", retry: "Reintentar", fiveHours: "5 horas", thisWeek: "Esta semana", staleRetry: "Últimos datos · reintentando", synced: "OpenAI · sincronizado", resetCredits: "Restablecimientos", refresh: "Actualizar ahora", exit: "Salir de Codex Island", exitConfirm: "Confirmar salida", exitQuestion: "¿Salir completamente de Codex Island?", exitBody: "Se detendrán la sincronización y la isla flotante.", cancel: "Cancelar", exitApp: "Salir", immersiveQuota: "Cuota inmersiva", openQuota: "Abrir cuota de Codex" },
  fr: { resetAfter: time => `Réinitialisation dans ${time}`, todayReset: time => `Réinitialisation aujourd’hui à ${time}`, tomorrowReset: time => `Réinitialisation demain à ${time}`, willReset: time => `Réinitialisation le ${time}`, earliestExpiry: time => `Première expiration le ${time}`, remaining: "restant", github: "Voir Codex Island sur GitHub", toolbox: "Ouvrir Feige Toolbox", pin: "Garder ouvert", unpin: "Ne plus garder ouvert", opacity: "Opacité de la fenêtre", syncUnavailable: "Synchronisation impossible", retry: "Réessayer", fiveHours: "5 heures", thisWeek: "Cette semaine", staleRetry: "Dernières données · nouvel essai", synced: "OpenAI · synchronisé", resetCredits: "Réinitialisations", refresh: "Actualiser", exit: "Quitter Codex Island", exitConfirm: "Confirmer la fermeture", exitQuestion: "Quitter complètement Codex Island ?", exitBody: "La synchronisation et l’île flottante s’arrêteront.", cancel: "Annuler", exitApp: "Quitter", immersiveQuota: "Quota immersif", openQuota: "Ouvrir le quota Codex" },
  de: { resetAfter: time => `Zurücksetzung in ${time}`, todayReset: time => `Heute um ${time} zurückgesetzt`, tomorrowReset: time => `Morgen um ${time} zurückgesetzt`, willReset: time => `Zurücksetzung am ${time}`, earliestExpiry: time => `Frühester Ablauf ${time}`, remaining: "verbleibend", github: "Codex Island auf GitHub ansehen", toolbox: "Feige Toolbox öffnen", pin: "Geöffnet halten", unpin: "Nicht mehr geöffnet halten", opacity: "Fensterdeckkraft", syncUnavailable: "Synchronisierung nicht möglich", retry: "Erneut versuchen", fiveHours: "5 Stunden", thisWeek: "Diese Woche", staleRetry: "Letzte Daten · neuer Versuch", synced: "OpenAI · synchronisiert", resetCredits: "Zurücksetzungen", refresh: "Jetzt aktualisieren", exit: "Codex Island beenden", exitConfirm: "Beenden bestätigen", exitQuestion: "Codex Island vollständig beenden?", exitBody: "Synchronisierung und schwebende Anzeige werden beendet.", cancel: "Abbrechen", exitApp: "Beenden", immersiveQuota: "Fokus-Kontingent", openQuota: "Codex-Kontingent öffnen" },
  "pt-BR": { resetAfter: time => `Redefine em ${time}`, todayReset: time => `Redefine hoje às ${time}`, tomorrowReset: time => `Redefine amanhã às ${time}`, willReset: time => `Redefine em ${time}`, earliestExpiry: time => `Expira primeiro em ${time}`, remaining: "restante", github: "Ver Codex Island no GitHub", toolbox: "Abrir Feige Toolbox", pin: "Manter aberto", unpin: "Parar de manter aberto", opacity: "Opacidade da janela", syncUnavailable: "Não foi possível sincronizar", retry: "Tentar novamente", fiveHours: "5 horas", thisWeek: "Esta semana", staleRetry: "Últimos dados · tentando novamente", synced: "OpenAI · sincronizado", resetCredits: "Redefinições", refresh: "Atualizar agora", exit: "Sair do Codex Island", exitConfirm: "Confirmar saída", exitQuestion: "Sair completamente do Codex Island?", exitBody: "A sincronização e a ilha flutuante serão encerradas.", cancel: "Cancelar", exitApp: "Sair", immersiveQuota: "Cota imersiva", openQuota: "Abrir cota do Codex" },
  ru: { resetAfter: time => `Сброс через ${time}`, todayReset: time => `Сброс сегодня в ${time}`, tomorrowReset: time => `Сброс завтра в ${time}`, willReset: time => `Сброс ${time}`, earliestExpiry: time => `Ближайшее истечение ${time}`, remaining: "осталось", github: "Открыть Codex Island на GitHub", toolbox: "Открыть Feige Toolbox", pin: "Оставить открытым", unpin: "Не оставлять открытым", opacity: "Прозрачность окна", syncUnavailable: "Не удалось синхронизировать", retry: "Повторить", fiveHours: "5 часов", thisWeek: "Эта неделя", staleRetry: "Последние данные · повтор", synced: "OpenAI · синхронизировано", resetCredits: "Сбросы", refresh: "Обновить", exit: "Выйти из Codex Island", exitConfirm: "Подтверждение выхода", exitQuestion: "Полностью закрыть Codex Island?", exitBody: "Синхронизация и плавающий остров будут остановлены.", cancel: "Отмена", exitApp: "Выйти", immersiveQuota: "Иммерсивная квота", openQuota: "Открыть квоту Codex" },
};
const hasTauriRuntime = "__TAURI_INTERNALS__" in window;
const isMacPlatform = /Macintosh|Mac OS X/.test(navigator.userAgent) || navigator.platform.toLowerCase().startsWith("mac");
if (isMacPlatform) document.documentElement.classList.add("platform-macos");
const previewMode = import.meta.env.DEV ? new URLSearchParams(window.location.search).get("preview") : null;
const isWeeklyPreview = previewMode === "weekly" || previewMode === "weekly-panel";
const isDetailWindow = hasTauriRuntime ? getCurrentWindow().label === "panel" : previewMode === "weekly-panel";
const previewUsage: Usage = { weekly: { used_percent: 2, remaining_percent: 98, reset_after_seconds: 595621, reset_at: 1784506992 }, plan_type: "pro", has_credits: false, fetched_at: "preview" };
if (isWeeklyPreview) document.documentElement.classList.add("weekly-preview");

const compactTime = (seconds: number) => {
  const minutes = Math.max(0, Math.floor(seconds / 60));
  return minutes >= 60 ? `${Math.floor(minutes / 60)}h ${minutes % 60}m` : `${minutes}m`;
};
const compactWeeklyTime = (seconds: number) => {
  const hours = Math.max(0, Math.floor(seconds / 3600));
  return hours >= 24 ? `${Math.floor(hours / 24)}d ${hours % 24}h` : compactTime(seconds);
};
const mixColor = (from: [number, number, number], to: [number, number, number], amount: number) => `rgb(${from.map((channel, index) => Math.round(channel + (to[index] - channel) * amount)).join(" ")})`;
const quotaTemperature = (remaining: number): QuotaTemperature => {
  const value = Math.max(0, Math.min(100, remaining));
  const stops: Array<[number, [number, number, number]]> = [
    [0, [153, 55, 65]], [10, [226, 97, 99]], [20, [225, 132, 76]],
    [35, [211, 168, 74]], [60, [91, 202, 139]], [100, [83, 220, 144]],
  ];
  const foundIndex = stops.findIndex(([stop]) => value <= stop);
  const upperIndex = foundIndex < 1 ? 1 : foundIndex;
  const [lowerStop, lowerColor] = stops[upperIndex - 1];
  const [upperStop, upperColor] = stops[upperIndex];
  return { color: mixColor(lowerColor, upperColor, (value - lowerStop) / (upperStop - lowerStop)), urgent: value < 20, critical: value < 10 };
};
const resetDate = (window: WindowData) => {
  if (window.reset_at) {
    const date = new Date(typeof window.reset_at === "number" ? window.reset_at * 1000 : window.reset_at);
    if (!Number.isNaN(date.getTime())) return date;
  }
  return null;
};
const shortResetText = (window: WindowData, locale: Locale, copy: AppCopy) => {
  const date = resetDate(window); if (!date) return copy.resetAfter(compactTime(window.reset_after_seconds));
  const today = new Date(); today.setHours(0, 0, 0, 0); const tomorrow = new Date(today); tomorrow.setDate(tomorrow.getDate() + 1);
  const time = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit", hour12: false }).format(date);
  if (date >= today && date < tomorrow) return copy.todayReset(time);
  const dayAfter = new Date(tomorrow); dayAfter.setDate(dayAfter.getDate() + 1);
  if (date >= tomorrow && date < dayAfter) return copy.tomorrowReset(time);
  return copy.willReset(new Intl.DateTimeFormat(locale, { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false }).format(date));
};
const weeklyResetText = (window: WindowData, locale: Locale, copy: AppCopy) => {
  const date = resetDate(window); if (!date) return copy.resetAfter(compactTime(window.reset_after_seconds));
  return copy.willReset(new Intl.DateTimeFormat(locale, { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false }).format(date));
};
const planLabel = (plan: string) => ({ plus: "Plus", pro: "Pro", business: "Business", team: "Team", enterprise: "Enterprise" }[plan.toLowerCase()] ?? plan.replace(/(^|[_-])(\w)/g, (_, __, char) => char.toUpperCase()));
const paidPlanClass = (plan: string | undefined) => plan && ["plus", "pro"].includes(plan.toLowerCase()) ? "plan-label--gold" : "";
const expiryText = (value: number | string | null | undefined, locale: Locale, copy: AppCopy) => { if (!value) return null; const date = new Date(typeof value === "number" ? value * 1000 : value); return Number.isNaN(date.getTime()) ? null : copy.earliestExpiry(new Intl.DateTimeFormat(locale, { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(date)); };

function Ring({ label, window, period, locale, copy }: { label: string; window: WindowData; period: "short" | "weekly"; locale: Locale; copy: AppCopy }) {
  const progress = Math.max(0, Math.min(100, window.remaining_percent));
  const temperature = quotaTemperature(progress);
  return <section className="ring-block">
    <div className={`ring ${temperature.urgent ? "ring--urgent" : ""}`} style={{ "--progress": `${progress * 3.6}deg`, "--ring": temperature.color, "--quota-color": temperature.color } as React.CSSProperties}>
      <div className="ring__inside"><span>{label}</span><strong>{Math.round(progress)}%</strong><small>{copy.remaining}</small></div>
    </div>
    <p>{period === "short" ? shortResetText(window, locale, copy) : weeklyResetText(window, locale, copy)}</p>
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
  const [locale, setLocale] = useState<Locale>(() => {
    const saved = localStorage.getItem("codex-island-language");
    return isLocale(saved) ? saved : "zh-CN";
  });
  const copy = translations[locale];
  const hasUsage = useRef(false);
  const collapseTimer = useRef<number | null>(null);
  const shrinkTimer = useRef<number | null>(null);
  const settingsTimer = useRef<number | null>(null);
  const immersiveTimer = useRef<number | null>(null);
  const immersiveCandidate = useRef<boolean | null>(null);
  const lastWindowBounds = useRef("");
  const islandRef = useRef<HTMLElement>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const dragOrigin = useRef<{ x: number; y: number } | null>(null);
  const dragging = useRef(false);
  const cursorInsideIsland = useRef(false);

  const refresh = async (force = false) => {
    setLoading(true);
    try {
      const nextUsage = isWeeklyPreview ? previewUsage : await invoke<Usage>("fetch_usage", { force });
      hasUsage.current = true;
      setUsage(nextUsage);
      setError(null); setStale(false);
    }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); setStale(hasUsage.current); }
    finally { setLoading(false); }
  };
  useEffect(() => { if (!isDetailWindow) void refresh(false); }, []);
  useEffect(() => {
    if (isWeeklyPreview) return;
    let active = true;
    const syncLocale = () => {
      void invoke<string>("get_app_language").then((value) => {
        if (active && isLocale(value)) setLocale((current) => current === value ? current : value);
      }).catch(() => undefined);
    };
    const handleVisibility = () => { if (!document.hidden) syncLocale(); };
    syncLocale();
    document.addEventListener("visibilitychange", handleVisibility);
    window.addEventListener("focus", syncLocale);
    // Hidden WebView2 windows can miss native events. The lightweight backend
    // preference check makes the persisted language the single source of truth.
    const timer = window.setInterval(syncLocale, isDetailWindow ? 300 : 1500);
    return () => {
      active = false;
      document.removeEventListener("visibilitychange", handleVisibility);
      window.removeEventListener("focus", syncLocale);
      window.clearInterval(timer);
    };
  }, []);
  useEffect(() => {
    if (isWeeklyPreview) return;
    let disposeLanguage: (() => void) | undefined;
    let disposeRefresh: (() => void) | undefined;
    let disposeUsage: (() => void) | undefined;
    let disposeUsageError: (() => void) | undefined;
    const handleDomLanguage = (event: Event) => {
      const value = (event as CustomEvent<unknown>).detail;
      if (isLocale(value)) setLocale(value);
    };
    window.addEventListener("codex-island-language-dom-change", handleDomLanguage);
    const handlePanelShown = () => { if (isDetailWindow) void refresh(false); };
    window.addEventListener("codex-island-panel-shown", handlePanelShown);
    void Promise.all([
      listen<string>("codex-island-language-change", (event) => { if (isLocale(event.payload)) setLocale(event.payload); }),
      listen("codex-island-refresh", () => { if (!isDetailWindow) void refresh(true); }),
      listen<Usage>("codex-island-usage-change", (event) => { hasUsage.current = true; setUsage(event.payload); setError(null); setStale(false); setLoading(false); }),
      listen<string>("codex-island-usage-error", (event) => { setError(event.payload); setStale(hasUsage.current); setLoading(false); }),
    ]).then(([languageListener, refreshListener, usageListener, usageErrorListener]) => { disposeLanguage = languageListener; disposeRefresh = refreshListener; disposeUsage = usageListener; disposeUsageError = usageErrorListener; });
    return () => { window.removeEventListener("codex-island-language-dom-change", handleDomLanguage); window.removeEventListener("codex-island-panel-shown", handlePanelShown); disposeLanguage?.(); disposeRefresh?.(); disposeUsage?.(); disposeUsageError?.(); };
  }, []);
  useEffect(() => { localStorage.setItem("codex-island-language", locale); document.documentElement.lang = locale; }, [locale]);
  useEffect(() => {
    const blockContextMenu = (event: MouseEvent) => event.preventDefault();
    document.addEventListener("contextmenu", blockContextMenu);
    return () => document.removeEventListener("contextmenu", blockContextMenu);
  }, []);
  useEffect(() => () => { if (collapseTimer.current) window.clearTimeout(collapseTimer.current); if (shrinkTimer.current) window.clearTimeout(shrinkTimer.current); if (settingsTimer.current) window.clearTimeout(settingsTimer.current); if (immersiveTimer.current) window.clearTimeout(immersiveTimer.current); }, []);
  // Pinning only controls auto-collapse. The island itself stays above other apps.
  useEffect(() => { localStorage.setItem("quota-island-pinned", String(pinned)); }, [pinned]);
  useEffect(() => { localStorage.setItem("codex-island-opacity", String(opacity)); document.documentElement.style.setProperty("--island-opacity", String(opacity / 100)); }, [opacity]);
  useEffect(() => {
    if (isWeeklyPreview) return;
    let dispose: (() => void) | undefined;
    void listen<number>("codex-island-opacity-change", (event) => setOpacity(Math.max(65, Math.min(100, event.payload)))).then((unlisten) => { dispose = unlisten; });
    return () => dispose?.();
  }, []);
  useLayoutEffect(() => {
    if (isDetailWindow) return;
    const target = expanded ? islandRef.current : barRef.current;
    if (!target) return;
    let frame = 0;
    const syncWindowBounds = () => {
      const bounds = target.getBoundingClientRect();
      // Vite/React hot reload can briefly detach the measured node. Never let
      // that transient zero-sized frame collapse the native Tauri window.
      if (bounds.width < 100 || bounds.height < 30) return;
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
    if (isDetailWindow || isWeeklyPreview) return;
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
    if (isDetailWindow || isWeeklyPreview) return;
    if (expanded && !immersive) {
      void emit("codex-island-detail-closing", false);
      void invoke("show_detail_panel").catch(() => undefined);
    } else {
      void invoke("hide_detail_panel").catch(() => undefined);
    }
  }, [expanded, immersive]);
  useEffect(() => {
    if (!isDetailWindow || isWeeklyPreview) return;
    let dispose: (() => void) | undefined;
    void listen<boolean>("codex-island-detail-closing", (event) => {
      setPanelClosing(event.payload);
      if (!event.payload) {
        void invoke<string>("get_app_language").then((value) => { if (isLocale(value)) setLocale(value); }).catch(() => undefined);
      }
    }).then((unlisten) => { dispose = unlisten; });
    return () => dispose?.();
  }, []);
  useEffect(() => {
    if (isDetailWindow || isWeeklyPreview) return;
    let disposeHover: (() => void) | undefined;
    let disposePin: (() => void) | undefined;
    void Promise.all([
      listen<boolean>("codex-island-detail-hover", (event) => { if (event.payload) openIsland(); else closeIslandLater(); }),
      listen<boolean>("codex-island-detail-pin", (event) => setPinned(event.payload)),
    ]).then(([hover, pin]) => { disposeHover = hover; disposePin = pin; });
    return () => { disposeHover?.(); disposePin?.(); };
  }, []);
  useEffect(() => {
    if (isDetailWindow || isWeeklyPreview) return;
    const handleWindowBlur = () => { if (expanded && !immersive) closeIslandLater(); };
    window.addEventListener("blur", handleWindowBlur);
    return () => window.removeEventListener("blur", handleWindowBlur);
  }, [expanded, immersive, pinned]);
  const topQuota = useMemo(() => {
    if (!usage) return null;
    return { quota: usage.weekly, temperature: quotaTemperature(usage.weekly.remaining_percent) };
  }, [usage]);
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
        shrinkTimer.current = window.setTimeout(() => { setExpanded(false); setClosing(false); shrinkTimer.current = null; }, 165);
      }).catch(() => { collapseTimer.current = null; });
    }, 150);
  };
  useEffect(() => {
    if (isDetailWindow || isWeeklyPreview) return;
    const checkCursor = () => {
      if (immersive) { cursorInsideIsland.current = false; return; }
      void invoke<boolean>("is_cursor_over_island").then((overIsland) => {
        if (overIsland) {
          const entered = !cursorInsideIsland.current;
          cursorInsideIsland.current = true;
          if (collapseTimer.current) window.clearTimeout(collapseTimer.current);
          collapseTimer.current = null;
          if (entered || !expanded) openIsland();
          return;
        }
        if (cursorInsideIsland.current) {
          cursorInsideIsland.current = false;
          closeIslandLater();
        }
      }).catch(() => undefined);
    };
    checkCursor();
    const timer = window.setInterval(checkCursor, 80);
    return () => window.clearInterval(timer);
  }, [expanded, immersive, pinned]);
  const keepSettingsOpen = () => { if (settingsTimer.current) window.clearTimeout(settingsTimer.current); };
  const hideSettingsLater = () => { settingsTimer.current = window.setTimeout(() => { setSettingsOpen(false); settingsTimer.current = null; }, 480); };
  const changeOpacity = (value: number) => { setOpacity(value); void emit("codex-island-opacity-change", value); };
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
      <header><div className="panel-brand"><span className="brand-orbit" /><strong>Codex Island</strong><span className={`plan-label ${paidPlanClass(usage?.plan_type)}`}>{usage ? planLabel(usage.plan_type) : "—"}{usage?.plan_multiplier ? ` · ${usage.plan_multiplier}` : ""}</span></div><div className="controls">
        <button className="icon-button icon-button--external" onClick={() => openExternal("https://github.com/s840207702/codex-island")} title={copy.github} aria-label={copy.github}><Github size={16} /></button><button className="icon-button icon-button--avatar" onClick={() => openExternal("https://www.feige177.com")} title={copy.toolbox} aria-label={copy.toolbox}><img src="/feige-toolbox-avatar.png" alt="" /></button><span className="control-divider" aria-hidden="true" /><button className={`icon-button ${pinned ? "icon-button--selected" : ""}`} onClick={() => setPinned(v => { const next = !v; if (isDetailWindow) void emit("codex-island-detail-pin", next); return next; })} title={pinned ? copy.unpin : copy.pin}><Pin size={16} /></button><button className={`icon-button icon-button--opacity ${settingsOpen ? "icon-button--selected" : ""}`} onPointerEnter={keepSettingsOpen} onPointerLeave={hideSettingsLater} onClick={() => setSettingsOpen(v => !v)} title={copy.opacity} aria-label={copy.opacity}><Layers2 size={17} strokeWidth={1.8} /></button>
      </div></header>
      {settingsOpen && <section onPointerEnter={keepSettingsOpen} onPointerLeave={hideSettingsLater} className="settings-popover" aria-label={copy.opacity}><span>{opacity}%</span><input aria-label={copy.opacity} type="range" min="65" max="100" value={opacity} onChange={(event) => changeOpacity(Number(event.target.value))} /></section>}
      {/* Legacy dual-quota view, retained for a possible 5-hour quota restoration:
          <div className="overview"><Ring label={copy.fiveHours} window={usage.primary} period="short" ... /><Ring label={copy.thisWeek} window={usage.secondary} period="weekly" ... /></div> */}
      {error && !usage ? <div className="error-state"><CircleAlert size={18} /><div><b>{copy.syncUnavailable}</b><span>{error}</span></div><button onClick={() => void refresh(true)}>{copy.retry}</button></div> : usage && <div className="overview overview--weekly"><Ring label={copy.thisWeek} window={usage.weekly} period="weekly" locale={locale} copy={copy} /></div>}
      <footer className="status-rail"><span className="status-source">{loading ? <LoaderCircle className="spinning sync-spinner" size={15} /> : <i className={`live-dot ${stale ? "live-dot--error" : ""}`} />}{stale ? copy.staleRetry : !loading && copy.synced}</span>{usage?.reset_credits != null && <em className="reset-credit">{copy.resetCredits} {usage.reset_credits}{expiryText(usage.reset_credit_expires_at, locale, copy) ? ` · ${expiryText(usage.reset_credit_expires_at, locale, copy)}` : ""}</em>}<div><button className="icon-button" onClick={() => void refresh(true)} title={copy.refresh}><RefreshCw size={16} className={loading ? "spinning" : ""} /></button><button className="icon-button" onClick={() => setExitConfirmOpen(true)} title={copy.exit} aria-label={copy.exit}><X size={16} /></button></div></footer>
      {exitConfirmOpen && <section className="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="exit-dialog-title"><div className="confirm-dialog__card"><span className="confirm-dialog__eyebrow">{copy.exitConfirm}</span><strong id="exit-dialog-title">{copy.exitQuestion}</strong><p>{copy.exitBody}</p><div><button className="text-action" onClick={() => setExitConfirmOpen(false)}>{copy.cancel}</button><button className="confirm-dialog__exit" onClick={quit}>{copy.exitApp}</button></div></div></section>}
    </article>;
  if (isDetailWindow) return <main className="detail-hitbox" onPointerEnter={() => void emit("codex-island-detail-hover", true)} onPointerLeave={() => void emit("codex-island-detail-hover", false)}>{detail}</main>;
  return <main ref={islandRef} className={`island-shell ${expanded ? "island-shell--active" : ""} ${immersive ? "island-shell--immersive" : ""}`} onPointerEnter={openIsland} onPointerLeave={closeIslandLater} onMouseDownCapture={beginPotentialDrag} onMouseMoveCapture={continuePotentialDrag} onMouseUpCapture={finishPotentialDrag}>
    <div ref={barRef} className="island-bar" role="status" aria-label={immersive ? copy.immersiveQuota : copy.openQuota}>
      <span className="bar-identity"><i className={`live-dot quota-dot ${error ? "live-dot--error" : ""} ${topQuota?.temperature.critical ? "quota-dot--pulse" : ""}`} style={!error && topQuota ? { "--quota-color": topQuota.temperature.color } as React.CSSProperties : undefined} /><span className="brand-orbit" aria-hidden="true" /><b>Codex</b></span><span className="bar-summary">{topQuota ? <span className="bar-summary__value"><span className="quota-value" style={{ "--quota-color": topQuota.temperature.color } as React.CSSProperties}>{Math.round(topQuota.quota.remaining_percent)}%</span> · {compactWeeklyTime(topQuota.quota.reset_after_seconds)}</span> : "—"}</span>
    </div>
  </main>;
}
createRoot(document.getElementById("root")!).render(<App />);
