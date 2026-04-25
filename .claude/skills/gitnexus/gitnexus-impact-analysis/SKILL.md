---
name: gitnexus-impact-analysis
description: "Use when the user wants to know what will break if they change something, or needs safety analysis before editing code. Examples: \"Is it safe to change X?\", \"What depends on this?\", \"What will break?\""
---

# Impact Analysis with GitNexus

> **IMPORTANT — How to use GitNexus**: GitNexus is a standalone CLI tool. Run it directly
> via `gitnexus <command>` in the Bash tool. Do **NOT** use `mcpl call gitnexus ...` or
> `npx gitnexus ...` — gitnexus is installed globally and invoked by name.

> **Multi-repo note**: Always pass `--repo <name>` to every command that operates on a
> specific repo to avoid "multiple repositories" errors.

## When to Use

- "Is it safe to change this function?"
- "What will break if I modify X?"
- "Show me the blast radius"
- "Who uses this code?"
- Before making non-trivial code changes
- Before committing — to understand what your changes affect

## Workflow

```
1. gitnexus impact "X" --direction upstream --repo <name>  → What depends on this
2. gitnexus detect-changes --repo <name>                   → Map current git changes to affected flows
3. Assess risk and report to user
```

> If "Index is stale" → run `gitnexus analyze` in terminal.

## Checklist

```
- [ ] gitnexus impact "<symbol>" --direction upstream --repo <name> to find dependents
- [ ] Review d=1 items first (these WILL BREAK)
- [ ] Check high-confidence (>0.8) dependencies
- [ ] gitnexus detect-changes --repo <name> for pre-commit check
- [ ] Assess risk level and report to user
```

## Understanding Output

| Depth | Risk Level       | Meaning                  |
| ----- | ---------------- | ------------------------ |
| d=1   | **WILL BREAK**   | Direct callers/importers |
| d=2   | LIKELY AFFECTED  | Indirect dependencies    |
| d=3   | MAY NEED TESTING | Transitive effects       |

## Risk Assessment

| Affected                       | Risk     |
| ------------------------------ | -------- |
| <5 symbols, few processes      | LOW      |
| 5-15 symbols, 2-5 processes    | MEDIUM   |
| >15 symbols or many processes  | HIGH     |
| Critical path (auth, payments) | CRITICAL |

## CLI Commands Reference

All commands are run directly via the Bash tool. Do **not** use `mcpl` or `npx`.

| Command | What it gives you | Example |
| ------- | ----------------- | ------- |
| `gitnexus impact "<symbol>" --direction upstream --repo <name>` | Symbol blast radius — what breaks at depth 1/2/3 with confidence | `gitnexus impact "validateUser" --direction upstream --repo <name>` |
| `gitnexus detect-changes --repo <name>` | Git-diff impact — what your changes affect | `gitnexus detect-changes --repo <name>` |
| `gitnexus status` | Index freshness check | `gitnexus status` |

## Example: "What breaks if I change validateUser?"

```
1. gitnexus impact "validateUser" --direction upstream --repo my-app
   → d=1: loginHandler, apiMiddleware (WILL BREAK)
   → d=2: authRouter, sessionManager (LIKELY AFFECTED)

2. Risk: 2 direct callers, 2 processes = MEDIUM
```
