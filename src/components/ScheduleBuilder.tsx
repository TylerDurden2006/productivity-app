import type { Cadence } from "../generated/Cadence";
import type { CadenceKind } from "../generated/CadenceKind";
import type { QuotaPeriod } from "../generated/QuotaPeriod";

const weekdayOptions = [
  ["Mon", 1],
  ["Tue", 2],
  ["Wed", 4],
  ["Thu", 8],
  ["Fri", 16],
  ["Sat", 32],
  ["Sun", 64],
] as const;

const kindOptions: Array<{ value: CadenceKind; label: string }> = [
  { value: "DAILY", label: "Daily" },
  { value: "WEEKLY", label: "Specific weekdays" },
  { value: "EVERY_N_DAYS", label: "Every N days" },
  { value: "EVERY_N_WEEKS", label: "Every N weeks" },
  { value: "MONTHLY_DAY", label: "Monthly on a day" },
  { value: "MONTHLY_NTH_WEEKDAY", label: "Monthly weekday" },
  { value: "YEARLY", label: "Yearly" },
  { value: "CUSTOM_DATES", label: "Custom dates" },
  { value: "QUOTA", label: "A number of times" },
  { value: "ONE_OFF", label: "One time" },
];

export function ScheduleBuilder({
  value,
  onChange,
}: {
  value: Cadence;
  onChange: (value: Cadence) => void;
}) {
  const update = (patch: Partial<Cadence>) => onChange({ ...value, ...patch });
  const changeKind = (kind: CadenceKind) => {
    const patch: Partial<Cadence> = { kind };
    if (kind === "WEEKLY" && value.weekdays_mask === 0) {
      patch.weekdays_mask = 127;
    }
    if (kind === "MONTHLY_DAY" && value.month_day === null) {
      patch.month_day = 1;
    }
    if (kind === "MONTHLY_NTH_WEEKDAY" && value.nth_weekday === null) {
      patch.nth_weekday = 1;
      patch.weekday = 0;
    }
    if (kind === "YEARLY" && value.month === null) {
      patch.month = 1;
      patch.month_day = 1;
    }
    if (kind === "QUOTA" && value.quota_count === null) {
      patch.quota_count = 3;
      patch.quota_period = "WEEK" as QuotaPeriod;
    }
    update(patch);
  };

  const toggleDay = (bit: number) => {
    update({ weekdays_mask: value.weekdays_mask ^ bit });
  };

  return (
    <div className="schedule-builder">
      <div className="schedule-row">
        <label className="field">
          <span className="field-label">Pattern</span>
          <select
            className="input"
            value={value.kind}
            onChange={(event) => changeKind(event.target.value as CadenceKind)}
          >
            {kindOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">Starts on</span>
          <input
            className="input"
            type="date"
            value={value.anchor_on}
            onChange={(event) => update({ anchor_on: event.target.value })}
          />
        </label>
      </div>

      {value.kind === "WEEKLY" || value.kind === "EVERY_N_WEEKS" ? (
        <div className="schedule-block">
          {value.kind === "EVERY_N_WEEKS" ? (
            <label className="field compact-field">
              <span className="field-label">Every</span>
              <span className="inline-input">
                <input
                  className="input"
                  type="number"
                  min={1}
                  max={52}
                  value={value.interval_n}
                  onChange={(event) =>
                    update({ interval_n: Number(event.target.value) || 1 })
                  }
                />
                <span>weeks</span>
              </span>
            </label>
          ) : null}
          <div className="weekday-row">
            {weekdayOptions.map(([label, bit]) => (
              <button
                key={label}
                type="button"
                className="weekday-chip"
                data-active={Boolean(value.weekdays_mask & bit)}
                onClick={() => toggleDay(bit)}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
      ) : null}

      {value.kind === "EVERY_N_DAYS" ? (
        <label className="field compact-field">
          <span className="field-label">Every</span>
          <span className="inline-input">
            <input
              className="input"
              type="number"
              min={1}
              max={3650}
              value={value.interval_n}
              onChange={(event) =>
                update({ interval_n: Number(event.target.value) || 1 })
              }
            />
            <span>days</span>
          </span>
        </label>
      ) : null}

      {value.kind === "MONTHLY_DAY" ? (
        <label className="field compact-field">
          <span className="field-label">Day of month</span>
          <input
            className="input"
            type="number"
            min={1}
            max={31}
            value={value.month_day ?? 1}
            onChange={(event) =>
              update({ month_day: Number(event.target.value) || 1 })
            }
          />
        </label>
      ) : null}

      {value.kind === "MONTHLY_NTH_WEEKDAY" ? (
        <div className="schedule-row">
          <label className="field">
            <span className="field-label">Which</span>
            <select
              className="input"
              value={value.nth_weekday ?? 1}
              onChange={(event) =>
                update({ nth_weekday: Number(event.target.value) })
              }
            >
              <option value={1}>First</option>
              <option value={2}>Second</option>
              <option value={3}>Third</option>
              <option value={4}>Fourth</option>
              <option value={-1}>Last</option>
            </select>
          </label>
          <label className="field">
            <span className="field-label">Weekday</span>
            <select
              className="input"
              value={value.weekday ?? 0}
              onChange={(event) =>
                update({ weekday: Number(event.target.value) })
              }
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
        </div>
      ) : null}

      {value.kind === "YEARLY" ? (
        <div className="schedule-row">
          <label className="field">
            <span className="field-label">Month</span>
            <input
              className="input"
              type="number"
              min={1}
              max={12}
              value={value.month ?? 1}
              onChange={(event) =>
                update({ month: Number(event.target.value) || 1 })
              }
            />
          </label>
          <label className="field">
            <span className="field-label">Day</span>
            <input
              className="input"
              type="number"
              min={1}
              max={31}
              value={value.month_day ?? 1}
              onChange={(event) =>
                update({ month_day: Number(event.target.value) || 1 })
              }
            />
          </label>
        </div>
      ) : null}

      {value.kind === "CUSTOM_DATES" ? (
        <label className="field">
          <span className="field-label">
            Dates <span className="field-hint">One YYYY-MM-DD per line</span>
          </span>
          <textarea
            className="text-area"
            value={value.dates.join("\n")}
            onChange={(event) =>
              update({
                dates: event.target.value
                  .split(/\s+/)
                  .map((entry) => entry.trim())
                  .filter(Boolean),
              })
            }
            placeholder={"2026-11-03\n2026-12-17"}
          />
        </label>
      ) : null}

      {value.kind === "QUOTA" ? (
        <div className="schedule-row">
          <label className="field">
            <span className="field-label">How many times</span>
            <input
              className="input"
              type="number"
              min={1}
              max={31}
              value={value.quota_count ?? 1}
              onChange={(event) =>
                update({ quota_count: Number(event.target.value) || 1 })
              }
            />
          </label>
          <label className="field">
            <span className="field-label">Per</span>
            <select
              className="input"
              value={value.quota_period ?? "WEEK"}
              onChange={(event) =>
                update({ quota_period: event.target.value as QuotaPeriod })
              }
            >
              <option value="WEEK">Week</option>
              <option value="MONTH">Month</option>
            </select>
          </label>
        </div>
      ) : null}
    </div>
  );
}
