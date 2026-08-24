---
name: review
description: Review a pull request for correctness and tests. Use when reviewing.
metadata:
  grok:
    when-to-use: review a pull request
    paths:
      - src/**
    user-invocable: true
---

Review the change for correctness.
