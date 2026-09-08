---
name: i18n-check
description: Verify es/en catalog parity and find user-facing strings that bypass i18n. Use after adding UI text, before a release, or when the user says "i18n", "traducciones", "faltan textos".
---

# i18n check

Two failure modes, neither of which the typecheck catches: a key that exists in
one catalog only (renders the raw key id in the UI), and a literal that never
went through `t()` at all.

## 1. Catalog parity

```bash
npm run test -- catalogs
```

`src/i18n/catalogs.test.ts` asserts: identical key sets both ways, matching
plural variants (`_one`/`_other`), no empty strings, and identical `{{interpolation}}`
placeholders per key. Report the failing key paths verbatim.

## 2. Hardcoded strings

Look for user-visible text that is not routed through `t()`:

```bash
# JSX text nodes with letters and no {t(...)} nearby
grep -rnE '>[A-Za-zÁÉÍÓÚÑáéíóúñ][A-Za-zÁÉÍÓÚÑáéíóúñ ,.:;!?-]{3,}<' src/components src/app --include=*.tsx | grep -v '{t('

# Attributes that reach the user
grep -rnE '(title|aria-label|placeholder|alt)="[^"]{4,}"' src --include=*.tsx | grep -v '{t('
```

Ignore: `data-*`, class names, icon `viewBox`, brand names (Steam, Epic, Discord)
and the `SOURCE_META` labels, which are proper nouns.

## 3. New keys on this branch

```bash
git diff master --unified=0 -- src/i18n/es.ts src/i18n/en.ts | grep -E '^\+\s+\w+:' | sort
```

Every added key must appear in both files with the same path.

## Report

- Missing keys, per language, as dotted paths.
- Hardcoded strings as `file:line — "text"`, with the key you would add.
- If everything passes, say so with the test output, not just "OK".
