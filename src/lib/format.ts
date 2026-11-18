import type { Slot } from "../generated/Slot";

const dayFormatter = new Intl.DateTimeFormat("en-GB", {
  weekday: "long",
  day: "numeric",
  month: "long",
});

const shortDayFormatter = new Intl.DateTimeFormat("en-GB", {
  weekday: "short",
  day: "numeric",
});

const monthFormatter = new Intl.DateTimeFormat("en-GB", {
  month: "long",
  year: "numeric",
});

const clockFormatter = new Intl.DateTimeFormat("en-GB", {
  hour: "2-digit",
  minute: "2-digit",
});

export function parseDay(value: string): Date {
  return new Date(`${value}T12:00:00`);
}

export function logicalTodayISO(rolloverHour = 0, now = new Date()): string {
  const date = new Date(now);
  if (date.getHours() < rolloverHour) {
    date.setDate(date.getDate() - 1);
  }
  return isoDate(date);
}

function isoDate(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function addDays(value: string, days: number): string {
  const date = parseDay(value);
  date.setDate(date.getDate() + days);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function formatLongDay(value: string): string {
  return dayFormatter.format(parseDay(value));
}

export function formatShortDay(value: string): string {
  return shortDayFormatter.format(parseDay(value));
}

export function formatMonthYear(value: Date = new Date()): string {
  return monthFormatter.format(value);
}

export function formatDayMonth(value: string): string {
  const date = parseDay(value);
  return `${date.getDate()} ${new Intl.DateTimeFormat("en-GB", { month: "short" }).format(date)}`;
}

export function formatClock(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return clockFormatter.format(date);
}

export function relativeDay(value: string, today: string): string {
  if (value === today) {
    return "Today";
  }
  if (value === addDays(today, 1)) {
    return "Tomorrow";
  }
  return formatShortDay(value);
}

export function formatMins(mins: number | null | undefined): string {
  if (mins === null || mins === undefined) {
    return "—";
  }
  if (mins < 60) {
    return `${mins}m`;
  }
  const hours = Math.floor(mins / 60);
  const rest = mins % 60;
  return rest === 0 ? `${hours}h` : `${hours}h ${rest}m`;
}

export function formatTimer(totalSeconds: number): string {
  const safe = Math.max(0, Math.floor(totalSeconds));
  const hours = Math.floor(safe / 3600);
  const minutes = Math.floor((safe % 3600) / 60);
  const seconds = safe % 60;
  const mm = String(minutes).padStart(2, "0");
  const ss = String(seconds).padStart(2, "0");
  return hours > 0 ? `${hours}:${mm}:${ss}` : `${mm}:${ss}`;
}

export function slotLabel(slot: Slot): string {
  switch (slot) {
    case "ONE":
      return "Main";
    case "THREE":
      return "Next";
    case "BATCH":
      return "Small";
    case "EXTRA":
      return "More";
  }
}

export function slotShort(slot: Slot): string {
  switch (slot) {
    case "ONE":
      return "1";
    case "THREE":
      return "3";
    case "BATCH":
      return "5";
    case "EXTRA":
      return "MORE";
  }
}

export function isDeepEstimate(estimate: number | null, doneWhen?: string | null): boolean {
  if (estimate !== null) {
    return estimate >= 30;
  }
  return Boolean(doneWhen);
}

export function plural(count: number, singular: string, pluralForm?: string): string {
  return count === 1 ? singular : (pluralForm ?? `${singular}s`);
}

export function truncate(value: string, length: number): string {
  if (value.length <= length) {
    return value;
  }
  return `${value.slice(0, length - 1).trimEnd()}…`;
}
