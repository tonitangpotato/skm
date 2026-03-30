# SKM Positioning & Market Analysis

## What SKM Is

**SKM is a DevOps toolchain for Agent Skills** — testing, selection, enforcement, monitoring, and optimization.

It is NOT just "a router for when you have too many skills." Even an agent with 5 skills benefits from SKM.

## Core Value Proposition

| Capability | Value with 5 skills | Value with 50+ skills |
|-----------|---------------------|----------------------|
| **Trigger testing** | ✅ Compliance requirement (Anthropic recommends 3-5 representative query evaluations) | ✅ Essential |
| **Enforcement hooks** | ✅ LLM not following skill instructions is a universal pain point | ✅ Essential |
| **Token budgeting** | ✅ Progressive disclosure saves money on every API call | ✅ Critical |
| **Usage analytics** | ✅ Know which skills are used/unused, optimize your catalog | ✅ Essential |
| **Semantic selection** | ⚪ Trigger matching suffices | ✅ Trigger matching breaks down |
| **Cascade routing** | ⚪ Single strategy suffices | ✅ Multi-strategy needed |

**4 out of 6 capabilities deliver value at ANY skill count.** Semantic selection and cascade routing are future-proofing.

## Market Context

### How Many Skills Does an Agent Typically Have?

- **Claude.ai**: ~10-20 built-in skills (docx, pdf, pptx, xlsx, frontend-design, etc.)
- **Claude Code / Codex**: User-defined skills, power users have 20-50
- **Enterprise internal agents**: Currently 5-15
- **Academic extreme**: 741 tools tested — model accuracy drops 79-100% (Qin et al., 2024)

Most agents today are in the "trigger matching is enough" range. SKM's semantic selection becomes critical at 15-20+ skills.

### The Growth Thesis

As Agent Skills standard matures, enterprises will encode institutional knowledge as skills:
- Approval workflows
- Compliance checks
- Report templates
- Data processing standards

Each department's processes become skills. A single agent could have 100+ skills. At that point, SKM is not optional — it's infrastructure.

**Timeline estimate: 6-12 months before this is mainstream.**

## Competitive Landscape

### Direct Competitors

| Tool | What it does | Limitation | SKM advantage |
|------|-------------|-----------|---------------|
| **agent-skills** (Govcraft) | Parse SKILL.md | Parse only, no selection/enforcement | Full lifecycle |
| **skillc** | CLI toolkit | CLI only, no library API | Library + CLI + HTTP |
| **Portkey mcp-tool-filter** | Tool selection | TypeScript only, selection only | Rust (perf) + full lifecycle |
| **Rolling your own** | Whatever you build | Maintenance burden, no standard | Standards-native, tested, maintained |

### Adjacent (Not Competing)

| Tool | Relationship to SKM |
|------|-------------------|
| **JFrog Agent Skills Registry** | **Complementary.** JFrog = npm registry (publish, audit, version). SKM = Node.js runtime (select, load, enforce). You pull skills from JFrog, run them with SKM. |
| **skillshub** | Package manager for skills. Complementary — distribution vs runtime. |
| **LangGraph / Strands / CrewAI** | Agent frameworks. SKM plugs INTO them, doesn't replace them. |

### The JFrog Analogy

```
JFrog : SKM = npm registry : Node.js runtime

JFrog answers: "Where do skills come from? Who approved them? What version?"
SKM answers:   "Which skill handles this query? Is it being followed? How well?"
```

## Deployment Architectures

### Architecture 1: Embedded Library (Current Focus)

Each agent embeds SKM as a Rust dependency (or Python/Node package).

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  Agent A    │  │  Agent B    │  │  Agent C    │
│ ┌─────────┐ │  │ ┌─────────┐ │  │ ┌─────────┐ │
│ │   skm   │ │  │ │   skm   │ │  │ │   skm   │ │
│ │ 20 skills│ │  │ │ 15 skills│ │  │ │ 30 skills│ │
│ └─────────┘ │  │ └─────────┘ │  │ └─────────┘ │
└─────────────┘  └─────────────┘  └─────────────┘
```

**Best for**: Individual developers, small teams, single-agent apps.

### Architecture 2: Centralized Skill Service (Future Vision)

SKM runs as a shared service. Multiple agents query it via HTTP API.

```
            ┌──────────────┐
            │  skm serve   │
            │  100 skills  │
            │ shared index │
            │ shared policy│
            └──────┬───────┘
       ┌───────────┼───────────┐
       ▼           ▼           ▼
   Agent A     Agent B     Agent C
   (Support)   (Finance)   (HR)
```

**Best for**: Enterprise with multiple agents sharing skills, policies, and analytics.

**This is already supported** via `skm serve` (HTTP API). No additional development needed — just deployment configuration.

### Architecture 3: Hybrid

Core skills embedded per-agent, shared/enterprise skills via centralized service.

```
   Agent A                    ┌──────────────┐
   ┌──────────────┐          │  skm serve   │
   │ local skm    │◄────────►│ shared skills│
   │ 5 own skills │  HTTP    │ 50 enterprise│
   └──────────────┘          └──────────────┘
```

## Strategic Recommendations

### Near-term (Now)

1. **Position as "DevOps for Agent Skills"** — testing, enforcement, monitoring
2. **Target individual developers and small teams** using Claude Code, Codex, Cursor
3. **Lead with enforcement + testing** value prop (immediate pain point)
4. **PyPI + npm packages** for maximum reach (Python/Node ecosystems are 95% of agent devs)

### Mid-term (3-6 months)

1. **Enterprise pilot** with Architecture 2 (centralized service)
2. **Integration guides** for LangChain, CrewAI, Strands, Vercel AI SDK
3. **Benchmarks** comparing SKM cascade vs naive skill selection

### Long-term (6-12 months)

1. **Skill marketplace integration** — pull from JFrog/skillshub, run with SKM
2. **Multi-agent routing** — not just "which skill for this query" but "which agent for this query"
3. **Organization-level analytics** — skill usage across all agents, optimization recommendations

## Key Insight

> SKM's moat is not "we do selection better." It's that we're the only framework-agnostic runtime that covers the FULL skill lifecycle — from parsing to selection to enforcement to learning — in a single, composable library. Everyone else does one piece.

---

*Last updated: 2026-03-30*
