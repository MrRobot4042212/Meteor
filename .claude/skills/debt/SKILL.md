---
name: debt
description: Close an item in Meteor's known-debt register (CLAUDE.md §12) and keep the ADR in sync. Use when a C*/H*/M*/F*/L* audit item is fixed, or when the user says "cierra la deuda X", "actualiza el registro".
---

# Close a debt item

The register in `CLAUDE.md` §12 is the project's memory of what is still broken.
It is only useful if it is true, so closing an item is part of the fix, not
paperwork afterwards.

## Steps

1. **Verify the fix in source, not from the diff summary.** Open the file and
   confirm the described behaviour is actually gone. If the fix is partial, the
   status says *partly fixed* and states what remains — never a bare "fixed".

2. **Confirm there is a regression test.** `CLAUDE.md` §10: every bug fix ships
   with one. If there is none, add it before closing (Rust: `#[cfg(test)]` next
   to the code; TS: a `*.test.ts` next to the module).

3. **Update the row** in `CLAUDE.md` §12, keeping the ID and severity:

   ```
   | C2 | critical | <where> | **fixed** — <what changed, in one line> |
   ```

   Anything the user must still do themselves (rotate a secret, run the app once
   to verify) goes in the same cell in italics.

4. **CHANGELOG.md** — add the user-visible part under `[No publicado]`, in the
   right subsection (`### Seguridad`, `### Corregido`, `### Cambiado`).

5. **ADR** — if the fix changed a deliberate decision (§4 / the ADR in the
   codebase-memory graph), update it with `manage_adr` and say which decision
   moved.

6. Report the gates you ran (use `/gate`), the commit if there is one, and any
   item that turned out to be *not* fixed after inspection.
