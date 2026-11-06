# Domain docs

Single-context repo: one `GLOSSARY.md` at the root, decisions in `docs/adr/`.

## Before changing an area

Read the ADRs in `docs/adr/` that touch it. If your change contradicts one, say so explicitly rather than silently overriding it:

> _Contradicts ADR-0002 (codes are never reused), but worth reopening because…_

## A concept the glossary lacks

Either you're inventing language the project doesn't use (pick an existing term), or there's a real gap: note it for `/domain-modeling`, which adds terms and ADRs as they get resolved.
