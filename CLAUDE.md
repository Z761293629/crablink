## Agent skills

### Issue tracker

Issues live in GitHub Issues of `Z761293629/crablink` (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`. See `docs/agents/domain.md`.

## Branches

Branches pushed to GitHub follow [Conventional Branch 1.1.0](https://conventionalbranch.org/): `type/description`.

- Types: `feature/` (new behaviour), `fix/` (bug fixes), `hotfix/` (urgent fixes), `release/` (release prep, e.g. `release/v1.2.0`), `chore/` (docs, dependencies, CI, other non-code work).
- Description: lowercase `a-z0-9`, words joined by single hyphens, dots only inside version numbers. An issue it addresses leads: `feature/issue-27-delete-link`.
- A worktree session starts on a `claude/*` branch, which stays local: rename it before the first push, e.g. `git branch -m chore/<description>`.

## Commits

Commit messages follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/): `type(scope): description`.

- Types: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `build`, `ci`, `chore`, `revert`.
- Scope names the area touched: `api`, `cli`, `server` (the crates), `tui`, `brand`, `release`. Leave it out when a change spans several.
- An issue the commit closes goes at the end of the description: `feat(tui): delete the selected Link after confirmation (#27)`.
