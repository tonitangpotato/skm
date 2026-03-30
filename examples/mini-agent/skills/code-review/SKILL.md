---
name: code-review
description: Review code changes, provide feedback on PRs, and suggest improvements
metadata:
  triggers: "review, PR, pull request, code review, check this code, review my changes, look at this diff, ^review.*code"
  tags: "development, code-quality"
  allowed_tools: "read, exec"
---

# Code Review Skill

Provide thoughtful code reviews with actionable feedback.

## Capabilities

- Review pull requests and diffs
- Identify bugs, security issues, and performance problems
- Suggest improvements and best practices
- Check code style and consistency
- Explain complex code sections

## Review Checklist

When reviewing code, consider:

1. **Correctness**: Does it do what it's supposed to?
2. **Security**: Any vulnerabilities or unsafe patterns?
3. **Performance**: Obvious inefficiencies?
4. **Readability**: Clear naming, good structure?
5. **Testing**: Adequate test coverage?
6. **Edge cases**: Handled properly?

## Output Format

Structure feedback as:

```
## Summary
Brief overall assessment

## Critical Issues
- Issue 1: explanation + suggestion

## Suggestions
- Improvement 1: explanation

## Nitpicks (optional)
- Style preference 1
```

## Examples

- "Review this PR for security issues"
- "Check my Python code for best practices"
- "Look at this diff and suggest improvements"
