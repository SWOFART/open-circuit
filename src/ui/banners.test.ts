import { describe, expect, it } from "vitest";
import { bannerReducer, NO_BANNERS, type Banners } from "./banners";

const failedOpen: Banners = {
  edit: null,
  project: "Project schematic failed validation: DuplicateRailName.",
};

describe("the error banners", () => {
  it("show the last failure of each kind", () => {
    const refused = bannerReducer(NO_BANNERS, {
      kind: "editFailed",
      reason: "Another rail is already named L+.",
    });
    const both = bannerReducer(refused, {
      kind: "projectFailed",
      reason: "Could not read project file.",
    });

    expect(both).toEqual({
      edit: "Another rail is already named L+.",
      project: "Could not read project file.",
    });
  });

  it("drops a stale project failure when a later edit succeeds", () => {
    expect(bannerReducer(failedOpen, { kind: "editSucceeded" })).toEqual(
      NO_BANNERS,
    );
  });

  it("drops an earlier refused edit when a later edit succeeds", () => {
    const refused: Banners = { edit: "refused", project: null };

    expect(bannerReducer(refused, { kind: "editSucceeded" })).toEqual(
      NO_BANNERS,
    );
  });

  it("keeps the project failure while an edit is refused", () => {
    expect(
      bannerReducer(failedOpen, { kind: "editFailed", reason: "refused" }),
    ).toEqual({ edit: "refused", project: failedOpen.project });
  });

  it("clears the project failure when a project operation starts, and only that one", () => {
    const both: Banners = { edit: "refused", project: "failed" };

    expect(bannerReducer(both, { kind: "projectStarted" })).toEqual({
      edit: "refused",
      project: null,
    });
  });

  it("replaces a project failure with a newer one", () => {
    expect(
      bannerReducer(failedOpen, { kind: "projectFailed", reason: "again" }),
    ).toEqual({ edit: null, project: "again" });
  });

  it("returns the same object when nothing changes, so nothing re-renders", () => {
    expect(bannerReducer(NO_BANNERS, { kind: "editSucceeded" })).toBe(
      NO_BANNERS,
    );
    expect(bannerReducer(NO_BANNERS, { kind: "projectStarted" })).toBe(
      NO_BANNERS,
    );
    expect(
      bannerReducer(failedOpen, {
        kind: "projectFailed",
        reason: failedOpen.project!,
      }),
    ).toBe(failedOpen);
  });
});
