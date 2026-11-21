import { useEffect, useRef } from "react";
import type { ParameterId } from "../generated/ParameterId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SimulationState } from "../store/simulation";
import { ComponentParameterFields } from "./ComponentParameterFields";
import { DesignatorField } from "./DesignatorField";

export interface ComponentEditorProps {
  readonly state: SimulationState;
  readonly schematic: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly componentId: StableId;
  /** Where the panel sits in its container, in pixels. */
  readonly position: { readonly left: number; readonly top: number };
  /** False while a project operation is pending. */
  readonly enabled: boolean;
  /** The assignment locks this part: its values are shown but cannot be edited. */
  readonly locked: boolean;
  readonly onCommit: (
    componentId: StableId,
    parameterId: ParameterId,
    value: number,
  ) => void;
  readonly onClear: (componentId: StableId, parameterId: ParameterId) => void;
  /**
   * Stores the part's designator and returns why it was refused, or null. Left out while an
   * assignment is open: assignments hold no documentation, so the field is not offered.
   */
  readonly onSetDesignator?: (
    componentId: StableId,
    text: string,
  ) => string | null;
  /** Escape or the close button: the host returns the focus to the canvas. */
  readonly onClose: () => void;
}

const FOCUSABLE = "input:not([disabled]), select:not([disabled])";

/**
 * The parameters of one component, opened by a double-click or Enter on the canvas
 * (docs/contracts/stage-11-drawer-and-editing.md section 5). It is not modal: the circuit keeps
 * running and the readings keep updating. It renders the same fields as the Inspector, through
 * the same callbacks, so it has no validation or command path of its own.
 */
export function ComponentEditor({
  state,
  schematic,
  labels,
  componentId,
  position,
  enabled,
  locked,
  onCommit,
  onClear,
  onSetDesignator,
  onClose,
}: ComponentEditorProps) {
  const body = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const component = schematic.components.find(({ id }) => id === componentId);
  const { catalog } = state;
  const label = component ? (labels.get(component.id) ?? component.typeId) : "";
  const designators = new Map(
    schematic.documentation.designators.map((entry) => [
      entry.componentId,
      entry.designator,
    ]),
  );

  // Opening moves the focus to the first field; a part with no field focuses Close. A double
  // press opens the editor while the browser is still handling the press, and the browser then
  // gives the canvas the focus, so the focus moves on the next frame.
  useEffect(() => {
    const frame = window.requestAnimationFrame(() =>
      (
        body.current?.querySelector<HTMLElement>(FOCUSABLE) ??
        closeButton.current
      )?.focus({ preventScroll: true }),
    );
    return () => window.cancelAnimationFrame(frame);
  }, [componentId]);

  if (!component || catalog === null) {
    return null;
  }

  const entry = catalog.componentTypes.find(
    (candidate) => candidate.typeId === component.typeId,
  );

  return (
    <section
      className="component-editor"
      role="dialog"
      aria-modal="false"
      aria-label={`Edit ${label}`}
      style={position}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="component-editor-header">
        <h2>{label}</h2>
        <button
          ref={closeButton}
          type="button"
          className="toolbar-button"
          aria-label={`Close the editor of ${label}`}
          onClick={onClose}
        >
          Close
        </button>
      </div>
      <p className="visually-hidden" role="status">
        Editing {label}
      </p>
      {locked && (
        <p className="parameter-hint">
          {label} is locked by the assignment. Parts locked by the assignment
          cannot be edited.
        </p>
      )}
      <div ref={body}>
        {onSetDesignator !== undefined && (
          <DesignatorField
            label={label}
            committed={designators.get(component.id)}
            others={[...designators]
              .filter(([id]) => id !== component.id)
              .map(([, name]) => name)}
            disabled={!enabled}
            onCommit={(text) => onSetDesignator(component.id, text)}
          />
        )}
        {entry && entry.parameters.length > 0 ? (
          <ComponentParameterFields
            catalog={catalog}
            component={component}
            label={label}
            editable={enabled && !locked && state.status !== "loading"}
            onCommit={onCommit}
            onClear={onClear}
          />
        ) : (
          <p className="panel-note">This part has no parameters.</p>
        )}
      </div>
    </section>
  );
}
