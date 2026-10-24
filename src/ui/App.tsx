import { invoke, isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import { EditorCore } from "../editor/EditorCore";

export function App() {
  const canvasHost = useRef<HTMLDivElement>(null);
  const [editorError, setEditorError] = useState<string | null>(null);
  const [catalog, setCatalog] = useState<ComponentCatalog | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);

  useEffect(() => {
    const host = canvasHost.current;

    if (!host) {
      return;
    }

    const editor = new EditorCore();
    void editor.mount(host).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      setEditorError(message);
    });

    return () => editor.destroy();
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      return;
    }

    let cancelled = false;
    void invoke<ComponentCatalog>("get_component_catalog")
      .then((result) => {
        if (!cancelled) {
          setCatalog(result);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          const message =
            error instanceof Error ? error.message : String(error);
          setCatalogError(message);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="workspace">
      <header className="app-header">
        <div>
          <p className="eyebrow">OPEN CIRCUIT</p>
          <h1>Circuit workspace</h1>
        </div>
        <p className="shell-status">Desktop shell ready</p>
      </header>

      <section className="editor-panel" aria-label="Circuit editor">
        <div className="editor-toolbar">
          <span>Untitled circuit</span>
          <span className="editor-hint">Editor scaffold</span>
        </div>
        <div ref={canvasHost} className="editor-canvas" />
        {editorError && (
          <p className="editor-error" role="alert">
            The canvas could not be initialized: {editorError}
          </p>
        )}
      </section>

      <footer className="app-footer">
        {catalog ? (
          <span>
            Rust catalog:{" "}
            {catalog.componentTypes.map((item) => item.typeId).join(", ")}
          </span>
        ) : (
          <span>Rust catalog is available in the desktop app.</span>
        )}
        {catalogError && (
          <span role="alert">Catalog unavailable: {catalogError}</span>
        )}
        <span>Simulation solver is not connected yet.</span>
      </footer>
    </main>
  );
}
