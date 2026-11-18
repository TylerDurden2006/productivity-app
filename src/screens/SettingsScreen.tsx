import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ArrowCounterClockwise,
  ArrowRight,
  Database,
  DownloadSimple,
  HardDrives,
  Info,
  SlidersHorizontal,
} from "@phosphor-icons/react";
import type { Settings } from "../generated/Settings";
import { cancelDataMove, openDataFolder, prepareDataMove, restartApp } from "../api";
import { useFlow } from "../store";

type FormState = {
  rollover: string;
  budget: string;
  cap: string;
  focusBlock: string;
  multiplier: string;
  weekStart: string;
  autoCarry: boolean;
  dashboardRange: string;
  notifyBlockEnd: boolean;
  planningReminderEnabled: boolean;
  planningReminderTime: string;
  reducedMotion: boolean;
};

type NumericField = "rollover" | "budget" | "cap" | "focusBlock" | "multiplier";

const labels: Array<{
  key: NumericField;
  label: string;
  hint: string;
  min: number;
  step: number;
}> = [
  {
    key: "rollover",
    label: "Day rollover hour",
    hint: "The logical day changes at this local hour.",
    min: 0,
    step: 1,
  },
  {
    key: "budget",
    label: "Daily focus budget",
    hint: "Minutes of deep work the plan aims to fit.",
    min: 1,
    step: 15,
  },
  {
    key: "focusBlock",
    label: "Focus block",
    hint: "The default length of one focus session.",
    min: 1,
    step: 5,
  },
];

const isLinux =
  typeof navigator !== "undefined" && /Linux/i.test(navigator.userAgent);

function toForm(settings: Settings): FormState {
  return {
    rollover: String(settings.day_rollover_hour),
    budget: String(settings.daily_focus_budget_mins),
    cap: String(settings.ready_cap),
    focusBlock: String(settings.focus_block_mins),
    multiplier: String(settings.default_multiplier),
    weekStart: String(settings.week_starts_on),
    autoCarry: settings.auto_carry_enabled,
    dashboardRange: settings.dashboard_default_range,
    notifyBlockEnd: settings.notify_block_end,
    planningReminderEnabled: settings.planning_reminder_enabled,
    planningReminderTime: settings.planning_reminder_time ?? "20:00",
    reducedMotion: settings.reduced_motion,
  };
}

export function SettingsScreen() {
  const settings = useFlow((state) => state.settings);
  const startup = useFlow((state) => state.startup);
  const refreshSettings = useFlow((state) => state.refreshSettings);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [form, setForm] = useState<FormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [lastTransfer, setLastTransfer] = useState<{
    kind: "export" | "backup";
    path: string;
  } | null>(null);
  const [pendingFolder, setPendingFolder] = useState<string | null>(null);
  const [version, setVersion] = useState("1.0.0");

  useEffect(() => {
    void getVersion().then(setVersion).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!settings) {
      void refreshSettings();
    }
  }, [refreshSettings, settings]);

  useEffect(() => {
    if (settings && !form) {
      setForm(toForm(settings));
    }
  }, [form, settings]);

  if (!settings || !form) {
    return (
      <>
        <div className="page-head">
          <div>
            <div className="kicker">Settings</div>
            <h1 className="page-title">
              Make Flow <em>fit.</em>
            </h1>
          </div>
        </div>
        <div className="surface" style={{ padding: 22 }}>
          <div className="skeleton skeleton-title" />
          <div className="skeleton skeleton-line" />
          <div className="skeleton skeleton-line" style={{ width: "58%" }} />
        </div>
      </>
    );
  }

  const set = (key: keyof FormState, value: string) => {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  };

  const submit = async () => {
    const numbers = {
      rollover: Number(form.rollover),
      budget: Number(form.budget),
      cap: Number(form.cap),
      focusBlock: Number(form.focusBlock),
      multiplier: Number(form.multiplier),
      weekStart: Number(form.weekStart),
    };
    if (Object.values(numbers).some((value) => !Number.isFinite(value))) {
      notify("Some values are not numbers.", "warning");
      return;
    }
    setSaving(true);
    const result = await act<Settings>("update_settings", {
      patch: {
        day_rollover_hour: numbers.rollover,
        daily_focus_budget_mins: numbers.budget,
        ready_cap: numbers.cap,
        focus_block_mins: numbers.focusBlock,
        default_multiplier: numbers.multiplier,
        week_starts_on: numbers.weekStart,
        auto_carry_enabled: form.autoCarry,
        dashboard_default_range: form.dashboardRange,
        notify_block_end: form.notifyBlockEnd,
        planning_reminder_enabled: form.planningReminderEnabled,
        planning_reminder_time: form.planningReminderTime || null,
        reduced_motion: form.reducedMotion,
      },
    });
    setSaving(false);
    if (result) {
      setForm(toForm(result));
      notify("Settings saved.", "success");
    }
  };

  const exportJson = async () => {
    const path = await save({
      title: "Export Flow data",
      defaultPath: `flow-export-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      return;
    }
    const result = await act<{ path: string }>("export_json", { path });
    if (result) {
      setLastTransfer({ kind: "export", path: result.path });
      notify("Export written.", "success", result.path);
    }
  };

  const backup = async () => {
    const path = await save({
      title: "Save a database backup",
      defaultPath: `flow-backup-${new Date().toISOString().slice(0, 10)}.db`,
      filters: [{ name: "SQLite database", extensions: ["db"] }],
    });
    if (!path) {
      return;
    }
    const result = await act<{ path: string }>("backup_db", { path });
    if (result) {
      setLastTransfer({ kind: "backup", path: result.path });
      notify("Backup written.", "success", result.path);
    }
  };

  const chooseFolder = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose Flow data folder",
    });
    if (typeof selected !== "string") {
      return;
    }
    try {
      const result = await prepareDataMove(selected);
      if (result?.ok) {
        setPendingFolder(result.target);
      }
    } catch (error) {
      notify("Could not use that folder.", "error", String(error));
    }
  };

  const openFolder = async () => {
    try {
      await openDataFolder();
    } catch (error) {
      notify("Could not open the folder.", "error", String(error));
    }
  };

  const restart = async () => {
    await restartApp();
  };

  const cancelMove = async () => {
    await cancelDataMove();
    setPendingFolder(null);
  };

  return (
    <>
      <div className="page-head">
        <div>
          <div className="kicker">Settings</div>
          <h1 className="page-title">
            Make Flow <em>fit.</em>
          </h1>
          <p className="page-sub">
            Every number lives on this machine. Change one and the next plan uses
            it immediately.
          </p>
        </div>
      </div>

      <div className="settings-layout">
        <section className="surface settings-card">
          <div className="settings-head">
            <div>
              <div className="kicker">Planning</div>
              <h2 className="settings-title">The day's dimensions</h2>
            </div>
            <SlidersHorizontal size={17} className="stats-icon" />
          </div>
          <div className="settings-grid">
            {labels.map((field) => (
              <label key={field.key} className="setting-field">
                <span className="setting-label">{field.label}</span>
                <span className="setting-hint">{field.hint}</span>
                <input
                  className="input setting-input"
                  data-testid={`setting-${field.key}`}
                  type="number"
                  min={field.min}
                  step={field.step}
                  value={form[field.key]}
                  onChange={(event) => set(field.key, event.target.value)}
                />
              </label>
            ))}
            <label className="setting-field">
              <span className="setting-label">Week starts on</span>
              <span className="setting-hint">Used by weekly plans.</span>
              <select
                className="input setting-input"
                value={form.weekStart}
                onChange={(event) => set("weekStart", event.target.value)}
              >
                {["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"].map(
                  (label, index) => (
                    <option key={label} value={index}>
                      {label}
                    </option>
                  ),
                )}
              </select>
            </label>
            <label className="setting-field setting-toggle">
              <span className="setting-label">Automatic carry</span>
              <span className="setting-hint">Unfinished work moves forward and keeps its section.</span>
              <input
                type="checkbox"
                checked={form.autoCarry}
                onChange={(event) => setForm({ ...form, autoCarry: event.target.checked })}
              />
            </label>
            <label className="setting-field">
              <span className="setting-label">Dashboard opens on</span>
              <span className="setting-hint">The default range for the first view.</span>
              <select
                className="input setting-input"
                value={form.dashboardRange}
                onChange={(event) => setForm({ ...form, dashboardRange: event.target.value })}
              >
                {["DAY", "WEEK", "MONTH", "QUARTER", "YEAR", "ALL"].map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            </label>
            <label className="setting-field setting-toggle">
              <span className="setting-label">Block-end notification</span>
              <span className="setting-hint">Show a native notice when a focus block ends.</span>
              <input
                type="checkbox"
                checked={form.notifyBlockEnd}
                onChange={(event) => setForm({ ...form, notifyBlockEnd: event.target.checked })}
              />
            </label>
            <label className="setting-field setting-toggle">
              <span className="setting-label">Nightly planning reminder</span>
              <span className="setting-hint">A quiet reminder to plan tomorrow before bed.</span>
              <input
                type="checkbox"
                checked={form.planningReminderEnabled}
                onChange={(event) =>
                  setForm({ ...form, planningReminderEnabled: event.target.checked })
                }
              />
            </label>
            <label className="setting-field">
              <span className="setting-label">Reminder time</span>
              <span className="setting-hint">Local time for the planning reminder.</span>
              <input
                className="input setting-input"
                type="time"
                value={form.planningReminderTime}
                onChange={(event) =>
                  setForm({ ...form, planningReminderTime: event.target.value })
                }
              />
            </label>
            <label className="setting-field setting-toggle">
              <span className="setting-label">Reduce motion</span>
              <span className="setting-hint">Use less movement in transitions and charts.</span>
              <input
                type="checkbox"
                checked={form.reducedMotion}
                onChange={(event) => setForm({ ...form, reducedMotion: event.target.checked })}
              />
            </label>
          </div>
          <details className="settings-advanced">
            <summary>Advanced</summary>
            <div className="settings-grid">
              <label className="setting-field">
                <span className="setting-label">Ready cap</span>
                <span className="setting-hint">Open tasks allowed before the soft gate.</span>
                <input
                  className="input setting-input"
                  data-testid="setting-cap"
                  type="number"
                  min={0}
                  value={form.cap}
                  onChange={(event) => set("cap", event.target.value)}
                />
              </label>
              <label className="setting-field">
                <span className="setting-label">Default multiplier</span>
                <span className="setting-hint">Used until calibration has enough samples.</span>
                <input
                  className="input setting-input"
                  data-testid="setting-multiplier"
                  type="number"
                  min={0.1}
                  step={0.1}
                  value={form.multiplier}
                  onChange={(event) => set("multiplier", event.target.value)}
                />
              </label>
            </div>
          </details>
          <div className="settings-actions">
            <button
              type="button"
              className="btn btn-ghost"
              data-testid="settings-reset"
              onClick={() => setForm(toForm(settings))}
            >
              <ArrowCounterClockwise size={14} /> Reset
            </button>
            <button
              type="button"
              className="btn btn-primary"
              data-testid="settings-save"
              disabled={saving}
              onClick={() => void submit()}
            >
              {saving ? "Saving…" : "Save settings"} <ArrowRight size={14} />
            </button>
          </div>
        </section>

        <section className="surface settings-card data-home-card">
          <div className="settings-head">
            <div>
              <div className="kicker">Flow Home</div>
              <h2 className="settings-title">Where the data lives</h2>
            </div>
            <HardDrives size={17} className="stats-icon" />
          </div>
          <p className="settings-prose">
            The database and its daily backups live in one folder. You can move
            the folder at any time. Flow copies the data, verifies it, then asks
            you to restart.
          </p>
          <div className="data-path">
            <span>Current folder</span>
            <code data-testid="settings-data-dir">{startup?.dataDir ?? "Unknown"}</code>
          </div>
          {startup?.dataWarning ? (
            <div className="form-error">{startup.dataWarning}</div>
          ) : null}
          {startup?.oldDataDir ? (
            <p className="inline-note">
              Previous copy kept at <code>{startup.oldDataDir}</code>
            </p>
          ) : null}
          <div className="settings-actions">
            <button type="button" className="btn btn-secondary" onClick={() => void openFolder()}>
              Open folder
            </button>
            <button type="button" className="btn btn-secondary" onClick={() => void chooseFolder()}>
              Change folder
            </button>
          </div>
          {pendingFolder ? (
            <div className="data-move-confirm">
              <strong>Move Flow to this folder?</strong>
              <code>{pendingFolder}</code>
              <p>Flow will copy the database and backups after a restart.</p>
              <div className="settings-actions">
                <button type="button" className="btn btn-ghost" onClick={() => void cancelMove()}>
                  Not now
                </button>
                <button type="button" className="btn btn-primary" onClick={() => void restart()}>
                  Restart and move
                </button>
              </div>
            </div>
          ) : null}
        </section>

        <section className="surface settings-card">
          <div className="settings-head">
            <div>
              <div className="kicker">Your data</div>
              <h2 className="settings-title">Export and backup</h2>
            </div>
            <Database size={17} className="stats-icon" />
          </div>
          <p className="settings-prose">
            Export writes readable JSON. Backup writes a complete SQLite copy.
            Flow already keeps the fourteen newest daily backups in its app data
            folder.
          </p>
          <div className="settings-actions">
            <button
              type="button"
              className="btn btn-secondary"
              data-testid="settings-export"
              onClick={() => void exportJson()}
            >
              <DownloadSimple size={14} /> Export JSON
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              data-testid="settings-backup"
              onClick={() => void backup()}
            >
              <HardDrives size={14} /> Backup database
            </button>
          </div>
          {lastTransfer ? (
            <div className="path-note">
              <span>{lastTransfer.kind === "export" ? "Exported" : "Backed up"}</span>
              <code>{lastTransfer.path}</code>
            </div>
          ) : null}
          <div className="path-block">
            <div>
              <span>Database</span>
              <code data-testid="settings-db-path">
                {startup?.dbPath ?? "Unknown"}
              </code>
            </div>
            <div>
              <span>Backups</span>
              <code data-testid="settings-backups-dir">
                {startup?.backupsDir ?? "Unknown"}
              </code>
            </div>
            <div>
              <span>Newest backup</span>
              <code data-testid="settings-newest-backup">
                {startup?.newestBackup ?? "None yet"}
              </code>
            </div>
          </div>
          <p className="inline-note">
            To restore, quit Flow and replace <code>flow.db</code> with a backup.
            Then reopen the app. There is no in-app restore, by design.
          </p>
        </section>

        <section className="surface settings-card">
          <div className="settings-head">
            <div>
              <div className="kicker">About</div>
              <h2 className="settings-title">Quiet by default</h2>
            </div>
            <Info size={17} className="stats-icon" />
          </div>
          <div className="settings-fact">
            <span>Version</span>
            <strong>{version}</strong>
          </div>
          <div className="settings-fact">
            <span>Storage</span>
            <strong>Local SQLite</strong>
          </div>
          <div className="settings-fact">
            <span>Network</span>
            <strong>None</strong>
          </div>
          <div className="settings-fact">
            <span>Tray or global shortcut</span>
            <strong>Not installed</strong>
          </div>
          <p className="inline-note">
            Flow runs while this window is open. Closing it quits; minimizing
            keeps it running.
          </p>
          {isLinux ? (
            <p className="inline-note" data-testid="settings-wayland-note">
              On Wayland, compositor-level shortcuts are unavailable. Use
              Ctrl+K inside Flow; the app never installs a global shortcut.
            </p>
          ) : null}
        </section>
      </div>
    </>
  );
}
