# Docs Index
Status: Current Index

Priority Agent keeps a lot of historical planning and audit notes. For current
implementation truth, start with the canonical docs below and then read the
code/tests for the exact surface you plan to change.

The root of this directory is reserved for current status, active workstreams,
and high-value reference docs. Older plans and completed audits live under
`archive/`.

## Canonical

| Doc | Use |
|-----|-----|
| [PROJECT_STATUS.md](PROJECT_STATUS.md) | Current project status, validated baselines, release evidence, and known issues. |
| [PROJECT_MAP.md](PROJECT_MAP.md) | Navigation map for runtime entrypoints, tools, memory, TUI, desktop, and validation. |
| [THREAT_MODEL.md](THREAT_MODEL.md) | Current local-agent threat model, trust boundaries, mitigations, and residual risks. |
| [SECURITY_RELEASE_CHECKLIST.md](SECURITY_RELEASE_CHECKLIST.md) | Release security checklist for gates, secret review, dependency review, and artifact checks. |
| [PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md](PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md) | Product principles: narrow, deep, personal, local, and verifiable. |
| [RELEASE_STRUCTURE_CLEANUP_RECOMMENDATIONS_2026-06-22.md](RELEASE_STRUCTURE_CLEANUP_RECOMMENDATIONS_2026-06-22.md) | Current release-structure cleanup plan and release-ready definition. |
| [REMAINING_STRUCTURE_REFINEMENT_PLAN_2026-06-22.md](REMAINING_STRUCTURE_REFINEMENT_PLAN_2026-06-22.md) | Follow-up structure refinement record: public API surface, maturity labels, source grouping, wording, and file-size guard. |
| [CODE_DOCUMENTATION_PLAN_2026-06-22.md](CODE_DOCUMENTATION_PLAN_2026-06-22.md) | Code documentation standard and rollout plan for module docs, public API rustdoc, and boundary comments. |
| [FRIEND_REVIEW_CODE_QUALITY_IMPROVEMENT_PLAN_2026-06-24.md](FRIEND_REVIEW_CODE_QUALITY_IMPROVEMENT_PLAN_2026-06-24.md) | External review triage and next quality-hardening plan for release trust, CI, cache safety, `priority-core`, credentials, and platform boundaries. |
| [QUALITY_GATES.md](../QUALITY_GATES.md) | Root release and phase gate definitions. |
| [TESTING.md](../TESTING.md) | Root testing command guide. |

## Current Workstreams

| Doc | Use |
|-----|-----|
| [DESKTOP_FRONTEND_PRODUCT_PLAN_2026-06-21.md](DESKTOP_FRONTEND_PRODUCT_PLAN_2026-06-21.md) | Desktop workbench product direction and QA path. |
| [DESKTOP_FRONTEND_COMPLETION_AUDIT_2026-06-22.md](DESKTOP_FRONTEND_COMPLETION_AUDIT_2026-06-22.md) | Requirement-level desktop completion evidence. |
| [DESKTOP_FRONTEND_CHANGESET_CLOSEOUT_2026-06-22.md](DESKTOP_FRONTEND_CHANGESET_CLOSEOUT_2026-06-22.md) | Desktop changeset scope, validation evidence, and closeout notes. |
| [NEXT_PRIORITY_CORE_WEIGHT_REFINEMENT_PLAN_2026-06-24.md](NEXT_PRIORITY_CORE_WEIGHT_REFINEMENT_PLAN_2026-06-24.md) | Next focused cleanup for `priority-core` weight analysis ownership, completion-state semantics, and status/validation truthfulness. |
| [LABRUN_GENERAL_AGENT_INTEGRATION_PLAN_2026-06-24.md](LABRUN_GENERAL_AGENT_INTEGRATION_PLAN_2026-06-24.md) | Next development plan for LabRun/general-agent integration, `/lab next`, actionable Lab context, stage-aware tools, permission presets, and proof surfaces. |
| [LABRUN_VALIDATION_SECURITY_AND_REVIEW_HARDENING_PLAN_2026-06-25.md](LABRUN_VALIDATION_SECURITY_AND_REVIEW_HARDENING_PLAN_2026-06-25.md) | LabRun validation-command security, artifact semantic gates, postdoc read-only audit, provider safeguards, and path-scope hardening plan. |
| [LABRUN_POST_HARDENING_FOLLOWUP_PLAN_2026-06-25.md](LABRUN_POST_HARDENING_FOLLOWUP_PLAN_2026-06-25.md) | Follow-up plan for truthful `/lab plan` gate output, code-aware postdoc audit, isolated-worktree enforcement, and package-script validation trust. |
| [LABRUN_SECURITY_GOVERNANCE_NEXT_PLAN_2026-06-25.md](LABRUN_SECURITY_GOVERNANCE_NEXT_PLAN_2026-06-25.md) | Next security/governance plan for LabRun audit redaction, validation argument hardening, package-script trust, role/stage policy overlay, and release governance files. |
| [LAB_AGENT_WORKFLOW_PLAN_2026-06-18.md](LAB_AGENT_WORKFLOW_PLAN_2026-06-18.md) | LabRun workflow design and staged agent roles. |
| [LAB_GRADUATE_EXECUTION_POLICY_DISCUSSION_2026-06-21.md](LAB_GRADUATE_EXECUTION_POLICY_DISCUSSION_2026-06-21.md) | LabRun graduate execution policy and evidence boundaries. |
| [UNWIRED_MODULES_AUDIT_2026-06-18.md](UNWIRED_MODULES_AUDIT_2026-06-18.md) | Audit of modules that are connected, opt-in, deprecated, or intentionally not on the main path. |

## Runtime And Architecture References

| Doc | Use |
|-----|-----|
| [UNIFIED_RUNTIME_ENTRYPOINTS_2026-06-01.md](UNIFIED_RUNTIME_ENTRYPOINTS_2026-06-01.md) | Runtime entrypoint convergence. |
| [CONTROLLER_MERGE_PLAN.md](CONTROLLER_MERGE_PLAN.md) | Conversation-loop controller boundaries and merge plan. |
| [ROUTING_AND_CONTEXT_ANALYSIS_2026-06-08.md](ROUTING_AND_CONTEXT_ANALYSIS_2026-06-08.md) | Routing and context-analysis notes. |
| [CONTEXT_INJECTION_AUDIT_2026-06-17.md](CONTEXT_INJECTION_AUDIT_2026-06-17.md) | Context injection audit and cleanup findings. |
| [CACHE_COMPRESSION_AUDIT_2026-06-17.md](CACHE_COMPRESSION_AUDIT_2026-06-17.md) | Cache and compression audit. |
| [MEMORY_SYSTEM_AUDIT_2026-06-17.md](MEMORY_SYSTEM_AUDIT_2026-06-17.md) | Memory system audit. |

## Provider And Evaluation References

| Doc | Use |
|-----|-----|
| [PROVIDER_ONBOARDING_PLAN_2026-06-10.md](PROVIDER_ONBOARDING_PLAN_2026-06-10.md) | Provider onboarding and setup flow. |
| [PROVIDER_MODEL_UNIFICATION_PLAN_2026-06-15.md](PROVIDER_MODEL_UNIFICATION_PLAN_2026-06-15.md) | Provider/model unification plan. |
| [PROVIDER_CERTIFICATION_MATRIX.md](PROVIDER_CERTIFICATION_MATRIX.md) | Provider certification status. |
| [EVAL_SUITE_ARCHITECTURE_AND_NEXT_STEPS.md](EVAL_SUITE_ARCHITECTURE_AND_NEXT_STEPS.md) | Eval suite architecture and follow-up work. |
| [RELEASE_CANDIDATE_STABILITY_TEST_PLAN_2026-06-05.md](RELEASE_CANDIDATE_STABILITY_TEST_PLAN_2026-06-05.md) | Release-candidate stability testing plan. |

## Useful Directories

| Directory | Contents |
|-----------|----------|
| [archive/](archive/) | Historical docs that should not be treated as current truth without re-checking code. |
| [benchmarks/](benchmarks/) | Live-eval snapshots, reports, and benchmark evidence. |
| [generated/](generated/) | Generated docs. |
| [lab/](lab/) | LabRun-specific docs. |
| [rc/](rc/) | Release-candidate material. |
| [workflow/](workflow/) | Workflow and gate specs. |

## Root Docs

| Doc | Use |
|-----|-----|
| [README.md](../README.md) | Repository entrypoint, quick start, and architecture summary. |
| [QUICKSTART.md](../QUICKSTART.md) | Install, provider setup, run, and basic validation. |
| [SECURITY.md](../SECURITY.md) | Security reporting policy, known limitations, and sensitive change guidance. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Contribution workflow, validation expectations, and security-sensitive change checklist. |
| [AGENTS.md](../AGENTS.md) | Prompt-injected runtime guidance. |
| [CLAUDE.md](../CLAUDE.md) | Claude Code-compatible compact project guide. |
| [PLAN.md](../PLAN.md) | Root plan snapshot. |
| [CAPABILITY_MATRIX.md](../CAPABILITY_MATRIX.md) | Capability matrix. |

## Suggested Reading Order

New contributor:

```text
PROJECT_STATUS.md -> PROJECT_MAP.md -> PERSONAL_AGENT_PRODUCT_PRINCIPLES_2026-05-18.md
```

Release cleanup:

```text
RELEASE_STRUCTURE_CLEANUP_RECOMMENDATIONS_2026-06-22.md -> REMAINING_STRUCTURE_REFINEMENT_PLAN_2026-06-22.md -> QUALITY_GATES.md -> TESTING.md
```

Runtime changes:

```text
PROJECT_MAP.md -> CONTROLLER_MERGE_PLAN.md -> UNIFIED_RUNTIME_ENTRYPOINTS_2026-06-01.md
```

Desktop workbench:

```text
DESKTOP_FRONTEND_PRODUCT_PLAN_2026-06-21.md -> DESKTOP_FRONTEND_COMPLETION_AUDIT_2026-06-22.md
```
