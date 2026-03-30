---
name: file-search
description: Find files and search within files by name, content, or pattern
metadata:
  triggers: "find file, search, locate, where is, look for file, grep, ^find.*, search for, which file"
  tags: "filesystem, search, utility"
  allowed_tools: "exec, read"
---

# File Search Skill

Find files and search within content.

## Capabilities

- Find files by name pattern
- Search file contents (grep)
- Filter by file type, size, date
- Recursive directory search
- Regular expression support

## Search Commands

### Find by Name
```bash
find . -name "*.rs"              # exact pattern
find . -iname "*config*"          # case-insensitive
fd "pattern"                      # faster alternative
```

### Search in Contents
```bash
grep -r "pattern" .               # recursive search
rg "pattern"                      # ripgrep (faster)
grep -l "pattern" *.txt           # list matching files
```

### Combined Filters
```bash
find . -name "*.py" -mtime -7     # modified in last 7 days
find . -type f -size +1M          # files larger than 1MB
```

## Examples

- "Find all TypeScript files"
- "Search for TODO comments in the codebase"
- "Where is the config file?"
- "Find files modified today"
- "Look for files containing 'API_KEY'"

## Best Practices

- Use `fd` and `rg` when available (faster than find/grep)
- Exclude node_modules, .git, target directories
- Show relative paths for readability
- Limit results to avoid overwhelming output
