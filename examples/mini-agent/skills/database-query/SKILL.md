---
name: database-query
description: Write and execute SQL queries, analyze database schemas, and manage data
metadata:
  triggers: "SQL, query, database, SELECT, INSERT, UPDATE, DELETE, table, schema, postgres, mysql, sqlite, ^run.*query"
  tags: "database, sql, data"
  allowed_tools: "exec"
---

# Database Query Skill

Write, explain, and execute SQL queries.

## Capabilities

- Write SQL queries for common operations
- Explain complex query logic
- Optimize slow queries
- Design database schemas
- Migrate data between formats

## Supported Databases

- PostgreSQL
- MySQL / MariaDB
- SQLite
- SQL Server (basic)

## Query Writing Guidelines

1. Always use parameterized queries to prevent SQL injection
2. Include appropriate indexes in schema suggestions
3. Use transactions for multi-statement operations
4. Limit result sets to avoid memory issues
5. Explain query plans when optimizing

## Examples

- "Write a query to find users who signed up last week"
- "Explain what this SQL does"
- "Create a table schema for a blog"
- "Optimize this slow query"

## Safety

- Never execute DROP or TRUNCATE without explicit confirmation
- Always show query before execution
- Suggest LIMIT clauses for large tables
