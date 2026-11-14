import { useEffect, useId, useState } from "react";
import type { Diagnostic } from "../generated/Diagnostic";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { nextRailName } from "../editor/railRules";
import { describeDiagnostic } from "./messages";

type Rail = Schematic["rails"][number];

/** What the panel asks the document for; a returned string is the reason it was refused. */
export interface RailFields {
  readonly name: string;
  readonly x: number;
  readonly top: number;
  readonly bottom: number;
}

export interface RailPanelProps {
  readonly rails: readonly Rail[];
  readonly selectedId: StableId | null;
  readonly diagnostics: readonly Diagnostic[];
  readonly disabled?: boolean;
  /** Why rails cannot be edited now (an assignment is open); null when they can. */
  readonly unavailable?: string | null;
  readonly onSelect: (id: StableId | null) => void;
  readonly onCreate: (fields: RailFields) => string | null;
  readonly onEdit: (id: StableId, patch: Partial<RailFields>) => string | null;
  readonly onDelete: (id: StableId) => void;
}

/** A whole number on the grid, or null for anything else. */
export function parseGridInteger(text: string): number | null {
  const trimmed = text.trim();
  return /^-?\d+$/.test(trimmed) ? Number(trimmed) : null;
}

interface FieldProps {
  readonly label: string;
  readonly value: string;
  readonly disabled: boolean;
  /** Commits a draft; returns the reason it was refused, or null. */
  readonly onCommit: (draft: string) => string | null;
  readonly hint?: string;
}

/**
 * A text field that commits on Enter or when it loses focus. A refused draft stays in the
 * field with its reason, and nothing is edited; Escape puts the committed value back.
 */
function RailField({ label, value, disabled, onCommit, hint }: FieldProps) {
  const id = useId();
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState<string | null>(null);

  // The committed value changed elsewhere (undo, a drag): show it.
  useEffect(() => {
    setDraft(value);
    setError(null);
  }, [value]);

  const commit = (): void => {
    if (draft === value) {
      setError(null);
      return;
    }

    setError(onCommit(draft));
  };

  return (
    <div className="parameter-field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type="text"
        value={draft}
        disabled={disabled}
        aria-invalid={error !== null}
        aria-describedby={error !== null ? `${id}-error` : undefined}
        spellCheck={false}
        autoComplete="off"
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit();
          } else if (event.key === "Escape") {
            setDraft(value);
            setError(null);
          }
        }}
      />
      {hint !== undefined && error === null && (
        <p className="parameter-hint">{hint}</p>
      )}
      {error !== null && (
        <p id={`${id}-error`} className="parameter-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

function CreateForm({
  rails,
  disabled,
  onCreate,
}: {
  readonly rails: readonly Rail[];
  readonly disabled: boolean;
  readonly onCreate: (fields: RailFields) => string | null;
}) {
  const suggested = nextRailName(rails.map((rail) => rail.name));
  const [name, setName] = useState(suggested);
  const [x, setX] = useState("0");
  const [top, setTop] = useState("0");
  const [bottom, setBottom] = useState("20");
  const [error, setError] = useState<string | null>(null);
  const ids = useId();

  // After a rail is created the next default name moves on.
  useEffect(() => {
    setName(suggested);
  }, [suggested]);

  const submit = (): void => {
    const [px, ptop, pbottom] = [x, top, bottom].map(parseGridInteger);
    if (px === null || ptop === null || pbottom === null) {
      setError("Enter whole numbers for x, top y and bottom y.");
      return;
    }

    setError(
      onCreate({
        name,
        x: px as number,
        top: ptop as number,
        bottom: pbottom as number,
      }),
    );
  };

  const field = (
    label: string,
    value: string,
    set: (next: string) => void,
    suffix: string,
  ) => (
    <div className="parameter-field">
      <label htmlFor={`${ids}-${suffix}`}>{label}</label>
      <input
        id={`${ids}-${suffix}`}
        type="text"
        value={value}
        disabled={disabled}
        spellCheck={false}
        autoComplete="off"
        onChange={(event) => set(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            submit();
          }
        }}
      />
    </div>
  );

  return (
    <form
      className="rail-create"
      aria-label="Create a rail"
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      {field("New rail name", name, setName, "name")}
      {field("New rail x", x, setX, "x")}
      {field("New rail top y", top, setTop, "top")}
      {field("New rail bottom y", bottom, setBottom, "bottom")}
      <button type="submit" className="toolbar-button" disabled={disabled}>
        Create rail
      </button>
      {error !== null && (
        <p className="parameter-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

/** The diagnostics that name this rail, in the Engine's words. */
function railMessages(
  diagnostics: readonly Diagnostic[],
  railId: StableId,
): string[] {
  return diagnostics
    .filter((diagnostic) =>
      diagnostic.references.some(
        (reference) => reference.kind === "rail" && reference.railId === railId,
      ),
    )
    .map((diagnostic) => describeDiagnostic(diagnostic));
}

/**
 * DC rails: the list of the document's rails, the properties of the selected one, and a form to
 * create one without the pointer. A rail is a named vertical conductor; it has no voltage of
 * its own, and a name such as L1, N or PE means nothing to the simulation.
 */
export function RailPanel({
  rails,
  selectedId,
  diagnostics,
  disabled = false,
  unavailable = null,
  onSelect,
  onCreate,
  onEdit,
  onDelete,
}: RailPanelProps) {
  const locked = disabled || unavailable !== null;
  const selected = rails.find((rail) => rail.id === selectedId) ?? null;

  return (
    <section className="side-panel" aria-labelledby="rails-heading">
      <h2 id="rails-heading">DC rails</h2>
      <p className="panel-note">
        A rail is a named vertical conductor in a direct-current circuit. It
        carries no voltage of its own: put a DC source between rails. Names such
        as L1, L2, L3, N or PE are plain labels and add no phase, neutral or
        earth behavior.
      </p>
      {unavailable !== null && (
        <p className="panel-error" role="status">
          {unavailable}
        </p>
      )}

      {rails.length === 0 ? (
        <p className="panel-note">No rails yet.</p>
      ) : (
        <ul className="rail-list" aria-label="Rails in the drawing">
          {rails.map((rail) => (
            <li key={rail.id}>
              <button
                type="button"
                className="toolbar-button"
                aria-pressed={rail.id === selectedId}
                onClick={() =>
                  onSelect(rail.id === selectedId ? null : rail.id)
                }
              >
                {rail.name} (x {rail.start.x}, y {rail.start.y} to {rail.end.y})
              </button>
            </li>
          ))}
        </ul>
      )}

      {selected !== null && (
        <div
          className="rail-properties"
          role="group"
          aria-label="Rail properties"
        >
          <RailField
            label="Rail name"
            value={selected.name}
            disabled={locked}
            hint="1 to 32 letters, digits, _, + or -; starts with a letter or digit."
            onCommit={(draft) => onEdit(selected.id, { name: draft })}
          />
          <RailField
            label="Rail x"
            value={String(selected.start.x)}
            disabled={locked}
            onCommit={(draft) => {
              const value = parseGridInteger(draft);
              return value === null
                ? "Enter a whole number."
                : onEdit(selected.id, { x: value });
            }}
          />
          <RailField
            label="Rail top y"
            value={String(selected.start.y)}
            disabled={locked}
            onCommit={(draft) => {
              const value = parseGridInteger(draft);
              return value === null
                ? "Enter a whole number."
                : onEdit(selected.id, { top: value });
            }}
          />
          <RailField
            label="Rail bottom y"
            value={String(selected.end.y)}
            disabled={locked}
            onCommit={(draft) => {
              const value = parseGridInteger(draft);
              return value === null
                ? "Enter a whole number."
                : onEdit(selected.id, { bottom: value });
            }}
          />
          {railMessages(diagnostics, selected.id).map((text) => (
            <p key={text} className="parameter-error" role="alert">
              {text}
            </p>
          ))}
          <button
            type="button"
            className="toolbar-button"
            disabled={locked}
            onClick={() => onDelete(selected.id)}
          >
            Delete rail
          </button>
        </div>
      )}

      <CreateForm rails={rails} disabled={locked} onCreate={onCreate} />
    </section>
  );
}
