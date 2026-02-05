#!/bin/bash

# Read JSON input from stdin
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

# Only trigger for .rs files
if [[ ! "$FILE_PATH" =~ \.rs$ ]]; then
  exit 0
fi

# Run cargo check with short output
cargo check --message-format=short 2>&1 | head -20
