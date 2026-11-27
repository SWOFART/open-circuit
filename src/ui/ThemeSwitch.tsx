import { THEME_MODES, type ThemeMode } from "../theme/preference";

export interface ThemeSwitchProps {
  readonly mode: ThemeMode;
  readonly onChange: (mode: ThemeMode) => void;
}

const MODE_NAMES: Readonly<Record<ThemeMode, string>> = {
  system: "System",
  light: "Light",
  dark: "Dark",
};

/**
 * The choice of theme: System (follows the operating system), Light or Dark. The mode is named
 * in words, so no meaning rests on a color or an icon.
 */
export function ThemeSwitch({ mode, onChange }: ThemeSwitchProps) {
  return (
    <label className="theme-switch">
      <span>Theme</span>
      <select
        aria-label="Theme"
        value={mode}
        onChange={(event) => onChange(event.target.value as ThemeMode)}
      >
        {THEME_MODES.map((value) => (
          <option key={value} value={value}>
            {MODE_NAMES[value]}
          </option>
        ))}
      </select>
    </label>
  );
}
