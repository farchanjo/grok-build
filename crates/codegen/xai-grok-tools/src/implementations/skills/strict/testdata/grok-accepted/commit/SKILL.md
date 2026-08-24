---
name: commit
description: Create well-formatted git commits. Use when the user wants to commit.
metadata:
  author: xai
  grok:
    when-to-use: commit changes
    paths:
      - "**/*.rs"
    argument-hint: commit message
    user-invocable: true
    disable-model-invocation: false
---
Review staged changes and create a conventional commit.
