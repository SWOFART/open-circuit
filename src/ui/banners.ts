/**
 * The two error banners under the canvas: the one for an edit that the document refused, and
 * the one for a project operation (open, save) that failed. Each shows the last failure of its
 * kind until something that succeeds makes it stale.
 */
export interface Banners {
  readonly edit: string | null;
  readonly project: string | null;
}

export const NO_BANNERS: Banners = { edit: null, project: null };

export type BannerAction =
  /** An edit was applied (or undone, or redone): no earlier refusal describes the document. */
  | { readonly kind: "editSucceeded" }
  | { readonly kind: "editFailed"; readonly reason: string }
  /** A project operation starts: its own earlier failure is not the answer to this attempt. */
  | { readonly kind: "projectStarted" }
  | { readonly kind: "projectFailed"; readonly reason: string };

export function bannerReducer(state: Banners, action: BannerAction): Banners {
  switch (action.kind) {
    case "editSucceeded":
      // A project failure that came before this edit is about a document that was not opened or
      // saved; once the user edits successfully it is no longer current.
      return state.edit === null && state.project === null ? state : NO_BANNERS;
    case "editFailed":
      return state.edit === action.reason
        ? state
        : { ...state, edit: action.reason };
    case "projectStarted":
      return state.project === null ? state : { ...state, project: null };
    case "projectFailed":
      return state.project === action.reason
        ? state
        : { ...state, project: action.reason };
  }
}
