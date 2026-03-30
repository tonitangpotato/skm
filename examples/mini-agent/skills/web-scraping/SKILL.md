---
name: web-scraping
description: Fetch web pages, extract content, and scrape data from URLs
metadata:
  triggers: "scrape, fetch page, crawl, extract from url, http://, https://, .com, .org, .io, webpage, website content"
  tags: "web, http, extraction"
  allowed_tools: "web_fetch, browser"
---

# Web Scraping Skill

Extract content from web pages and URLs.

## Capabilities

- Fetch and render web pages
- Extract text, tables, and structured data
- Handle JavaScript-rendered content
- Follow pagination and links
- Respect robots.txt and rate limits

## Usage

When a user provides a URL or asks to fetch content from a website:

1. Validate the URL format
2. Check robots.txt if doing bulk scraping
3. Fetch the page content
4. Extract the requested information
5. Return structured results

## Examples

- "Scrape the product prices from this page"
- "Fetch the article content from https://example.com/post"
- "Extract all links from this webpage"
- "Get the main text from this URL"

## Rate Limiting

- Wait 1 second between requests to the same domain
- Maximum 100 requests per session
- Respect Retry-After headers
