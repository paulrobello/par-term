---
description: Bump version, update docs, sanity check and deploy project
---
ensure project is using latest published version of core library
run 'make pre-commit' fix all issues
bump version update changelog and readme
use docs/DOCUMENTATION_STYLE_GUIDE.md to update all docs/ and or create new docs for all the changes since last release
commit and push all changes
run 'make deploy' to trigger cicd deployment
