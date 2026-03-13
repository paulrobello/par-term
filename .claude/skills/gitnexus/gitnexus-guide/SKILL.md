---
name: gitnexus-guide
description: "Use when the user asks about GitNexus itself — available tools, how to query the knowledge graph, MCP resources, graph schema, or workflow reference. Examples: \"What GitNexus tools are available?\", \"How do I use GitNexus?\""
---

# GitNexus Guide

Quick reference for all GitNexus CLI commands and the knowledge graph schema.

> **IMPORTANT — How to use GitNexus**: GitNexus is a standalone CLI tool. Run it directly via `gitnexus <command>` in the Bash tool (e.g., `gitnexus query "auth flow"`, `gitnexus impact "myFunc" --direction upstream`). Do **NOT** use `mcpl call gitnexus ...` — gitnexus is not invoked through mcpl.

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
| `gitnexus query "<concept>"` | Execution flows related to a concept | `gitnexus query "auth flow"` |
| `gitnexus context "<symbol>"` | 360-degree symbol view — callers, callees, processes | `gitnexus context "validateUser"` |
| `gitnexus impact "<symbol>" --direction upstream` | Blast radius — what breaks at depth 1/2/3 | `gitnexus impact "myFunc" --direction upstream` |
| `gitnexus cypher "<query>"` | Raw Cypher graph queries | `gitnexus cypher "MATCH ..."` |
| `gitnexus status` | Index freshness check | `gitnexus status` |
| `gitnexus analyze` | Build or refresh the index | `gitnexus analyze` |
| `gitnexus list` | List all indexed repos | `gitnexus list` |

## Graph Schema

**Nodes:** File, Function, Class, Interface, Method, Community, Process
**Edges (via CodeRelation.type):** CALLS, IMPORTS, EXTENDS, IMPLEMENTS, DEFINES, MEMBER_OF, STEP_IN_PROCESS

```cypher
MATCH (caller)-[:CodeRelation {type: 'CALLS'}]->(f:Function {name: "myFunc"})
RETURN caller.name, caller.filePath
```
