import { invoke, isTauri } from "@tauri-apps/api/core";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { SetParameterRequest } from "../generated/SetParameterRequest";
import type { SetParameterResult } from "../generated/SetParameterResult";

/**
 * The UI's only view of the Rust Engine. Requests and results are the
 * generated contract types; the UI never calculates physics itself.
 */
export interface EngineAdapter {
  getCatalog(): Promise<ComponentCatalog>;
  compile(request: CompileRequest): Promise<CompileResult>;
  setParameter(request: SetParameterRequest): Promise<SetParameterResult>;
}

/** The backend cannot be reached, as opposed to rejecting the circuit. */
export class EngineUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "EngineUnavailableError";
  }
}

/**
 * Production adapter. Only the catalog command exists in the Tauri bridge so
 * far; compile and parameter updates arrive with the simulation-thread bridge
 * (issue #8). Until then they fail clearly rather than returning fake numbers.
 */
export class TauriEngineAdapter implements EngineAdapter {
  async getCatalog(): Promise<ComponentCatalog> {
    if (!isTauri()) {
      throw new EngineUnavailableError(
        "The Rust engine is only available in the desktop app.",
      );
    }

    return invoke<ComponentCatalog>("get_component_catalog");
  }

  compile(): Promise<CompileResult> {
    return Promise.reject(
      new EngineUnavailableError(
        "The simulation backend is not connected yet.",
      ),
    );
  }

  setParameter(): Promise<SetParameterResult> {
    return Promise.reject(
      new EngineUnavailableError(
        "The simulation backend is not connected yet.",
      ),
    );
  }
}

/**
 * Picks the adapter. The fixture-backed mock is reachable only in development
 * builds and only when explicitly requested, so a production bundle can never
 * show mock readings to a user.
 */
export async function createEngineAdapter(): Promise<EngineAdapter> {
  if (import.meta.env.DEV && import.meta.env.VITE_ENGINE_ADAPTER === "mock") {
    const { createDividerFixtureAdapter } = await import("./mockEngineAdapter");
    return createDividerFixtureAdapter();
  }

  return new TauriEngineAdapter();
}
