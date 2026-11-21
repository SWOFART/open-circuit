/**
 * The designator rules of docs/contracts/stage-12-cabinet-documentation.md sections 3.3 and
 * 3.4, mirrored for an immediate check where the user types. The Engine's validation is the
 * authority; this only keeps a refusal close to the field.
 */

/** One to three capital letters, the first an identification letter, then 1 to 9999. */
export const DESIGNATOR_PATTERN =
  /^[ABCEFGHKMPQRSTUVWX][A-HJ-NP-Z]{0,2}[1-9][0-9]{0,3}$/;

export type DesignatorProblem = "empty" | "form" | "duplicate";

/** Why `text` cannot be a designator next to `others` (the other parts' designators). */
export function designatorProblem(
  text: string,
  others: Iterable<string>,
): DesignatorProblem | null {
  if (text === "") return "empty";
  if (!DESIGNATOR_PATTERN.test(text)) return "form";
  for (const other of others) {
    if (other === text) return "duplicate";
  }
  return null;
}

export const DESIGNATOR_MESSAGES: Readonly<Record<DesignatorProblem, string>> =
  {
    empty: "A designator cannot be empty.",
    form: "A designator is one to three capital letters, then a number from 1 to 9999, such as K1 or QF12. Use a letter from the identification table; I and O are not used.",
    duplicate: "Another part already has this designator.",
  };

const PARTS = /^([A-Z]+)([0-9]+)$/;
const LAST_NUMBER = 9999;

/**
 * The next designator for a part whose default letters are `prefix`: the highest number in use
 * with exactly these letters plus one (1 when none is). Gaps are left alone; a number freed by
 * a deletion may be taken again.
 */
export function nextDesignator(
  prefix: string,
  taken: Iterable<string>,
): string {
  let highest = 0;
  for (const name of taken) {
    const match = PARTS.exec(name);
    if (match?.[1] === prefix) {
      highest = Math.max(highest, Number(match[2]));
    }
  }
  if (highest >= LAST_NUMBER) {
    throw new Error(`There is no free ${prefix} designator left.`);
  }
  return `${prefix}${highest + 1}`;
}
