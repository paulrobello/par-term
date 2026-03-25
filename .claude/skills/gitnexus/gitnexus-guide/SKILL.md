---
name: gitnexus-guide
description: "Use when the user asks about GitNexus itself — available tools, how to query the knowledge graph, graph schema, or workflow reference. Examples: \"What GitNexus tools are available?\", \"How do I use GitNexus?\""
---

# GitNexus Guide

Quick reference for all GitNexus CLI commands, tools, and the knowledge graph schema.

> **IMPORTANT — How to use GitNexus**: GitNexus is a standalone CLI tool. Run it directly
> via `gitnexus <command>` in the Bash tool (e.g., `gitnexus query "auth flow"`,
> `gitnexus impact "myFunc" --direction upstream`). Do **NOT** use `mcpl call gitnexus ...`
> — gitnexus is not invoked through mcpl.

## Always Start Here

For any task involving code understanding, debugging, impact analysis, or refactoring:

1. **Run `gitnexus status`** — check index freshness
2. **Match your task to a skill below** and **read that skill file**
3. **Follow the skill's workflow and checklist**

> If step 1 warns the index is stale, run `gitnexus analyze` in the terminal first.

## Skills

| Task                                         | Skill to read       |
| -------------------------------------------- | ------------------- |
| Understand architecture / "How does X work?" | `gitnexus-exploring`         |
| Blast radius / "What breaks if I change X?"  | `gitnexus-impact-analysis`   |
| Trace bugs / "Why is X failing?"             | `gitnexus-debugging`         |
| Rename / extract / split / refactor          | `gitnexus-refactoring`       |
| Tools, resources, schema reference           | `gitnexus-guide` (this file) |
| Index, status, clean, wiki CLI commands      | `gitnexus-cli`               |

## CLI Commands Reference

All commands are run directly via the Bash tool. Do **not** use `mcpl`.

| Command | What it gives you | Example |
| ------- | ----------------- | ------- |
| `gitnexus query "<concept>" --repo <name>` | Process-grouped code intelligence — execution flows related to a concept | `gitnexus query "auth flow" --repo par-term` |
| `gitnexus context "<symbol>" --repo <name>` | 360-degree symbol view — categorized refs, processes it participates in | `gitnexus context "validateUser" --repo par-term` |
| `gitnexus impact "<symbol>" --direction upstream --repo <name>` | Symbol blast radius — what breaks at depth 1/2/3 with confidence | `gitnexus impact "myFunc" --direction upstream --repo par-term` |
| `gitnexus detect-changes --repo <name>` | Git-diff impact — what do your current changes affect | `gitnexus detect-changes --repo par-term` |
| `gitnexus rename "<old>" "<new>" --repo <name>` | Multi-file coordinated rename with confidence-tagged edits | `gitnexus rename "myFunc" "myNewFunc" --repo par-term` |
| `gitnexus cypher "<query>" --repo <name>` | Raw graph queries | `gitnexus cypher "MATCH ..." --repo par-term` |
| `gitnexus status` | Index freshness check | `gitnexus status` |
| `gitnexus analyze` | Build or refresh the index | `gitnexus analyze` |
| `gitnexus list` | Discover indexed repos | `gitnexus list` |

> **Multi-repo note**: This workspace has multiple repos indexed. Always pass
> `--repo <name>` to every command to avoid "multiple repositories" errors.

## Graph Schema

**Nodes:** File, Function, Class, Interface, Method, Community, Process
**Edges (via CodeRelation.type):** CALLS, IMPORTS, EXTENDS, IMPLEMENTS, DEFINES, MEMBER_OF, STEP_IN_PROCESS

```cypher
MATCH (caller)-[:CodeRelation {type: 'CALLS'}]->(f:Function {name: "myFunc"})
RETURN caller.name, caller.filePath
```
