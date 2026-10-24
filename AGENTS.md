# Agent instructions

These instructions apply to work in this repository. Follow system and user instructions first. If repository instructions conflict, identify the conflict and ask the user before proceeding with the affected work.

## Before implementation

- Inspect the current branch, working-tree status, task acceptance criteria, relevant code, and existing changes.
- Preserve unrelated user changes and keep them out of commits.
- Keep each change focused on the requested task.
- Use current primary documentation when an integration or dependency detail may have changed.

## Architecture

- Read the repository-root `ARCHITECTURE.md` before planning or changing code. Treat its documented decisions, module boundaries, data contracts, physics assumptions, and deferred work as constraints for implementation.
- Check the proposed change against `ARCHITECTURE.md` and keep code, tests, and documentation consistent with it.
- If the task conflicts with the architecture or requires a new architectural decision, identify the conflict and ask the user before implementing the divergent approach.
- When an approved task changes an architectural decision or public contract, update `ARCHITECTURE.md` or add an ADR before or in the same change as the implementation.

## Implementation

- Make the smallest coherent change that satisfies the acceptance criteria.
- Validate untrusted input at system boundaries and preserve existing security and data-integrity guarantees.
- Do not weaken compiler, lint, type-check, test, security, or CI settings to get a passing result.
- Add or update tests when behavior changes or a defect is fixed.
- Update user-facing or developer documentation when behavior, interfaces, or setup instructions change.
- Do not leave unrelated cleanup, dead code, unexplained suppressions, placeholder credentials, or hidden follow-up work.

## Secrets and privacy

- Never log, display, persist, commit, or transmit passwords, private keys, tokens, API secrets, credentials, personal data, or sensitive runtime configuration.
- Keep secrets in approved secret stores or ignored local files. Commit only safe examples with placeholder values.
- Review staged and new files for secrets before committing or sharing a diff.
- Do not provide secrets or private data to reviewers. If safe review is impossible, stop and explain the blocker.

## Git and pull requests

- Do not implement directly on protected branches. Use a short-lived branch focused on one issue or tightly related change.
- Do not force-push, rewrite shared history, or push directly to protected branches without explicit user authorization.
- Keep commits scoped to intended files and changes. Do not include unrelated user work.
- Always create pull requests as drafts. Do not mark them ready for review unless the user explicitly asks.
- Agents do not merge pull requests. A human reviews and performs the merge.
- Run applicable local checks and report the commands and results. If a required check cannot run or fails, report that clearly; do not bypass it.

## Independent Gate A review

- Before asking for Gate A review, finish the implementation, run applicable local checks, inspect the complete intended diff, and identify the exact candidate being reviewed.
- Do not launch or perform the independent Gate A review yourself.
- Provide the user with the reusable prompt in `.github/prompts/gate-a-review.md`, filled in with the task, acceptance criteria, base revision, candidate revision or tree, and any safe validation evidence that applies.
- Then stop and wait for the user to send the independent review result. Do not commit, push, open or update a pull request, or continue to a later review stage while that result is pending.
- If the user returns a failed or incomplete result, address the findings as requested. Any change to the reviewed candidate requires a new Gate A review prompt and a fresh user-provided result.
- Treat only an explicit passing verdict for the exact candidate as a pass. Missing, ambiguous, incomplete, stale, or failed results do not pass.

## Issues

- When you find a distinct actionable defect or problem outside the current task, create a GitHub issue using the repository's issue template. Include evidence, impact, reproduction steps when available, and clear acceptance criteria. Avoid duplicate issues by checking existing issues first.
- When you begin work on an existing issue, assign that issue to yourself before changing files. If you cannot assign yourself with the available GitHub access, stop before implementation and ask the user to assign it or provide access.
- Keep the issue updated with a concise summary of progress, checks, and any remaining blocker.
