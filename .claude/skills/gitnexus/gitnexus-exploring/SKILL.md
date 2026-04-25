---
name: gitnexus-exploring
description: "Use when the user asks how code works, wants to understand architecture, trace execution flows, or explore unfamiliar parts of the codebase. Examples: \"How does X work?\", \"What calls this function?\", \"Show me the auth flow\""
---

# Exploring Codebases with GitNexus

> **IMPORTANT — How to use GitNexus**: GitNexus is a standalone CLI tool. Run it directly
> via `gitnexus <command>` in the Bash tool. Do **NOT** use `mcpl call gitnexus ...` or
> `npx gitnexus ...` — gitnexus is installed globally and invoked by name.

> **Multi-repo note**: Always pass `--repo <name>` to every command that operates on a
> specific repo to avoid "multiple repositories" errors.

## When to Use

- "How does authentication work?"
- "What's the project structure?"
- "Show me the main components"
- "Where is the database logic?"
- Understanding code you haven't seen before

## Workflow

```
1. gitnexus status                                     → Check index freshness
2. gitnexus query "<what you want>" --repo <name>      → Find related execution flows
3. gitnexus context "<symbol>" --repo <name>           → Deep dive on specific symbol
4. Read source files from the output for implementation details
```

> If step 1 says "Index is stale" → run `gitnexus analyze` in terminal.

## Checklist

```
- [ ] gitnexus status — check index freshness
- [ ] gitnexus query for the concept you want to understand
- [ ] Review returned processes (execution flows)
- [ ] gitnexus context on key symbols for callers/callees
- [ ] Read source files for implementation details
```

## CLI Commands Reference

All commands are run directly via the Bash tool. Do **not** use `mcpl` or `npx`.

| Command | What it gives you | Example |
| ------- | ----------------- | ------- |
| `gitnexus query "<concept>" --repo <name>` | Process-grouped execution flows related to a concept | `gitnexus query "payment processing" --repo <name>` |
| `gitnexus context "<symbol>" --repo <name>` | 360-degree symbol view — callers, callees, processes | `gitnexus context "validateUser" --repo <name>` |
| `gitnexus status` | Index freshness check | `gitnexus status` |

## Example: "How does payment processing work?"

```
1. gitnexus status                                    → 918 symbols, 45 processes
2. gitnexus query "payment processing" --repo my-app
   → CheckoutFlow: processPayment → validateCard → chargeStripe
   → RefundFlow: initiateRefund → calculateRefund → processRefund
3. gitnexus context "processPayment" --repo my-app
   → Incoming: checkoutHandler, webhookHandler
   → Outgoing: validateCard, chargeStripe, saveTransaction
4. Read src/payments/processor.ts for implementation details
```
