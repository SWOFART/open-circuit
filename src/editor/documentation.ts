import type { ProjectDocumentation } from "../generated/ProjectDocumentation";

/** A project with no designators, wire numbers, terminals, notes or sheets. */
export function emptyDocumentation(): ProjectDocumentation {
  return {
    designators: [],
    wireNumbers: [],
    terminals: [],
    notes: [],
    titleBlock: {
      title: "",
      drawingNumber: "",
      revision: "",
      author: "",
      company: "",
      date: "",
    },
    sheets: [],
  };
}
