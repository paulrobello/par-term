# Implementation Plans

This directory contains design documents and implementation plans created during feature development.

> **Warning:** Line numbers in plan files reference the codebase state at the time the plan was written. Code evolves after plans are created, so any line number references in these files are likely stale. Use file paths and function/struct names to locate the relevant code — do not rely on line numbers.

## Purpose

Each plan file documents:

- The problem being solved and the chosen approach
- File-level task breakdown for the implementing engineer
- Code snippets and pseudocode for key changes

Plans are historical records of design decisions. After implementation, the plan may not reflect the final code exactly.

## Naming Convention

Files follow the pattern `YYYY-MM-DD-<feature>-<type>.md` where type is one of:

- `design` — architecture and approach discussion
- `plan` — task-by-task implementation steps
- `impl` — implementation notes written during or after coding

## Related Documentation

- [Architecture Overview](../ARCHITECTURE.md) — current system architecture
- [Crate Structure](../CRATE_STRUCTURE.md) — workspace organization
