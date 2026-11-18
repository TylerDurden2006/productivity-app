import { useEffect, useRef, useState } from "react";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import {
  ChartLineUp,
  Leaf,
  ListChecks,
  Plus,
  SlidersHorizontal,
  Sparkle,
  SquaresFour,
  Timer,
} from "@phosphor-icons/react";
import { runAction, type StartupView } from "./api";
import { AddTaskDialog } from "./components/AddTaskDialog";
import { OverrideDialog } from "./components/OverrideDialog";
import { ProgressThread, Toasts } from "./components/ui";
import { DailyScreen } from "./screens/DailyScreen";
import { DashboardScreen } from "./screens/DashboardScreen";
import { DoScreen } from "./screens/DoScreen";
import { HorizonsScreen } from "./screens/HorizonsScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { TryScreen } from "./screens/TryScreen";
import { useFlow, type Nav } from "./store";
import { formatLongDay, formatMonthYear, logicalTodayISO, parseDay } from "./lib/format";

const navItems: Array<{
  id: Nav;
  label: string;
  icon: typeof SquaresFour;
}> = [
  { id: "daily", label: "Daily", icon: ListChecks },
  { id: "plan", label: "Plan", icon: SquaresFour },
  { id: "do", label: "Do", icon: Timer },
  { id: "try", label: "Try", icon: Sparkle },
  { id: "dashboard", label: "Dashboard", icon: ChartLineUp },
  { id: "settings", label: "Settings", icon: SlidersHorizontal },
];

export default function App() {
  const boot = useFlow((state) => state.boot);
  const booted = useFlow((state) => state.booted);
  const bootError = useFlow((state) => state.bootError);
  const startup = useFlow((state) => state.startup);
  const settings = useFlow((state) => state.settings);
  const nav = useFlow((state) => state.nav);
  const mode = useFlow((state) => state.mode);
  const plan = useFlow((state) => state.plan);
  const doView = useFlow((state) => state.doView);
  const navigate = useFlow((state) => state.navigate);
  const setCaptureOpen = useFlow((state) => state.setCaptureOpen);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const refreshAfter = useFlow((state) => state.refreshAfter);
  const reconcileBusy = useRef(false);
  const [currentDate, setCurrentDate] = useState(() =>
    logicalTodayISO(settings?.day_rollover_hour ?? 0),
  );
  const currentDateRef = useRef(currentDate);

  useEffect(() => {
    currentDateRef.current = currentDate;
  }, [currentDate]);

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    if (!startup?.ok) {
      return;
    }
    const rolloverHour = settings?.day_rollover_hour ?? 0;
    let cancelled = false;
    const syncDate = async () => {
      const next = logicalTodayISO(rolloverHour);
      if (next === currentDateRef.current) {
        return;
      }
      currentDateRef.current = next;
      setCurrentDate(next);
      try {
        await runAction("reconcile");
      } catch {
        // The regular reconcile tick retries.
      }
      await refreshAfter("date-tick");
    };
    let boundaryTimer: number | undefined;
    const scheduleBoundary = () => {
      if (cancelled) {
        return;
      }
      const now = new Date();
      const boundary = new Date(now);
      boundary.setHours(rolloverHour, 0, 0, 0);
      if (boundary.getTime() <= now.getTime()) {
        boundary.setDate(boundary.getDate() + 1);
      }
      boundaryTimer = window.setTimeout(
        () => {
          void syncDate().finally(() => {
            if (!cancelled) {
              scheduleBoundary();
            }
          });
        },
        boundary.getTime() - now.getTime() + 250,
      );
    };
    void syncDate();
    scheduleBoundary();
    const timer = window.setInterval(() => void syncDate(), 30_000);
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        void syncDate();
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      if (boundaryTimer !== undefined) {
        window.clearTimeout(boundaryTimer);
      }
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [refreshAfter, settings?.day_rollover_hour, startup?.ok]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCaptureOpen(true);
      }
      if (
        (event.ctrlKey || event.metaKey) &&
        event.key >= "1" &&
        event.key <= "6"
      ) {
        const item = navItems[Number(event.key) - 1];
        if (item) {
          event.preventDefault();
          navigate(item.id);
        }
      }
      if (
        (event.ctrlKey || event.metaKey) &&
        event.key.toLowerCase() === "z" &&
        !target?.closest("input, textarea, select, [contenteditable='true']") &&
        !document.querySelector('[role="dialog"]')
      ) {
        event.preventDefault();
        void act<unknown>("undo_last").then((result) => {
          if (result) {
            notify("Undone.", "success");
          }
        });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [act, navigate, notify, setCaptureOpen]);

  useEffect(() => {
    document.documentElement.dataset.reducedMotion = settings?.reduced_motion
      ? "true"
      : "false";
  }, [settings?.reduced_motion]);

  useEffect(() => {
    if (
      !startup?.ok ||
      !settings?.planning_reminder_enabled ||
      !settings.planning_reminder_time
    ) {
      return;
    }
    const check = async () => {
      const now = new Date();
      const time = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
      if (time !== settings.planning_reminder_time) {
        return;
      }
      const today = now.toLocaleDateString("en-CA");
      const key = "flow-planning-reminder";
      if (window.localStorage.getItem(key) === today) {
        return;
      }
      let granted = await isPermissionGranted();
      if (!granted) {
        granted = (await requestPermission()) === "granted";
      }
      if (!granted) {
        return;
      }
      sendNotification({
        title: "Plan tomorrow",
        body: "A few minutes now makes the morning quiet.",
      });
      window.localStorage.setItem(key, today);
    };
    void check();
    const timer = window.setInterval(() => void check(), 60_000);
    return () => window.clearInterval(timer);
  }, [
    settings?.planning_reminder_enabled,
    settings?.planning_reminder_time,
    startup?.ok,
  ]);

  useEffect(() => {
    if (!startup?.ok || !doView?.session?.running) {
      return;
    }
    const beat = window.setInterval(() => {
      void runAction("heartbeat").catch(() => undefined);
    }, 60_000);
    return () => window.clearInterval(beat);
  }, [startup?.ok, doView?.session?.running]);

  useEffect(() => {
    if (!startup?.ok) {
      return;
    }
    const reconcile = async () => {
      if (reconcileBusy.current) {
        return;
      }
      reconcileBusy.current = true;
      try {
        const report = await runAction<{
          carried: string[];
          frozen: string[];
          stale_session: unknown;
        }>("reconcile");
        if (
          report.carried.length > 0 ||
          report.frozen.length > 0 ||
          report.stale_session
        ) {
          await refreshAfter("reconcile");
          if (report.carried.length > 0) {
            notify(
              "The day moved on.",
              "info",
              `${report.carried.length} unfinished task${
                report.carried.length === 1 ? " was" : "s were"
              } carried forward.`,
            );
          }
          if (report.stale_session) {
            notify(
              "A timer was recovered.",
              "warning",
              "Flow froze the abandoned session and kept its elapsed time.",
            );
          }
        }
      } catch {
        // Reconciliation is retried on the next tick.
      } finally {
        reconcileBusy.current = false;
      }
    };
    const tick = window.setInterval(() => void reconcile(), 5 * 60_000);
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        void reconcile();
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.clearInterval(tick);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [notify, refreshAfter, startup?.ok]);

  if (!booted) {
    return (
      <div className="fatal">
        <div className="fatal-card">
          <div className="skeleton skeleton-title" />
          <div className="skeleton skeleton-line" style={{ width: "62%" }} />
          <div className="skeleton skeleton-line" style={{ width: "44%" }} />
          <p className="inline-note" style={{ marginTop: 22 }}>
            Opening the local database…
          </p>
        </div>
      </div>
    );
  }

  if (bootError || (startup && !startup.ok)) {
    return <StartupFailure message={bootError} startup={startup} />;
  }

  if (nav === "do") {
    return (
      <>
        <DoScreen />
        <AddTaskDialogHost />
        <OverrideDialog />
        <Toasts />
        <ProgressThread />
      </>
    );
  }

  const inboxCount = plan?.inbox.length ?? 0;
  const doCount = doView?.remaining_count ?? 0;
  const today = currentDate;

  return (
    <div className="app-shell" data-testid="app-shell">
      <aside className="rail">
        <div className="brand">
          <div className="brand-mark">
            <Leaf size={17} weight="fill" />
          </div>
          <div className="brand-copy">
            <div className="brand-name">Flow</div>
            <div className="brand-sub">A quieter workday</div>
          </div>
        </div>

        <div className="rail-date">
          <div className="rail-date-day">{formatLongDay(today)}</div>
          <div className="rail-date-year">{formatMonthYear(parseDay(today))}</div>
        </div>

        <nav className="nav" aria-label="Main">
          {navItems.map((item) => {
            const Icon = item.icon;
            const count =
              item.id === "plan"
                ? inboxCount
                : item.id === "do"
                  ? doCount
                  : 0;
            return (
              <button
                key={item.id}
                type="button"
                className="nav-item"
                data-testid={`nav-${item.id}`}
                data-active={nav === item.id}
                onClick={() => navigate(item.id)}
              >
                <Icon size={17} />
                <span className="nav-label">{item.label}</span>
                {count ? <span className="nav-count">{count}</span> : null}
              </button>
            );
          })}
        </nav>

        <div className="rail-foot">
          <div className="mode-chip" data-testid="mode-chip">
            <span className="mode-dot" data-mode={mode} />
            {mode === "DO" ? "Doing the plan" : "Planning"}
          </div>
          <div className="rail-note">
            Local to this machine.
            <br />
            Saved on every action.
          </div>
        </div>
      </aside>

      <main className="main">
        <div className="scene">
          <header className="topbar">
            <div className="topbar-note">
              {mode === "DO" ? "Plan locked · one thing at a time" : "Make the day small"}
            </div>
            <div className="topbar-spacer" />
            <div className="topbar-actions">
              <span className="inline-note nowrap">
                <span className="kbd">Ctrl</span> <span className="kbd">K</span>
              </span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                data-testid="topbar-capture"
                onClick={() => setCaptureOpen(true)}
              >
                <Plus size={13} /> Add task
              </button>
            </div>
          </header>

          {startup?.dataWarning ? (
            <div className="banner" data-tone="rust" style={{ marginTop: 18 }}>
              <div>
                <div className="banner-title">Data folder warning</div>
                <div className="banner-body">{startup.dataWarning}</div>
              </div>
            </div>
          ) : null}

          <div key={nav} className="reveal">
            {nav === "daily" ? <DailyScreen /> : null}
            {nav === "plan" ? <HorizonsScreen /> : null}
            {nav === "try" ? <TryScreen /> : null}
            {nav === "dashboard" ? <DashboardScreen /> : null}
            {nav === "settings" ? <SettingsScreen /> : null}
          </div>
        </div>
      </main>

      <AddTaskDialogHost />
      <OverrideDialog />
      <Toasts />
      <ProgressThread />
    </div>
  );
}

function AddTaskDialogHost() {
  const open = useFlow((state) => state.captureOpen);
  const setOpen = useFlow((state) => state.setCaptureOpen);
  return open ? <AddTaskDialog onClose={() => setOpen(false)} /> : null;
}

function StartupFailure({
  message,
  startup,
}: {
  message: string | null;
  startup: StartupView | null;
}) {
  const detail = message ?? startup?.error ?? "The database could not be opened.";
  return (
    <div className="fatal">
      <div className="fatal-card">
        <div className="kicker" style={{ color: "var(--rust)" }}>
          Database unavailable
        </div>
        <h1 className="fatal-title">Flow could not open your day.</h1>
        <p className="fatal-text">{detail}</p>
        <p className="fatal-text">
          Your tasks are still in the database on disk. Close Flow, then replace
          the database file with the newest backup below. Restart after the copy
          finishes.
        </p>
        {startup ? (
          <>
            <div className="path-block">
              <div className="path-label">Database</div>
              <div className="path-value">{startup.dbPath}</div>
            </div>
            <div className="path-block">
              <div className="path-label">Backups</div>
              <div className="path-value">{startup.backupsDir}</div>
            </div>
            <div className="path-block">
              <div className="path-label">Newest backup</div>
              <div className="path-value">
                {startup.newestBackup ?? "No backup was found."}
              </div>
            </div>
          </>
        ) : null}
        <div className="data-actions">
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => window.location.reload()}
          >
            Try again
          </button>
        </div>
      </div>
    </div>
  );
}

