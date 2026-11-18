import { useEffect, useMemo, useState } from "react";
import {
  ArrowRight,
  CalendarBlank,
  Check,
  Flask,
  Plus,
} from "@phosphor-icons/react";
import type { EnergyLevel } from "../generated/EnergyLevel";
import type { SocialMode } from "../generated/SocialMode";
import type { TimeOfDay } from "../generated/TimeOfDay";
import type { TryCost } from "../generated/TryCost";
import type { TryEffort } from "../generated/TryEffort";
import type { TryItem } from "../generated/TryItem";
import type { TryItemInput } from "../generated/TryItemInput";
import { formatDayMonth } from "../lib/format";
import { useFlow } from "../store";
import { Field, Modal } from "../components/ui";

const statusOrder: Record<string, number> = {
  SCHEDULED: 0,
  IDEA: 1,
  TRIED: 2,
  SKIPPED: 3,
};

export function TryScreen() {
  const items = useFlow((state) => state.tryItems);
  const refreshTry = useFlow((state) => state.refreshTry);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [addOpen, setAddOpen] = useState(false);
  const [scheduling, setScheduling] = useState<TryItem | null>(null);
  const [trying, setTrying] = useState<TryItem | null>(null);
  const [filter, setFilter] = useState<"ALL" | "IDEA" | "SCHEDULED" | "TRIED" | "SKIPPED">("ALL");

  useEffect(() => {
    void refreshTry();
  }, [refreshTry]);

  const visible = useMemo(
    () =>
      items
        .filter((item) => filter === "ALL" || item.status === filter)
        .sort((a, b) => {
          const order = (statusOrder[a.status] ?? 9) - (statusOrder[b.status] ?? 9);
          return order || b.created_at.localeCompare(a.created_at);
        }),
    [filter, items],
  );

  const unschedule = async (item: TryItem) => {
    const result = await act<TryItem>("unschedule_try_item", { id: item.id });
    if (result) {
      notify("Back on the shelf.", "info", item.title);
    }
  };

  const skip = async (item: TryItem) => {
    const result = await act<TryItem>("skip_try_item", { id: item.id });
    if (result) {
      notify("Skipped for now.", "info", item.title);
    }
  };

  const markTried = async (
    item: TryItem,
    rating: number | null,
    reflection: string | null,
  ) => {
    const result = await act<TryItem>("mark_try_item_tried", {
      id: item.id,
      rating,
      reflection,
    });
    if (result) {
      notify("A new thing tried.", "success", item.title);
      setTrying(null);
    }
  };

  return (
    <div className="page try-page" data-testid="try-page">
      <div className="page-head">
        <div>
          <div className="kicker">New things</div>
          <h1 className="page-title">
            Keep a wider <em>life.</em>
          </h1>
          <p className="page-sub">
            A shelf for things worth trying. No deadline until you give it a day.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => setAddOpen(true)}
        >
          <Plus size={15} /> Add an idea
        </button>
      </div>

      <div className="segmented try-filter">
        {(["ALL", "IDEA", "SCHEDULED", "TRIED", "SKIPPED"] as const).map((value) => (
          <button
            key={value}
            type="button"
            data-active={filter === value}
            onClick={() => setFilter(value)}
          >
            {value === "ALL" ? "All" : value.charAt(0) + value.slice(1).toLowerCase()}
          </button>
        ))}
      </div>

      {visible.length === 0 ? (
        <div className="empty-shelf">
          <Flask size={24} />
          <h2>The shelf is quiet.</h2>
          <p>Add something you have been meaning to try.</p>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => setAddOpen(true)}
          >
            Add the first one
          </button>
        </div>
      ) : (
        <div className="try-grid">
          {visible.map((item) => (
            <article className="try-card" key={item.id} data-status={item.status}>
              <div className="try-card-head">
                <span className="status-dot" />
                <span className="tiny-label">{item.status.toLowerCase()}</span>
                {item.category ? <span className="pill">{item.category}</span> : null}
              </div>
              <h2>{item.title}</h2>
              {item.notes ? <p>{item.notes}</p> : null}
              <div className="try-meta">
                <span>{item.effort.toLowerCase()} effort</span>
                <span>{item.cost.toLowerCase()} cost</span>
                <span>{item.social_mode.toLowerCase()}</span>
                <span>{item.energy.toLowerCase()} energy</span>
              </div>
              {item.scheduled_for ? (
                <div className="try-date">
                  <CalendarBlank size={14} />
                  {formatDayMonth(item.scheduled_for)}
                  {item.scheduled_day_part
                    ? ` · ${item.scheduled_day_part.toLowerCase()}`
                    : ""}
                </div>
              ) : null}
              <div className="try-actions">
                {item.status === "TRIED" ? (
                  <span className="inline-note">
                    {item.rating ? `${item.rating}/5` : "Tried"}
                  </span>
                ) : (
                  <>
                    <button
                      type="button"
                      className="btn btn-ghost btn-sm"
                      onClick={() => setScheduling(item)}
                    >
                      <CalendarBlank size={13} /> {item.scheduled_for ? "Move" : "Schedule"}
                    </button>
                    {item.scheduled_for ? (
                      <button
                        type="button"
                        className="quiet-link"
                        onClick={() => void unschedule(item)}
                      >
                        Unschedule
                      </button>
                    ) : null}
                    <button
                      type="button"
                      className="quiet-link"
                      onClick={() => void skip(item)}
                    >
                      Skip
                    </button>
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      onClick={() => setTrying(item)}
                    >
                      <Check size={13} /> Tried it
                    </button>
                  </>
                )}
              </div>
            </article>
          ))}
        </div>
      )}

      <div className="daily-foot">
        <span>Schedule only what you want to give a real day to.</span>
        <button type="button" className="quiet-link" onClick={() => useFlow.getState().navigate("daily")}>
          Open Daily <ArrowRight size={14} />
        </button>
      </div>

      {addOpen ? <TryEditor onClose={() => setAddOpen(false)} /> : null}
      {scheduling ? (
        <TryScheduleDialog
          item={scheduling}
          onClose={() => setScheduling(null)}
        />
      ) : null}
      {trying ? (
        <TryTriedDialog
          item={trying}
          onClose={() => setTrying(null)}
          onSave={(rating, reflection) => void markTried(trying, rating, reflection)}
        />
      ) : null}
    </div>
  );
}

function TryEditor({ onClose }: { onClose: () => void }) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [category, setCategory] = useState("");
  const [effort, setEffort] = useState<TryEffort>("MEDIUM");
  const [cost, setCost] = useState<TryCost>("LOW");
  const [social, setSocial] = useState<SocialMode>("EITHER");
  const [energy, setEnergy] = useState<EnergyLevel>("MEDIUM");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    if (!title.trim()) {
      setError("Give the idea a name.");
      return;
    }
    setSaving(true);
    setError(null);
    const input: TryItemInput = {
      title: title.trim(),
      notes: notes.trim() || null,
      area_id: null,
      category: category.trim() || null,
      link: null,
      effort,
      cost,
      social_mode: social,
      energy,
    };
    const result = await act<TryItem>("create_try_item", { input });
    setSaving(false);
    if (result) {
      notify("Idea added to the shelf.", "success");
      onClose();
    }
  };

  return (
    <Modal
      kicker="New things to try"
      title="Add an idea"
      onClose={onClose}
      footer={
        <>
          <span className="inline-note left">No date yet. That is allowed.</span>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={saving}
            onClick={() => void save()}
          >
            {saving ? "Adding." : "Add idea"}
          </button>
        </>
      }
    >
      {error ? <div className="form-error">{error}</div> : null}
      <div className="form-grid">
        <Field label="What do you want to try?">
          <input
            className="input"
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="Take a pottery class"
          />
        </Field>
        <Field label="Category" hint="Optional">
          <input
            className="input"
            value={category}
            onChange={(event) => setCategory(event.target.value)}
            placeholder="Make, move, learn"
          />
        </Field>
        <Field label="Notes" hint="Optional">
          <textarea
            className="text-area"
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
          />
        </Field>
        <div className="form-grid cols-2">
          <Field label="Effort">
            <select className="input" value={effort} onChange={(event) => setEffort(event.target.value as TryEffort)}>
              <option value="LOW">Low</option>
              <option value="MEDIUM">Medium</option>
              <option value="HIGH">High</option>
            </select>
          </Field>
          <Field label="Cost">
            <select className="input" value={cost} onChange={(event) => setCost(event.target.value as TryCost)}>
              <option value="FREE">Free</option>
              <option value="LOW">Low</option>
              <option value="MEDIUM">Medium</option>
              <option value="HIGH">High</option>
            </select>
          </Field>
          <Field label="Social">
            <select className="input" value={social} onChange={(event) => setSocial(event.target.value as SocialMode)}>
              <option value="EITHER">Either</option>
              <option value="SOLO">Solo</option>
              <option value="SOCIAL">Social</option>
            </select>
          </Field>
          <Field label="Energy">
            <select className="input" value={energy} onChange={(event) => setEnergy(event.target.value as EnergyLevel)}>
              <option value="LOW">Low</option>
              <option value="MEDIUM">Medium</option>
              <option value="HIGH">High</option>
            </select>
          </Field>
        </div>
      </div>
    </Modal>
  );
}

function TryScheduleDialog({ item, onClose }: { item: TryItem; onClose: () => void }) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [date, setDate] = useState(item.scheduled_for ?? "");
  const [part, setPart] = useState<TimeOfDay>(item.scheduled_day_part ?? "ANYTIME");
  const [saving, setSaving] = useState(false);

  const save = async () => {
    if (!date) {
      return;
    }
    setSaving(true);
    const result = await act<TryItem>("schedule_try_item", {
      id: item.id,
      day: date,
      dayPart: part,
    });
    setSaving(false);
    if (result) {
      notify("A day to try it.", "success", formatDayMonth(date));
      onClose();
    }
  };

  return (
    <Modal
      kicker="Give it a day"
      title={item.title}
      onClose={onClose}
      footer={
        <>
          <span className="inline-note left">One day, one thing.</span>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn btn-primary" disabled={saving} onClick={() => void save()}>
            {saving ? "Saving." : "Schedule"}
          </button>
        </>
      }
    >
      <div className="form-grid">
        <Field label="Date">
          <input className="input" type="date" value={date} onChange={(event) => setDate(event.target.value)} />
        </Field>
        <Field label="Day part">
          <select className="input" value={part} onChange={(event) => setPart(event.target.value as TimeOfDay)}>
            <option value="ANYTIME">Anytime</option>
            <option value="MORNING">Morning</option>
            <option value="AFTERNOON">Afternoon</option>
            <option value="EVENING">Evening</option>
          </select>
        </Field>
      </div>
    </Modal>
  );
}

function TryTriedDialog({
  item,
  onClose,
  onSave,
}: {
  item: TryItem;
  onClose: () => void;
  onSave: (rating: number | null, reflection: string | null) => void;
}) {
  const [rating, setRating] = useState<number | null>(item.rating);
  const [reflection, setReflection] = useState(item.reflection ?? "");
  return (
    <Modal
      kicker="You tried it"
      title={item.title}
      onClose={onClose}
      footer={
        <>
          <span className="inline-note left">A small note is enough.</span>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => onSave(rating, reflection.trim() || null)}
          >
            Mark tried
          </button>
        </>
      }
    >
      <div className="form-grid">
        <Field label="How was it?">
          <div className="rating-row">
            {[1, 2, 3, 4, 5].map((value) => (
              <button
                key={value}
                type="button"
                className="rating-button"
                data-active={rating === value}
                onClick={() => setRating(value)}
              >
                {value}
              </button>
            ))}
          </div>
        </Field>
        <Field label="A note for next time" hint="Optional">
          <textarea
            className="text-area"
            value={reflection}
            onChange={(event) => setReflection(event.target.value)}
          />
        </Field>
      </div>
    </Modal>
  );
}
