import {
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { EditorCommand } from "../editor/commands";
import { EditorCore } from "../editor/EditorCore";
import type { EditorStatus } from "../editor/interaction";
import {
  DIVIDER_OUTPUT_COMPONENT_ID,
  dividerSchematic,
} from "../store/fixtures";
import { SchematicDocumentStore } from "../store/document";
import { tauriProjectFiles, type ProjectFilePort } from "../store/projectFiles";
import {
  ProjectWorkflow,
  resolveUnsavedAction,
  type ProjectAction,
  type UnsavedChoice,
} from "../store/projectWorkflow";
import { applyEditorCommand, liveParameterUpdate } from "./editorCommands";
import {
  projectShortcutFromEvent,
  type ProjectShortcut,
} from "./projectShortcuts";
import { Inspector } from "./Inspector";
import { Oscilloscope } from "./Oscilloscope";
import { Palette, toolHint } from "./Palette";
import { UnsavedChangesDialog } from "./UnsavedChangesDialog";
import { ResultPanel } from "./ResultPanel";
import { useOscilloscope } from "./useOscilloscope";
import { useSimulation } from "./useSimulation";

// True only in a development build that explicitly asked for the mock adapter.
const usingMockAdapter =
  import.meta.env.DEV && import.meta.env.VITE_ENGINE_ADAPTER === "mock";

const IDLE_STATUS: EditorStatus = {
  tool: { kind: "select" },
  routing: false,
  rejected: null,
};

interface AppProps {
  readonly projectFiles?: ProjectFilePort;
}

export function App({ projectFiles = tauriProjectFiles }: AppProps = {}) {
  const canvasHost = useRef<HTMLDivElement>(null);
  const editorRef = useRef<EditorCore | null>(null);
  const allowNextCloseRef = useRef(false);
  const pendingActionRef = useRef<ProjectAction | "close" | null>(null);
  const projectShortcutHandler = useRef<(shortcut: ProjectShortcut) => void>(
    () => {},
  );
  const [editorError, setEditorError] = useState<string | null>(null);
  const [editError, setEditError] = useState<string | null>(null);
  const [projectError, setProjectError] = useState<string | null>(null);
  const [projectBusy, setProjectBusy] = useState(false);
  const [pendingAction, setPendingAction] = useState<
    ProjectAction | "close" | null
  >(null);
  const [documentName, setDocumentName] = useState("Example");
  const [status, setStatus] = useState<EditorStatus>(IDLE_STATUS);
  const documentStore = useMemo(
    () => new SchematicDocumentStore(dividerSchematic),
    [],
  );
  const projectWorkflow = useMemo(
    () => new ProjectWorkflow(documentStore, projectFiles),
    [documentStore, projectFiles],
  );
  pendingActionRef.current = pendingAction;
  const document = useSyncExternalStore(
    documentStore.subscribe,
    documentStore.getState,
    documentStore.getState,
  );
  const projectOperationLocked = projectBusy || pendingAction !== null;
  // The store's own copy, so the first document sync below is a no-op.
  const initialSchematic = useMemo(
    () => documentStore.getState().schematic,
    [documentStore],
  );
  const { state, controller, adapter } = useSimulation(initialSchematic);
  const { scope, controller: scopeController } = useOscilloscope(
    adapter,
    controller,
  );
  // Names stay with their components through edits, undo, and redo.
  const labels = document.labels;
  const hint = toolHint(status);

  const saveProject = async (saveAs = false): Promise<boolean> => {
    setProjectError(null);
    try {
      const saved = await projectWorkflow.save(saveAs);
      if (saved) setDocumentName("Project");
      return saved;
    } catch (error) {
      setProjectError(error instanceof Error ? error.message : String(error));
      return false;
    }
  };

  const saveProjectDirectly = async (saveAs = false): Promise<void> => {
    if (projectBusy || pendingActionRef.current !== null) return;
    setProjectBusy(true);
    try {
      await saveProject(saveAs);
    } finally {
      setProjectBusy(false);
    }
  };

  const performProjectAction = async (
    action: ProjectAction,
  ): Promise<boolean> => {
    setProjectError(null);
    try {
      const performed = await projectWorkflow.perform(action);
      if (performed) {
        setDocumentName(
          action === "new"
            ? "Untitled"
            : action === "example"
              ? "Example"
              : "Project",
        );
        editorRef.current?.setTool(IDLE_STATUS.tool);
      }
      return performed;
    } catch (error) {
      setProjectError(error instanceof Error ? error.message : String(error));
      return false;
    }
  };

  const performProjectActionDirectly = async (
    action: ProjectAction,
  ): Promise<void> => {
    if (projectBusy || pendingActionRef.current !== null) return;
    setProjectBusy(true);
    try {
      await performProjectAction(action);
    } finally {
      setProjectBusy(false);
    }
  };

  const requestProjectAction = (action: ProjectAction): void => {
    if (projectBusy || pendingActionRef.current !== null) return;
    if (documentStore.getState().isDirty) {
      setPendingAction(action);
      return;
    }
    void performProjectActionDirectly(action);
  };

  const closeWindow = async (): Promise<boolean> => {
    if (!isTauri()) return false;
    try {
      allowNextCloseRef.current = true;
      await getCurrentWindow().close();
      return true;
    } catch (error) {
      allowNextCloseRef.current = false;
      setProjectError(error instanceof Error ? error.message : String(error));
      return false;
    }
  };

  const resolvePendingAction = async (choice: UnsavedChoice): Promise<void> => {
    const action = pendingActionRef.current;
    if (action === null || projectBusy) return;

    setProjectBusy(true);
    setProjectError(null);
    try {
      await resolveUnsavedAction(
        choice,
        () => saveProject(),
        () =>
          action === "close" ? closeWindow() : performProjectAction(action),
      );
    } catch (error) {
      setProjectError(error instanceof Error ? error.message : String(error));
    } finally {
      setPendingAction(null);
      setProjectBusy(false);
    }
  };

  useEffect(() => {
    projectShortcutHandler.current = (shortcut) => {
      if (projectBusy || pendingActionRef.current !== null) return;
      switch (shortcut) {
        case "new":
          requestProjectAction("new");
          break;
        case "open":
          requestProjectAction("open");
          break;
        case "save":
          void saveProjectDirectly();
          break;
        case "saveAs":
          void saveProjectDirectly(true);
          break;
      }
    };
  });

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const isMacPlatform = /Mac|iPhone|iPad/.test(navigator.platform);
      const shortcut = projectShortcutFromEvent(event, isMacPlatform);
      if (shortcut === null || pendingActionRef.current !== null) return;
      event.preventDefault();
      projectShortcutHandler.current(shortcut);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    if (typeof window === "undefined" || !isTauri()) return;

    let unlisten: (() => void) | undefined;
    let disposed = false;
    void getCurrentWindow()
      .onCloseRequested((event) => {
        if (allowNextCloseRef.current) {
          allowNextCloseRef.current = false;
          return;
        }
        if (!documentStore.getState().isDirty) return;

        event.preventDefault();
        setPendingAction("close");
      })
      .then((stopListening) => {
        if (disposed) stopListening();
        else unlisten = stopListening;
      })
      .catch((error: unknown) => {
        setProjectError(error instanceof Error ? error.message : String(error));
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [documentStore]);

  // The editor is created once; commands always use the latest catalog.
  const commandHandler = useRef<(command: EditorCommand) => void>(() => {});
  useEffect(() => {
    commandHandler.current = (command) => {
      if (projectBusy || pendingActionRef.current !== null) return;
      try {
        applyEditorCommand(documentStore, command, state.catalog);
        setEditError(null);
      } catch (error) {
        setEditError(error instanceof Error ? error.message : String(error));
      }
    };
  });

  useEffect(() => {
    const host = canvasHost.current;

    if (!host) {
      return;
    }

    const editor = new EditorCore({
      onCommand: (command) => commandHandler.current(command),
      onStatusChange: setStatus,
    });
    editorRef.current = editor;
    void editor.mount(host).catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      setEditorError(message);
    });

    return () => {
      editor.destroy();
      editorRef.current = null;
    };
  }, []);

  useEffect(() => {
    editorRef.current?.setSchematic(document.schematic, labels);
  }, [document.schematic, labels]);

  // Every document change reaches the Engine; parameter edits are sent
  // directly by the Inspector, so this then finds nothing new.
  useEffect(() => {
    void controller?.syncDocument(document.schematic);
  }, [controller, document.schematic]);

  return (
    <main className="workspace">
      <header className="app-header">
        <div>
          <p className="eyebrow">OPEN CIRCUIT</p>
          <h1>Circuit workspace</h1>
        </div>
        <p className="shell-status">Desktop shell ready</p>
      </header>

      <div className="workspace-body">
        <section className="editor-panel" aria-label="Circuit editor">
          <div className="editor-toolbar">
            <Palette
              catalog={state.catalog}
              tool={status.tool}
              disabled={projectOperationLocked}
              onSelectTool={(next) => editorRef.current?.setTool(next)}
            />
            <div className="editor-actions">
              <span className="editor-hint">
                {document.isDirty ? "Unsaved changes" : documentName}
              </span>
              <div className="project-actions" aria-label="Project actions">
                <button
                  type="button"
                  className="toolbar-button"
                  disabled={projectOperationLocked}
                  onClick={() => requestProjectAction("new")}
                >
                  New
                </button>
                <button
                  type="button"
                  className="toolbar-button"
                  disabled={projectOperationLocked}
                  onClick={() => requestProjectAction("open")}
                >
                  Open
                </button>
                <button
                  type="button"
                  className="toolbar-button"
                  disabled={projectOperationLocked}
                  onClick={() => void saveProjectDirectly()}
                >
                  Save
                </button>
                <button
                  type="button"
                  className="toolbar-button"
                  disabled={projectOperationLocked}
                  onClick={() => void saveProjectDirectly(true)}
                >
                  Save As
                </button>
                <button
                  type="button"
                  className="toolbar-button"
                  disabled={projectOperationLocked}
                  onClick={() => requestProjectAction("example")}
                >
                  Open Example
                </button>
              </div>
              <button
                type="button"
                className="toolbar-button"
                disabled={projectOperationLocked || !document.canUndo}
                onClick={() => commandHandler.current({ kind: "undo" })}
              >
                Undo
              </button>
              <button
                type="button"
                className="toolbar-button"
                disabled={projectOperationLocked || !document.canRedo}
                onClick={() => commandHandler.current({ kind: "redo" })}
              >
                Redo
              </button>
              <button
                type="button"
                className="toolbar-button"
                onClick={() => editorRef.current?.fitToCircuit()}
              >
                Fit to circuit
              </button>
            </div>
          </div>
          <div
            ref={canvasHost}
            className={
              projectOperationLocked
                ? "editor-canvas editor-canvas-locked"
                : "editor-canvas"
            }
          />
          <p
            className={
              hint.warning ? "tool-hint tool-hint-warning" : "tool-hint"
            }
            role="status"
          >
            {hint.text}
          </p>
          {editError && (
            <p className="editor-error" role="alert">
              The edit was not applied: {editError}
            </p>
          )}
          {projectError && (
            <p className="editor-error" role="alert">
              The project operation failed: {projectError}
            </p>
          )}
          {editorError && (
            <p className="editor-error" role="alert">
              The canvas could not be initialized: {editorError}
            </p>
          )}
        </section>

        <aside className="side">
          <Inspector
            state={state}
            schematic={document.schematic}
            labels={labels}
            disabled={projectOperationLocked}
            onCommit={(componentId, parameterId, value) => {
              if (projectOperationLocked) return;
              if (!state.catalog) return;
              try {
                documentStore.setParameter(
                  componentId,
                  parameterId,
                  value,
                  state.catalog,
                );
                setEditError(null);
              } catch (error) {
                setEditError(
                  error instanceof Error ? error.message : String(error),
                );
                return;
              }
              const schematic = documentStore.getState().schematic;
              // An initial condition only sets the state at time zero, so
              // setting one recompiles instead of sending a lightweight update.
              void controller?.syncDocument(
                schematic,
                liveParameterUpdate(
                  state.catalog,
                  schematic,
                  componentId,
                  parameterId,
                  value,
                ),
              );
            }}
            onClear={(componentId, parameterId) => {
              if (projectOperationLocked) return;
              if (!state.catalog) return;
              try {
                documentStore.clearParameter(
                  componentId,
                  parameterId,
                  state.catalog,
                );
                setEditError(null);
              } catch (error) {
                setEditError(
                  error instanceof Error ? error.message : String(error),
                );
                return;
              }
              // Clearing an optional parameter always recompiles.
              void controller?.syncDocument(documentStore.getState().schematic);
            }}
          />
          <ResultPanel
            state={state}
            labels={labels}
            outputComponentId={DIVIDER_OUTPUT_COMPONENT_ID}
          />
        </aside>

        <Oscilloscope
          simulation={state}
          scope={scope}
          labels={labels}
          onToggleSignal={(key) => scopeController?.toggleSignal(key)}
          onWindowChange={(seconds) =>
            scopeController?.setWindowSeconds(seconds)
          }
        />
      </div>

      <footer className="app-footer">
        {usingMockAdapter ? (
          <span role="status">
            Development mock: predetermined fixture responses, not the Rust
            engine.
          </span>
        ) : (
          <span>Values come from the Rust engine only.</span>
        )}
      </footer>
      {pendingAction !== null && (
        <UnsavedChangesDialog
          closing={pendingAction === "close"}
          disabled={projectBusy}
          onChoice={(choice) => void resolvePendingAction(choice)}
        />
      )}
    </main>
  );
}
