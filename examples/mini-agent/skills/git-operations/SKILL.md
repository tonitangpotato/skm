---
name: git-operations
description: Manage git repositories, branches, commits, and version control operations
metadata:
  triggers: "git, commit, branch, merge, push, pull, checkout, rebase, stash, ^git.*, version control, repo"
  tags: "git, vcs, development"
  allowed_tools: "exec"
---

# Git Operations Skill

Manage version control with Git.

## Capabilities

- Create and manage branches
- Stage and commit changes
- Push and pull from remotes
- Resolve merge conflicts
- Interactive rebase
- Stash and restore work

## Common Workflows

### Feature Branch
```bash
git checkout -b feature/name
# make changes
git add .
git commit -m "feat: description"
git push -u origin feature/name
```

### Sync with Main
```bash
git fetch origin
git rebase origin/main
# or
git merge origin/main
```

### Undo Last Commit
```bash
git reset --soft HEAD~1  # keep changes staged
git reset --hard HEAD~1  # discard changes
```

## Commit Message Format

Follow conventional commits:
- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation
- `refactor:` code refactoring
- `test:` adding tests
- `chore:` maintenance

## Examples

- "Create a new branch for this feature"
- "Commit my changes with a good message"
- "Show the git log"
- "Help me resolve this merge conflict"
