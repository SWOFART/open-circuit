import { useId } from "react";
import type { CheckCriterion } from "../../generated/CheckCriterion";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { ComponentTypeId } from "../../generated/ComponentTypeId";
import type { DeviceStateName } from "../../generated/DeviceStateName";
import type { Schematic } from "../../generated/Schematic";
import type { StableId } from "../../generated/StableId";
import type { Unit } from "../../generated/Unit";
import type { AssignmentError } from "../../generated/AssignmentError";
import { message } from "../messages";
import { formatSi, parseSi, unitSymbols } from "../units";
import {
  checkTitle,
  describeAssignmentError,
  deviceStatesFor,
  deviceStateWords,
  formatSeconds,
  hasErrorAt,
  parseSeconds,
  type SignalOption,
} from "./assignmentView";
import { NumberField, type NumberParse } from "./NumberField";

/** A copy of `value` without `key`, which must be an optional field. */
function without<T extends object>(value: T, key: keyof T): T {
  return Object.fromEntries(
    Object.entries(value).filter(([name]) => name !== key),
  ) as T;
}

const parseRatio = (text: string): NumberParse => {
  const trimmed = text.trim();
  const value = Number(trimmed);
  return trimmed !== "" && Number.isFinite(value)
    ? { ok: true, value }
    : { ok: false };
};

const parseCount = (text: string): NumberParse =>
  /^\d+$/.test(text.trim())
    ? { ok: true, value: Number(text.trim()) }
    : { ok: false };

const formatPlain = (value: number): string => String(value);

function unitParse(unit: Unit): (text: string) => NumberParse {
  return (text) => parseSi(text, unit);
}

function unitFormat(unit: Unit): (value: number) => string {
  return (value) => formatSi(value, unit);
}

/** Stands for a count without an upper limit in a number field; no count is negative. */
const NO_LIMIT = -1;

const SECONDS_HINT = "Enter a time, for example 5 ms or 0.002.";

export interface CheckEditorProps {
  readonly criterion: CheckCriterion;
  readonly index: number;
  /** The Engine's errors for this check. */
  readonly errors: readonly AssignmentError[];
  readonly allErrors: readonly AssignmentError[];
  readonly signals: readonly SignalOption[];
  readonly startingCircuit: Schematic | null;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly catalog: ComponentCatalog | null;
  readonly disabled: boolean;
  readonly onChange: (criterion: CheckCriterion) => void;
  readonly onRemove: () => void;
}

function Select<T extends string>({
  label,
  value,
  options,
  invalid,
  disabled,
  onChange,
}: {
  readonly label: string;
  readonly value: T;
  readonly options: readonly { readonly value: T; readonly label: string }[];
  readonly invalid: boolean;
  readonly disabled: boolean;
  readonly onChange: (value: T) => void;
}) {
  const id = useId();
  // A value the choices do not offer (a part that is gone) stays visible, not silently replaced.
  const offered = options.some((option) => option.value === value);

  return (
    <div className="parameter-field">
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={value}
        disabled={disabled}
        aria-invalid={invalid}
        onChange={(event) => onChange(event.target.value as T)}
      >
        {!offered && <option value={value}>(not available) {value}</option>}
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}

/** One check of a `build` task, with controls for its fields and the Engine's errors. */
export function CheckEditor({
  criterion,
  index,
  errors,
  allErrors,
  signals,
  startingCircuit,
  labels,
  catalog,
  disabled,
  onChange,
  onRemove,
}: CheckEditorProps) {
  const labelId = useId();
  const invalid = (field: string): boolean =>
    hasErrorAt(allErrors, "checks", field, index);
  const signalOptions = signals.map((signal) => ({
    value: signal.key,
    label: signal.label,
  }));
  const unitOf = (key: string): Unit | undefined =>
    signals.find((signal) => signal.key === key)?.unit;

  const signalControls = (
    current: { key: string; unit: Unit },
    update: (signal: { key: string; unit: Unit }) => void,
  ) => (
    <Select
      label="Signal"
      value={current.key}
      options={signalOptions}
      invalid={invalid("signal")}
      disabled={disabled}
      onChange={(key) => update({ key, unit: unitOf(key) ?? current.unit })}
    />
  );

  const fields = (() => {
    switch (criterion.kind) {
      case "value": {
        const unit = criterion.signal.unit;
        return (
          <>
            {signalControls(criterion.signal, (signal) =>
              onChange({ ...criterion, signal }),
            )}
            <NumberField
              label="At time"
              value={criterion.atSeconds}
              parse={parseSeconds}
              format={formatSeconds}
              hint={SECONDS_HINT}
              engineError={invalid("atSeconds")}
              disabled={disabled}
              onCommit={(atSeconds) => onChange({ ...criterion, atSeconds })}
            />
            <NumberField
              label={`Expected value (${unitSymbols[unit]})`}
              value={criterion.expected}
              parse={unitParse(unit)}
              format={unitFormat(unit)}
              formatKey={unit}
              hint={`Enter a number, optionally with a prefix, for example 2.5 or 4.7k.`}
              engineError={invalid("expected")}
              disabled={disabled}
              onCommit={(expected) => onChange({ ...criterion, expected })}
            />
            <NumberField
              label={`Absolute tolerance (${unitSymbols[unit]})`}
              value={criterion.toleranceAbs}
              parse={unitParse(unit)}
              format={unitFormat(unit)}
              formatKey={unit}
              hint="Enter a number that is not negative, for example 25m."
              engineError={invalid("toleranceAbs")}
              disabled={disabled}
              onCommit={(toleranceAbs) =>
                onChange({ ...criterion, toleranceAbs })
              }
            />
            <NumberField
              label="Relative tolerance (fraction of the expected value)"
              value={criterion.toleranceRel}
              parse={parseRatio}
              format={formatPlain}
              hint="Enter a fraction that is not negative, for example 0.01 for 1 %."
              engineError={invalid("toleranceRel")}
              disabled={disabled}
              onCommit={(toleranceRel) =>
                onChange({ ...criterion, toleranceRel })
              }
            />
          </>
        );
      }
      case "range": {
        const unit = criterion.signal.unit;
        return (
          <>
            {signalControls(criterion.signal, (signal) =>
              onChange({ ...criterion, signal }),
            )}
            <NumberField
              label="From time"
              value={criterion.fromSeconds}
              parse={parseSeconds}
              format={formatSeconds}
              hint={SECONDS_HINT}
              engineError={invalid("fromSeconds")}
              disabled={disabled}
              onCommit={(fromSeconds) =>
                onChange({ ...criterion, fromSeconds })
              }
            />
            <NumberField
              label="To time"
              value={criterion.toSeconds}
              parse={parseSeconds}
              format={formatSeconds}
              hint={SECONDS_HINT}
              engineError={invalid("toSeconds")}
              disabled={disabled}
              onCommit={(toSeconds) => onChange({ ...criterion, toSeconds })}
            />
            <NumberField
              label={`Minimum (${unitSymbols[unit]})`}
              value={criterion.min}
              parse={unitParse(unit)}
              format={unitFormat(unit)}
              formatKey={unit}
              hint="Enter a number, optionally with a prefix, for example 2.4."
              engineError={invalid("min")}
              disabled={disabled}
              onCommit={(min) => onChange({ ...criterion, min })}
            />
            <NumberField
              label={`Maximum (${unitSymbols[unit]})`}
              value={criterion.max}
              parse={unitParse(unit)}
              format={unitFormat(unit)}
              formatKey={unit}
              hint="Enter a number, optionally with a prefix, for example 2.6."
              engineError={invalid("max")}
              disabled={disabled}
              onCommit={(max) => onChange({ ...criterion, max })}
            />
          </>
        );
      }
      case "deviceState": {
        const parts = (startingCircuit?.components ?? []).filter(
          (component) => deviceStatesFor(component.typeId).length > 0,
        );
        const part = parts.find(
          (component) => component.id === criterion.componentId,
        );
        const states: DeviceStateName[] = part
          ? deviceStatesFor(part.typeId)
          : [criterion.state];
        return (
          <>
            <Select
              label="Part"
              value={criterion.componentId}
              options={parts.map((component) => ({
                value: component.id,
                label: labels.get(component.id) ?? component.typeId,
              }))}
              invalid={invalid("componentId")}
              disabled={disabled}
              onChange={(componentId) => {
                const next = parts.find(
                  (component) => component.id === componentId,
                );
                const allowed = next ? deviceStatesFor(next.typeId) : [];
                onChange({
                  ...criterion,
                  componentId,
                  state: allowed.includes(criterion.state)
                    ? criterion.state
                    : (allowed[0] ?? criterion.state),
                });
              }}
            />
            <Select
              label="State"
              value={criterion.state}
              options={states.map((state) => ({
                value: state,
                label: deviceStateWords[state],
              }))}
              invalid={invalid("state")}
              disabled={disabled}
              onChange={(state) => onChange({ ...criterion, state })}
            />
            <NumberField
              label="At time"
              value={criterion.atSeconds}
              parse={parseSeconds}
              format={formatSeconds}
              hint={SECONDS_HINT}
              engineError={invalid("atSeconds")}
              disabled={disabled}
              onCommit={(atSeconds) => onChange({ ...criterion, atSeconds })}
            />
          </>
        );
      }
      case "componentCount": {
        const types = catalog?.componentTypes ?? [];
        return (
          <>
            <Select<ComponentTypeId>
              label="Part type"
              value={criterion.typeId}
              options={types.map((entry) => ({
                value: entry.typeId,
                label: message(entry.displayNameKey),
              }))}
              invalid={invalid("typeId")}
              disabled={disabled}
              onChange={(typeId) => onChange({ ...criterion, typeId })}
            />
            <NumberField
              label="At least"
              value={criterion.min}
              parse={parseCount}
              format={formatPlain}
              hint="Enter a whole number of parts."
              engineError={invalid("min")}
              disabled={disabled}
              onCommit={(min) => onChange({ ...criterion, min })}
            />
            <NumberField
              label="At most (leave empty for no limit)"
              value={criterion.max ?? NO_LIMIT}
              parse={(text) =>
                text.trim() === ""
                  ? { ok: true, value: NO_LIMIT }
                  : parseCount(text)
              }
              format={(value) => (value === NO_LIMIT ? "" : formatPlain(value))}
              hint="Enter a whole number of parts, or nothing."
              engineError={invalid("max")}
              disabled={disabled}
              onCommit={(max) => {
                const rest = without(criterion, "max");
                onChange(max === NO_LIMIT ? rest : { ...rest, max });
              }}
            />
          </>
        );
      }
    }
  })();

  return (
    <fieldset className="check-editor" aria-labelledby={labelId}>
      <legend id={labelId}>{checkTitle(criterion, index)}</legend>
      <div className="parameter-field">
        <label htmlFor={`${labelId}-label`}>Name (optional)</label>
        <input
          id={`${labelId}-label`}
          type="text"
          maxLength={120}
          value={criterion.label ?? ""}
          disabled={disabled}
          aria-invalid={invalid("label")}
          onChange={(event) => {
            const rest = without(criterion, "label");
            const label = event.target.value;
            onChange(label === "" ? rest : { ...rest, label });
          }}
        />
      </div>
      {fields}
      {errors.length > 0 && (
        <ul className="field-error-list" role="alert">
          {errors.map((error, position) => (
            <li key={`${error.path}-${position}`}>
              {describeAssignmentError(error)}
            </li>
          ))}
        </ul>
      )}
      <button
        type="button"
        className="toolbar-button"
        disabled={disabled}
        onClick={onRemove}
      >
        Remove check {index + 1}
      </button>
    </fieldset>
  );
}
