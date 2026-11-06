# Git conventions

## Branches

Branches pushed to GitHub follow [Conventional Branch 1.1.0](https://conventionalbranch.org/): `type/description`.

- Types: `feature/` (new behaviour), `fix/` (bug fixes), `hotfix/` (urgent fixes), `release/` (release prep, e.g. `release/v1.2.0`), `chore/` (docs, dependencies, CI, other non-code work).
- Description: lowercase `a-z0-9`, words joined by single hyphens, dots only inside version numbers. An issue it addresses leads: `feature/issue-27-delete-link`.
- `/prototype` captures its throwaway code on `prototype/<name>` (same description rules). Push it so the implementation issue can point at it, but never open a PR from it.
- A worktree session starts on a `claude/*` branch, which stays local: rename it before the first push, e.g. `git branch -m chore/<description>`.

## Commits

Commit messages follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/): `type(scope): description`.

- Types: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `build`, `ci`, `chore`, `revert`.
- Scope names the area touched: `api`, `server`, `cli` (the crates), `tui` (`crates/cli/src/tui.rs`; the rest of the CLI is `cli`), `brand`, `release`. Leave it out when a change spans several.
- The issue a commit works on goes at the end of the description as a reference: `feat(tui): delete the selected Link after confirmation (#27)`.

## Pull requests

`(#27)` in a commit only links the issue. The PR closes it: put `Closes #27` in the PR body, one line per issue the PR resolves.
