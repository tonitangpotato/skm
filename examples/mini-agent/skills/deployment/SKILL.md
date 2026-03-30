---
name: deployment
description: Deploy applications, manage releases, and handle production deployments
metadata:
  triggers: "deploy, ship, release, push to production, go live, rollout, ^deploy.*, publish, launch"
  tags: "devops, ci-cd, infrastructure"
  allowed_tools: "exec, read, write"
---

# Deployment Skill

Handle application deployments and releases.

## Capabilities

- Deploy to various platforms (Vercel, AWS, GCP, Heroku)
- Manage environment variables and secrets
- Handle database migrations
- Rollback failed deployments
- Monitor deployment status

## Deployment Checklist

Before deploying:

1. [ ] All tests passing
2. [ ] No uncommitted changes
3. [ ] Environment variables set
4. [ ] Database migrations prepared
5. [ ] Rollback plan ready

## Platform Commands

### Vercel
```bash
vercel --prod
```

### Docker
```bash
docker build -t app:latest .
docker push registry/app:latest
```

### Kubernetes
```bash
kubectl apply -f k8s/
kubectl rollout status deployment/app
```

## Safety

- Always confirm production deployments
- Show diff of what will change
- Check for breaking changes
- Verify environment before executing

## Examples

- "Deploy to production"
- "Ship the latest changes to staging"
- "Rollback to the previous version"
- "Check deployment status"
