// The title bar's right side: runs in progress (hover to Pause all), the
// bell, and light or dark.

import { useCallback, useEffect, useRef, useState } from "react";
import {
  Bell, CircleAlert, CircleCheck, CircleHelp, LoaderCircle, MessageCircle, Moon, Pause, Play, Sun, type LucideIcon,
} from "lucide-react";
import { useApi } from "../../api/api";
import type { Notice, NoticeKind } from "../../api/types";
import { hrefFor } from "../../app/routes";
import { useSettings } from "../../settings/SettingsProvider";
import { ago } from "../runs/format";
import { useActivity } from "./activity";
import styles from "./TitleTools.module.css";

export function TitleTools() {
  return (
    <div className={styles.tools}>
      <Running />
      <NoticeBell />
      <ThemeButton />
    </div>
  );
}

/** "2 running" with a spinner; hovered, it's Pause all. With nothing running,
    just the pause icon. Paused, an amber Resume. */
function Running() {
  const { activity, pauseAll } = useActivity();
  if (activity.paused) {
    return (
      <button type="button" className={styles.paused} onClick={() => pauseAll(false)}
        aria-label="Workflows are paused. Resume all workflows" title="New files wait and schedules skip until you resume">
        <Play size={13} strokeWidth={2.2} aria-hidden /> Paused · Resume
      </button>
    );
  }
  if (!activity.running) {
    return (
      <button type="button" className={styles.icon} onClick={() => pauseAll(true)}
        aria-label="Pause all workflows" title="Pause all workflows">
        <Pause size={17} strokeWidth={1.8} aria-hidden />
      </button>
    );
  }
  return (
    <button type="button" className={styles.running} onClick={() => pauseAll(true)}
      aria-label={`${activity.running} running. Pause all workflows`} title="Pause all workflows">
      <span className={styles.spin}><LoaderCircle size={15} strokeWidth={2.2} aria-hidden /></span>
      <span className={styles.pause}><Pause size={15} strokeWidth={2} aria-hidden /></span>
      <span className={styles.count}>{activity.running} running</span>
      <span className={styles.pauseLabel}>Pause all</span>
    </button>
  );
}

const ICONS: Record<NoticeKind, { icon: LucideIcon; tone: string; word: string }> = {
  question: { icon: CircleHelp, tone: styles.danger, word: "Question" },
  failed: { icon: CircleAlert, tone: styles.danger, word: "Failed" },
  message: { icon: MessageCircle, tone: styles.accent, word: "Notify step" },
  undo: { icon: CircleCheck, tone: styles.ok, word: "Undo" },
};

function NoticeBell() {
  const api = useApi();
  const [notices, setNotices] = useState<Notice[]>([]);
  const [open, setOpen] = useState(false);
  const wrapper = useRef<HTMLDivElement>(null);
  const refresh = useCallback(async () => setNotices(await api.listNotices()), [api]);
  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => api.onNoticesChanged(() => { void refresh(); }), [api, refresh]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => { if (!wrapper.current?.contains(e.target as Node)) setOpen(false); };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    const onNavigate = () => setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("hashchange", onNavigate);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("hashchange", onNavigate);
    };
  }, [open]);

  const unread = notices.filter((n) => !n.read).length;
  const label = unread ? `Notifications, ${unread} unread` : "Notifications";
  return (
    <div className={styles.bellWrap} ref={wrapper}>
      <button type="button" className={open ? `${styles.icon} ${styles.active}` : styles.icon} aria-label={label} title={label}
        aria-expanded={open} aria-haspopup="dialog" onClick={() => setOpen(!open)}>
        <Bell size={17} strokeWidth={1.8} aria-hidden />
        {unread > 0 && <span className={styles.badge} aria-hidden>{unread > 99 ? "99+" : unread}</span>}
      </button>
      {open && (
        <div className={styles.panel} role="dialog" aria-label="Notifications">
          <div className={styles.panelHead}>
            <h2>Notifications</h2>
            {unread > 0 && (
              <button type="button" className={styles.textButton} onClick={() => api.markNoticesRead()}>Mark all as read</button>
            )}
          </div>
          {notices.length ? (
            <ul className={styles.notices}>
              {notices.slice(0, 50).map((n) => {
                const { icon: Icon, tone, word } = ICONS[n.kind];
                return (
                  <li key={n.id}>
                    <a className={n.read ? styles.notice : `${styles.notice} ${styles.unread}`}
                      href={n.runId ? hrefFor({ page: "run", id: n.runId }) : hrefFor({ page: "history" })}
                      onClick={() => { if (!n.read) void api.markNoticesRead([n.id]); }}>
                      <span className={styles.dot} aria-label={n.read ? undefined : "Unread"} />
                      <Icon size={17} strokeWidth={1.8} className={tone} aria-hidden />
                      <span className={styles.noticeText}>
                        <span className={styles.noticeWorkflow}>{n.workflowName}</span>
                        <span>{n.message}</span>
                        <span className={styles.when}>{word} · {ago(n.at)}</span>
                      </span>
                    </a>
                  </li>
                );
              })}
            </ul>
          ) : <p className={styles.empty}>Nothing yet. Questions, failed runs and Notify steps show up here.</p>}
          <a className={styles.seeAll} href={hrefFor({ page: "history" })}>See all in History</a>
        </div>
      )}
    </div>
  );
}

/** Sun in light, moon in dark; switches to the other, as Settings › Appearance does. */
function ThemeButton() {
  const { settings, update } = useSettings();
  const macDark = useMacDark();
  const dark = settings.appearance === "dark" || (settings.appearance === "system" && macDark);
  const label = dark ? "Switch to light mode" : "Switch to dark mode";
  return (
    <button type="button" className={styles.icon} aria-label={label} title={label}
      onClick={() => update({ appearance: dark ? "light" : "dark" })}>
      {dark ? <Moon size={17} strokeWidth={1.8} aria-hidden /> : <Sun size={17} strokeWidth={1.8} aria-hidden />}
    </button>
  );
}

/** Whether the Mac is in dark mode, kept up to date. */
function useMacDark() {
  const query = typeof window !== "undefined" && window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;
  const [dark, setDark] = useState(query?.matches ?? false);
  useEffect(() => {
    if (!query) return;
    const onChange = () => setDark(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [query]);
  return dark;
}
