---
name: gitnexus-debugging
description: "Use when the user is debugging a bug, tracing an error, or asking why something fails. Examples: \"Why is X failing?\", \"Where does this error come from?\", \"Trace this bug\""
---

# Debugging with GitNexus

> **IMPORTANT — How to use GitNexus**: GitNexus is a standalone CLI tool. Run it directly
> via `gitnexus <command>` in the Bash tool. Do **NOT** use `mcpl call gitnexus ...` or
> `npx gitnexus ...` — gitnexus is installed globally and invoked by name.

> **Multi-repo note**: Always pass `--repo <name>` to every command that operates on a
> specific repo to avoid "multiple repositories" errors.

## When to Use

- "Why is this function failing?"
- "Trace where this error comes from"
- "Who calls this method?"
- "This endpoint returns 500"
- Investigating bugs, errors, or unexpected behavior

## Workflow

```
1. gitnexus query "<error or symptom>" --repo <name>      → Find related execution flows
2. gitnexus context "<suspect>" --repo <name>              → See callers/callees/processes
3. gitnexus cypher 'MATCH path...' --repo <name>           → Custom traces if needed
4. Read source files to confirm root cause
```

> If "Index is stale" → run `gitnexus analyze` in terminal.

## Checklist

```
- [ ] Understand the symptom (error message, unexpected behavior)
- [ ] gitnexus query for error text or related code
- [ ] Identify the suspect function from returned processes
- [ ] gitnexus context to see callers and callees
- [ ] gitnexus cypher for custom call chain traces if needed
- [ ] Read source files to confirm root cause
```

## Debugging Patterns

| Symptom              | GitNexus Approach                                              |
| -------------------- | -------------------------------------------------------------- |
| Error message        | `gitnexus query` for error text → `gitnexus context` on throw sites |
| Wrong return value   | `gitnexus context` on the function → trace callees for data flow     |
| Intermittent failure | `gitnexus context` → look for external calls, async deps             |
| Performance issue    | `gitnexus context` → find symbols with many callers (hot paths)      |
| Recent regression    | `gitnexus detect-changes` to see what your changes affect            |

## CLI Commands Reference

All commands are run directly via the Bash tool. Do **not** use `mcpl` or `npx`.

| Command | What it gives you | Example |
| ------- | ----------------- | ------- |
| `gitnexus query "<concept>" --repo <name>` | Execution flows related to a concept | `gitnexus query "payment validation error" --repo <name>` |
| `gitnexus context "<symbol>" --repo <name>` | 360-degree symbol view — callers, callees, processes | `gitnexus context "validatePayment" --repo <name>` |
| `gitnexus cypher "<query>" --repo <name>` | Raw graph queries for custom call chain traces | `gitnexus cypher "MATCH ..." --repo <name>` |
| `gitnexus detect-changes --repo <name>` | What your current changes affect | `gitnexus detect-changes --repo <name>` |

## Example: "Payment endpoint returns 500 intermittently"

```
1. gitnexus query "payment error handling" --repo my-app
   → Processes: CheckoutFlow, ErrorHandling
   → Symbols: validatePayment, handlePaymentError

2. gitnexus context "validatePayment" --repo my-app
   → Outgoing calls: verifyCard, fetchRates (external API!)

3. Root cause: fetchRates calls external API without proper timeout
```
