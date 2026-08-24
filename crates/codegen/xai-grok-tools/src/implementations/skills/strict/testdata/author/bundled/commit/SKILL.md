---
name: commit
description: Create well-formatted git commits. Use when the user wants to commit.
metadata:
  grok:
    when-to-use: commit changes
    paths:
      - src/**
    argument-hint: commit message
    user-invocable: true
---

Review staged changes and create a conventional commit.
