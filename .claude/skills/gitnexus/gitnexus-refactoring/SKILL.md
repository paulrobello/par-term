---
name: gitnexus-refactoring
description: "Use when the user wants to rename, extract, split, move, or restructure code safely. Examples: \"Rename this function\", \"Extract this into a module\", \"Refactor this class\", \"Move this to a separate file\""
---

# Refactoring with GitNexus

> **IMPORTANT — How to use GitNexus**: GitNexus is a standalone CLI tool. Run it directly
> via `gitnexus <command>` in the Bash tool. Do **NOT** use `mcpl call gitnexus ...` or
> `npx gitnexus ...` — gitnexus is installed globally and invoked by name.

> **Multi-repo note**: Always pass `--repo <name>` to every command that operates on a
> specific repo to avoid "multiple repositories" errors.

## When to Use

- "Rename this function safely"
- "Extract this into a module"
- "Split this service"
- "Move this to a new file"
- Any task involving renaming, extracting, splitting, or restructuring code

## Workflow

```
1. gitnexus impact "X" --direction upstream --repo <name>  → Map all dependents
2. gitnexus query "X" --repo <name>                        → Find execution flows involving X
3. gitnexus context "X" --repo <name>                      → See all incoming/outgoing refs
4. Plan update order: interfaces → implementations → callers → tests
```

> If "Index is stale" → run `gitnexus analyze` in terminal.

## Checklists

### Rename Symbol

```
- [ ] gitnexus rename "oldName" "newName" --repo <name> --dry-run — preview all edits
- [ ] Review graph edits (high confidence) and ast_search edits (review carefully)
- [ ] If satisfied: gitnexus rename "oldName" "newName" --repo <name> — apply edits
- [ ] gitnexus detect-changes --repo <name> — verify only expected files changed
- [ ] Run tests for affected processes
```

### Extract Module

```
- [ ] gitnexus context "<target>" --repo <name> — see all incoming/outgoing refs
- [ ] gitnexus impact "<target>" --direction upstream --repo <name> — find all external callers
- [ ] Define new module interface
- [ ] Extract code, update imports
- [ ] gitnexus detect-changes --repo <name> — verify affected scope
- [ ] Run tests for affected processes
```

### Split Function/Service

```
- [ ] gitnexus context "<target>" --repo <name> — understand all callees
- [ ] Group callees by responsibility
- [ ] gitnexus impact "<target>" --direction upstream --repo <name> — map callers to update
- [ ] Create new functions/services
- [ ] Update callers
- [ ] gitnexus detect-changes --repo <name> — verify affected scope
- [ ] Run tests for affected processes
```

## CLI Commands Reference

All commands are run directly via the Bash tool. Do **not** use `mcpl` or `npx`.

| Command | What it gives you | Example |
| ------- | ----------------- | ------- |
| `gitnexus rename "<old>" "<new>" --repo <name>` | Multi-file coordinated rename with confidence-tagged edits | `gitnexus rename "validateUser" "authenticateUser" --repo <name> --dry-run` |
| `gitnexus impact "<symbol>" --direction upstream --repo <name>` | Symbol blast radius — dependents at depth 1/2/3 | `gitnexus impact "validateUser" --direction upstream --repo <name>` |
| `gitnexus detect-changes --repo <name>` | Git-diff impact — what your changes affect | `gitnexus detect-changes --repo <name>` |
| `gitnexus context "<symbol>" --repo <name>` | 360-degree symbol view — callers, callees, processes | `gitnexus context "validateUser" --repo <name>` |
| `gitnexus query "<concept>" --repo <name>` | Execution flows related to a concept | `gitnexus query "user validation" --repo <name>` |
| `gitnexus cypher "<query>" --repo <name>` | Raw graph queries for custom reference queries | `gitnexus cypher "MATCH ..." --repo <name>` |

## Risk Rules

| Risk Factor         | Mitigation                                |
| ------------------- | ----------------------------------------- |
| Many callers (>5)   | Use gitnexus rename for automated updates |
| Cross-area refs     | Use detect-changes after to verify scope  |
| String/dynamic refs | gitnexus query to find them               |
| External/public API | Version and deprecate properly            |

## Example: Rename `validateUser` to `authenticateUser`

```
1. gitnexus rename "validateUser" "authenticateUser" --repo <name> --dry-run
   → 12 edits: 10 graph (safe), 2 ast_search (review)
   → Files: validator.ts, login.ts, middleware.ts, config.json...

2. Review ast_search edits (config.json: dynamic reference!)

3. gitnexus rename "validateUser" "authenticateUser" --repo <name>
   → Applied 12 edits across 8 files

4. gitnexus detect-changes --repo <name>
   → Affected: LoginFlow, TokenRefresh
   → Risk: MEDIUM — run tests for these flows
```
