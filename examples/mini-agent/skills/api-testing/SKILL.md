---
name: api-testing
description: Test HTTP APIs, make curl requests, and debug endpoints
metadata:
  triggers: "test API, curl, endpoint, HTTP request, REST, POST, GET, API call, ^test.*endpoint, request to"
  tags: "api, testing, http"
  allowed_tools: "exec, web_fetch"
---

# API Testing Skill

Test and debug HTTP APIs.

## Capabilities

- Make HTTP requests (GET, POST, PUT, DELETE)
- Set headers and authentication
- Send JSON payloads
- Parse and format responses
- Test API endpoints
- Debug request/response issues

## Request Examples

### GET Request
```bash
curl -X GET "https://api.example.com/users" \
  -H "Authorization: Bearer $TOKEN"
```

### POST with JSON
```bash
curl -X POST "https://api.example.com/users" \
  -H "Content-Type: application/json" \
  -d '{"name": "John", "email": "john@example.com"}'
```

### With Verbose Output
```bash
curl -v "https://api.example.com/health"
```

## Testing Checklist

- [ ] Correct HTTP method
- [ ] Valid endpoint URL
- [ ] Required headers present
- [ ] Request body formatted correctly
- [ ] Authentication token valid
- [ ] Expected status code
- [ ] Response body validation

## Examples

- "Test the health endpoint"
- "Make a POST request to create a user"
- "Debug why this API call is failing"
- "Check the response headers from this endpoint"

## Tips

- Use `jq` to format JSON responses
- Save common requests as shell scripts
- Check rate limits before bulk testing
