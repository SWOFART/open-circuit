import { useEffect, useRef } from "react";
import {
  matchShortcut,
  SHORTCUT_GROUPS,
  shortcutKeys,
  shortcutsInGroup,
  type Platform,
} from "../editor/shortcuts";
import { message } from "./messages";

export interface ShortcutWindowProps {
  /** The platform whose key names are shown (Cmd on a Mac, Ctrl elsewhere). */
  readonly platform: Platform;
  readonly onClose: () => void;
}

const FOCUSABLE =
  'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])';

/**
 * The window of every keyboard shortcut (stage 16 contract, section 10). It is generated from the
 * same table the key handlers read, so a binding that works is listed and a listed binding works.
 * It is a modal dialog: labelled, with a focus trap, closed by Escape and by its button, and it
 * returns the focus to what opened it. Every key is kept from the application behind it.
 */
export function ShortcutWindow({ platform, onClose }: ShortcutWindowProps) {
  const dialog = useRef<HTMLElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    // What had the focus when the window opened gets it back when it closes.
    const opener =
      typeof document === "undefined" ? null : document.activeElement;
    closeButton.current?.focus();
    return () => {
      if (opener instanceof HTMLElement) opener.focus();
    };
  }, []);

  const onKeyDown = (event: React.KeyboardEvent): void => {
    // The window is modal: no key reaches the canvas or the file shortcuts behind it.
    event.stopPropagation();
    const binding = matchShortcut(event, "dialog", undefined, true);
    if (binding === "dialog.close") {
      event.preventDefault();
      onClose();
      return;
    }
    if (binding !== "dialog.focus") return;

    const focusable = [
      ...(dialog.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? []),
    ];
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!first || !last) return;

    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  return (
    <div className="project-dialog-backdrop">
      <section
        ref={dialog}
        className="project-dialog shortcut-window"
        role="dialog"
        aria-modal="true"
        aria-labelledby="shortcut-window-title"
        onKeyDown={onKeyDown}
      >
        <h2 id="shortcut-window-title">Keyboard shortcuts</h2>
        <div
          className="shortcut-groups"
          tabIndex={0}
          role="region"
          aria-label="Shortcuts by group"
        >
          {SHORTCUT_GROUPS.map((group) => {
            const entries = shortcutsInGroup(group);
            return entries.length === 0 ? null : (
              <section key={group} aria-labelledby={`shortcut-group-${group}`}>
                <h3 id={`shortcut-group-${group}`}>{group}</h3>
                <dl>
                  {entries.map((entry) => (
                    <div key={entry.id} className="shortcut-entry">
                      <dt>{message(entry.description)}</dt>
                      <dd>
                        {shortcutKeys(entry, platform).map((keys, index) => (
                          <span key={keys}>
                            {index > 0 && " or "}
                            <kbd>{keys}</kbd>
                          </span>
                        ))}
                      </dd>
                    </div>
                  ))}
                </dl>
              </section>
            );
          })}
        </div>
        <div className="project-dialog-actions">
          <button
            ref={closeButton}
            type="button"
            className="toolbar-button"
            onClick={onClose}
          >
            Close
          </button>
        </div>
      </section>
    </div>
  );
}
