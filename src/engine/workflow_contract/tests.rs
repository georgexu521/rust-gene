use super::*;
use crate::engine::intent_router::IntentRouter;

#[test]
fn prompt_emphasizes_model_led_judgment() {
    let route = IntentRouter::new().route("帮我做一个网站");
    let prompt = WorkflowContractPrompt::new("帮我做一个网站", route, ".").render();

    assert!(prompt.contains("You provide judgment"));
    assert!(prompt.contains("Do not assume the user must fill in numeric weights"));
    assert!(prompt.contains("Treat acceptance criteria as a checklist"));
    assert!(prompt.contains("If the user names a required file/path/command"));
    assert!(prompt.contains("Return only valid JSON"));
}

#[test]
fn code_change_routes_need_model_judgment() {
    let route = IntentRouter::new().route("实现一个新网站");
    let prompt = WorkflowContractPrompt::new("实现一个新网站", route, ".");

    assert!(prompt.should_ask_model());
}

#[test]
fn direct_routes_can_skip_model_judgment() {
    let route = IntentRouter::new().route("你好");
    let prompt = WorkflowContractPrompt::new("你好", route, ".");

    assert!(!prompt.should_ask_model());
}

#[test]
fn parse_judgment_from_fenced_text() {
    let content = r#"```json
{
  "task_type": "feature",
  "complexity": "medium",
  "risk": "medium",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": ["Use existing patterns"],
  "guided_reasoning_required": false,
  "guided_reasoning_triggers": [],
  "plan": [
{
  "description": "Inspect existing code",
  "priority": "p0",
  "weight": 1.2,
  "reason": "Need context before editing",
  "acceptance_criteria": ["Relevant files read"]
}
  ],
  "acceptance": {
"original_user_goal": "Add feature",
"assumptions": [],
"criteria": [],
"unresolved_items": [],
"residual_risks": []
  }
}
```"#;

    let judgment = parse_workflow_judgment(content).unwrap();

    assert_eq!(judgment.plan[0].weight, Some(1.0));
    assert_eq!(judgment.plan[0].importance_score, Some(1.0));
    assert_eq!(judgment.plan[0].computed_weight_share(), 1.0);
    assert_eq!(judgment.acceptance.criteria.len(), 1);
    assert_eq!(judgment.sorted_plan()[0].priority, PriorityLabel::P0);
}

#[test]
fn parse_judgment_accepts_json5_model_output() {
    let content = r#"{
  task_type: 'feature',
  complexity: 'medium',
  risk: 'medium',
  requirement_complete_enough: true,
  needs_user_questions: false,
  question_reason: null,
  questions: [],
  assumptions: ['Use the existing live-eval script layout'],
  guided_reasoning_required: false,
  guided_reasoning_triggers: [],
  plan: [
{
  id: 'summary',
  description: 'Implement summary mode',
  priority: 'p0',
  importance_score: 0.9,
  weight_share: 1.0,
  reason: 'Required summary command is blocked',
  acceptance_criteria: ['summary command passes'],
},
  ],
  acceptance: {
original_user_goal: 'Add live eval summary mode',
assumptions: [],
criteria: [],
unresolved_items: [],
residual_risks: [],
  },
}"#;

    let judgment = parse_workflow_judgment(content).unwrap();

    assert_eq!(judgment.task_type, "feature");
    assert_eq!(judgment.plan[0].id.as_deref(), Some("summary"));
    assert_eq!(judgment.acceptance.criteria.len(), 1);
}

#[test]
fn workflow_judgment_tool_like_parse_noise_is_recoverable() {
    let err = parse_workflow_judgment(r#"{tool => "file_read", args => {"path": "x"}}"#)
        .expect_err("tool-like model leakage is not valid judgment JSON");

    assert!(is_recoverable_workflow_judgment_parse_error(&err));
}

#[test]
fn workflow_judgment_schema_errors_still_surface() {
    let err = parse_workflow_judgment(r#"{"task_type":"feature"}"#)
        .expect_err("incomplete JSON should fail schema validation");

    assert!(!is_recoverable_workflow_judgment_parse_error(&err));
}

#[test]
fn computes_factor_based_importance_and_weight_share() {
    let content = r#"{
  "task_type": "feature",
  "complexity": "high",
  "risk": "high",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": [],
  "guided_reasoning_required": true,
  "guided_reasoning_triggers": ["high_risk_area"],
  "plan": [
{
  "id": "schema",
  "description": "Define data model",
  "priority": "p2",
  "factors": {
    "dependency": 0.95,
    "user_value": 0.80,
    "risk_reduction": 0.70,
    "uncertainty_reduction": 0.75,
    "blocking": 0.90,
    "cost": 0.30
  },
  "reason": "Everything depends on schema",
  "acceptance_criteria": ["Schema supports tags"]
},
{
  "id": "ui",
  "description": "Polish UI",
  "priority": "p2",
  "factors": {
    "dependency": 0.20,
    "user_value": 0.60,
    "risk_reduction": 0.20,
    "uncertainty_reduction": 0.20,
    "blocking": 0.10,
    "cost": 0.40
  },
  "reason": "Important but not blocking",
  "acceptance_criteria": ["UI remains usable"]
}
  ],
  "acceptance": {
"original_user_goal": "Build app",
"assumptions": [],
"criteria": [],
"unresolved_items": [],
"residual_risks": []
  }
}"#;

    let judgment = parse_workflow_judgment(content).unwrap();
    let sorted = judgment.sorted_plan();

    assert_eq!(sorted[0].id.as_deref(), Some("schema"));
    assert_eq!(sorted[0].weight_source(), Some(WeightSource::Factors));
    assert!(sorted[0].normalized_weight() > sorted[1].normalized_weight());
    let total_share = judgment
        .plan
        .iter()
        .map(WorkflowPlanStep::computed_weight_share)
        .sum::<f32>();
    assert!((total_share - 1.0).abs() < 0.001);
}

#[test]
fn workflow_judgment_defaults_missing_factor_fields() {
    let content = r#"{
  "task_type": "bug_fix",
  "complexity": "medium",
  "risk": "high",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": [],
  "guided_reasoning_required": true,
  "guided_reasoning_triggers": ["high_risk_area"],
  "plan": [
{
  "id": "validate",
  "description": "Run required validation",
  "priority": "p1",
  "factors": {
    "dependency": 0.7,
    "user_value": 0.8,
    "risk_reduction": 0.9,
    "blocking": 0.8,
    "cost": 0.2
  },
  "reason": "Validation proves whether the current behavior is already satisfied",
  "acceptance_criteria": ["Required commands pass"]
}
  ],
  "acceptance": {
"original_user_goal": "Audit memory behavior",
"assumptions": [],
"criteria": [],
"unresolved_items": [],
"residual_risks": []
  }
}"#;

    let judgment = parse_workflow_judgment(content).unwrap();

    let factors = judgment.plan[0].factors.expect("factors should parse");
    assert_eq!(factors.uncertainty_reduction, 0.0);
    assert!(judgment.plan[0].normalized_weight() > 0.0);
}

#[test]
fn high_risk_zero_factor_plan_gets_actionable_priority_floor() {
    let content = r#"{
  "task_type": "bug_fix",
  "complexity": "medium",
  "risk": "high",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": [],
  "guided_reasoning_required": true,
  "guided_reasoning_triggers": ["high_risk_area"],
  "plan": [
{
  "id": "inspect",
  "description": "Inspect memory planning integration",
  "priority": "p3",
  "factors": {
    "dependency": 0.0,
    "user_value": 0.0,
    "risk_reduction": 0.0,
    "uncertainty_reduction": 0.0,
    "blocking": 0.0,
    "cost": 0.0
  },
  "reason": "Model supplied empty factors",
  "acceptance_criteria": ["Relevant path identified"]
},
{
  "id": "validate",
  "description": "Run memory planning tests",
  "priority": "p3",
  "factors": {
    "dependency": 0.0,
    "user_value": 0.0,
    "risk_reduction": 0.0,
    "uncertainty_reduction": 0.0,
    "blocking": 0.0,
    "cost": 0.0
  },
  "reason": "Model supplied empty factors",
  "acceptance_criteria": ["Tests pass"]
}
  ],
  "acceptance": {
"original_user_goal": "Repair memory planning",
"assumptions": [],
"criteria": [],
"unresolved_items": [],
"residual_risks": []
  }
}"#;

    let judgment = parse_workflow_judgment(content).unwrap();
    let top = judgment.top_plan_step().unwrap();

    assert_eq!(top.id.as_deref(), Some("inspect"));
    assert!(matches!(
        top.priority,
        PriorityLabel::P0 | PriorityLabel::P1 | PriorityLabel::P2
    ));
    assert!(top.normalized_weight() >= 0.40);
}

#[test]
fn rejects_large_low_confidence_override() {
    let mut step = WorkflowPlanStep {
        id: Some("ui".into()),
        description: "Polish UI".into(),
        priority: PriorityLabel::P2,
        weight: None,
        importance_score: Some(0.40),
        weight_share: None,
        factors: None,
        override_adjustment: Some(WeightOverride {
            adjusted_importance_score: 0.90,
            reason: "looks better".into(),
            confidence: 0.40,
        }),
        computation: None,
        reason: "visual task".into(),
        acceptance_criteria: Vec::new(),
    };

    step.computation = Some(compute_step_weight(&step));

    let computation = step.computation.unwrap();
    assert_eq!(computation.override_status, WeightOverrideStatus::Rejected);
    assert!((computation.adjusted_importance_score - 0.40).abs() < 0.001);
}

#[test]
fn feedback_reweights_acceptance_gaps_upward() {
    let mut step = WorkflowPlanStep {
        id: Some("validation".into()),
        description: "Validate persistence".into(),
        priority: PriorityLabel::P2,
        weight: None,
        importance_score: None,
        weight_share: None,
        factors: Some(WeightFactors {
            dependency: 0.40,
            user_value: 0.45,
            risk_reduction: 0.40,
            uncertainty_reduction: 0.35,
            blocking: 0.30,
            cost: 0.25,
        }),
        override_adjustment: None,
        computation: None,
        reason: "Need evidence".into(),
        acceptance_criteria: Vec::new(),
    };
    recompute_step_weight(&mut step);
    let before = step.normalized_weight();

    apply_weight_feedback(
        &mut step,
        &WeightFeedbackEvent {
            kind: WeightFeedbackKind::AcceptanceGap,
            severity: WeightFeedbackSeverity::High,
            confidence: 0.90,
            reason: Some("acceptance review found missing persistence proof".into()),
        },
    );

    assert!(step.normalized_weight() > before + 0.05);
    assert!(matches!(
        step.priority,
        PriorityLabel::P0 | PriorityLabel::P1 | PriorityLabel::P2
    ));
}

#[test]
fn completed_step_feedback_lowers_remaining_importance() {
    let mut step = WorkflowPlanStep {
        id: Some("inspect".into()),
        description: "Inspect entry points".into(),
        priority: PriorityLabel::P0,
        weight: None,
        importance_score: None,
        weight_share: None,
        factors: Some(WeightFactors {
            dependency: 0.95,
            user_value: 0.70,
            risk_reduction: 0.80,
            uncertainty_reduction: 0.90,
            blocking: 0.90,
            cost: 0.30,
        }),
        override_adjustment: None,
        computation: None,
        reason: "Entry points block the refactor".into(),
        acceptance_criteria: Vec::new(),
    };
    recompute_step_weight(&mut step);
    let before = step.normalized_weight();

    apply_weight_feedback(
        &mut step,
        &WeightFeedbackEvent {
            kind: WeightFeedbackKind::StepCompleted,
            severity: WeightFeedbackSeverity::High,
            confidence: 1.0,
            reason: Some("entry points inspected".into()),
        },
    );

    assert!(step.normalized_weight() < before);
}

#[test]
fn normalizes_only_open_steps_when_completed_ids_are_known() {
    let mut steps = vec![
        WorkflowPlanStep {
            id: Some("done".into()),
            description: "Completed work".into(),
            priority: PriorityLabel::P0,
            weight: Some(0.90),
            importance_score: None,
            weight_share: None,
            factors: None,
            override_adjustment: None,
            computation: None,
            reason: "Already complete".into(),
            acceptance_criteria: Vec::new(),
        },
        WorkflowPlanStep {
            id: Some("open".into()),
            description: "Open work".into(),
            priority: PriorityLabel::P1,
            weight: Some(0.70),
            importance_score: None,
            weight_share: None,
            factors: None,
            override_adjustment: None,
            computation: None,
            reason: "Still needed".into(),
            acceptance_criteria: Vec::new(),
        },
    ];
    for step in &mut steps {
        recompute_step_weight(step);
    }

    normalize_open_weight_shares(&mut steps, &[String::from("done")]);

    assert_eq!(steps[0].computed_weight_share(), 0.0);
    assert_eq!(steps[1].computed_weight_share(), 1.0);
}

#[test]
fn detects_meaningful_reweight_changes() {
    let old_steps = vec![WorkflowPlanStep {
        id: Some("repair".into()),
        description: "Repair persistence".into(),
        priority: PriorityLabel::P2,
        weight: Some(0.45),
        importance_score: Some(0.45),
        weight_share: Some(1.0),
        factors: None,
        override_adjustment: None,
        computation: Some(WeightComputation {
            formula_importance_score: 0.45,
            adjusted_importance_score: 0.45,
            weight_share: 1.0,
            priority: PriorityLabel::P2,
            source: WeightSource::ModelImportance,
            override_status: WeightOverrideStatus::None,
            override_reason: None,
        }),
        reason: "Fix issue".into(),
        acceptance_criteria: Vec::new(),
    }];
    let mut new_steps = old_steps.clone();
    new_steps[0].importance_score = Some(0.75);
    new_steps[0].computation = Some(WeightComputation {
        formula_importance_score: 0.75,
        adjusted_importance_score: 0.75,
        weight_share: 1.0,
        priority: PriorityLabel::P1,
        source: WeightSource::ModelImportance,
        override_status: WeightOverrideStatus::None,
        override_reason: None,
    });
    new_steps[0].priority = PriorityLabel::P1;

    assert!(should_record_reweight(&old_steps, &new_steps));
}

#[test]
fn acceptance_contract_counts_incomplete_items() {
    let contract = AcceptanceContract::pending(
        "Build app",
        vec!["Main flow works".into()],
        vec!["Local storage".into()],
    );

    assert_eq!(contract.incomplete_count(), 1);
}

#[test]
fn parse_workflow_judgment_maps_freeform_guided_triggers() {
    let content = r#"{
  "task_type": "bug_fix",
  "complexity": "medium",
  "risk": "medium",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": [],
  "guided_reasoning_required": true,
  "guided_reasoning_triggers": [
"Need to understand how memory_save currently bypasses gates",
"test_failure"
  ],
  "plan": [
{
  "id": "inspect",
  "description": "Inspect memory_save",
  "priority": "p0",
  "importance_score": 0.8,
  "weight_share": 1.0,
  "reason": "Entry point blocks the fix",
  "acceptance_criteria": ["tests pass"]
}
  ],
  "acceptance": {
"original_user_goal": "Fix memory_save quality gates",
"assumptions": [],
"criteria": [
  {
    "criterion": "memory_save uses quality gates",
    "status": "pending",
    "evidence": null
  }
],
"unresolved_items": [],
"residual_risks": []
  }
}"#;

    let judgment = parse_workflow_judgment(content).unwrap();

    assert_eq!(judgment.guided_reasoning_triggers.len(), 2);
    assert!(judgment
        .guided_reasoning_triggers
        .contains(&GuidedReasoningTrigger::UnfamiliarCodePath));
    assert!(judgment
        .guided_reasoning_triggers
        .contains(&GuidedReasoningTrigger::TestFailure));
}

#[test]
fn acceptance_review_prompt_requires_per_target_evidence() {
    let prompt = AcceptanceReviewPrompt::new(
        AcceptanceContract::pending(
            "Repair save output",
            vec!["src/tui/app.rs no longer hardcodes Saved output".into()],
            Vec::new(),
        ),
        vec!["src/memory/quality.rs".into()],
        true,
        vec!["cargo test passed".into()],
    )
    .render();

    assert!(prompt.contains("Check every independent acceptance target separately"));
    assert!(prompt.contains("If the user named a file"));
    assert!(prompt.contains("mark it failed or not_verified"));
}

#[test]
fn parse_acceptance_review_from_fenced_text() {
    let content = r#"```json
{
  "accepted": false,
  "confidence": "medium",
  "criteria": [
{
  "criterion": "Tests pass",
  "status": "not_verified",
  "evidence": "No test command was run"
}
  ],
  "unresolved_items": ["Run focused tests"],
  "residual_risks": ["Manual browser flow not checked"],
  "next_action": "continue_repair"
}
```"#;

    let review = parse_acceptance_review(content).unwrap();

    assert!(!review.accepted);
    assert_eq!(review.unresolved_count(), 2);
    assert_eq!(review.next_action, AcceptanceNextAction::ContinueRepair);
}

#[test]
fn parse_acceptance_review_accepts_json5_model_output() {
    let content = r#"{
  accepted: true,
  confidence: 'high',
  criteria: [
{
  criterion: 'Required command passed',
  status: 'passed',
  evidence: 'cargo test passed',
},
  ],
  unresolved_items: [],
  residual_risks: [],
  next_action: 'finish',
}"#;

    let review = parse_acceptance_review(content).unwrap();

    assert!(review.accepted);
    assert_eq!(review.confidence, AcceptanceConfidence::High);
    assert_eq!(review.next_action, AcceptanceNextAction::Finish);
}

#[test]
fn parse_acceptance_review_rejects_accepted_with_unresolved_items() {
    let content = r#"{
  "accepted": true,
  "confidence": "low",
  "criteria": [
{
  "criterion": "Required commands passed",
  "status": "not_verified",
  "evidence": "No command output was provided"
}
  ],
  "unresolved_items": ["Run required commands"],
  "residual_risks": [],
  "next_action": "finish"
}"#;

    let review = parse_acceptance_review(content).unwrap();

    assert!(!review.accepted);
    assert_eq!(review.unresolved_count(), 2);
    assert_eq!(review.next_action, AcceptanceNextAction::ContinueRepair);
}

#[test]
fn parse_acceptance_review_sanitizes_structured_items() {
    let content = r#"{
  "accepted": true,
  "confidence": {"level": "high"},
  "criteria": [
{
  "criterion": {"description": "Forbidden retry format is removed"},
  "status": {"result": "failed"},
  "evidence": {"command": "rg retry src/engine/conversation_loop/mod.rs", "output": "match found"}
},
"Focused tests pass"
  ],
  "unresolved_items": [
{"item": "Remove the forbidden pattern"},
{"reason": "Required command is still failing"}
  ],
  "residual_risks": [
{"risk": "Repair loop may leave stale formatting"}
  ],
  "next_action": {"action": "continue-repair"}
}"#;

    let review = parse_acceptance_review(content).unwrap();

    assert!(!review.accepted);
    assert_eq!(review.confidence, AcceptanceConfidence::High);
    assert_eq!(
        review.criteria[0].criterion,
        "Forbidden retry format is removed"
    );
    assert_eq!(review.criteria[0].status, AcceptanceStatus::Failed);
    assert!(review.criteria[0]
        .evidence
        .as_deref()
        .unwrap_or_default()
        .contains("rg retry"));
    assert_eq!(review.criteria[1].criterion, "Focused tests pass");
    assert_eq!(review.criteria[1].status, AcceptanceStatus::NotVerified);
    assert_eq!(review.unresolved_items.len(), 2);
    assert_eq!(review.residual_risks.len(), 1);
    assert_eq!(review.next_action, AcceptanceNextAction::ContinueRepair);
}

#[test]
fn parse_guided_debugging_analysis_from_json() {
    let content = r#"{
  "blocker": true,
  "symptom": "cargo test failed with a type error",
  "likely_causes": ["new enum variant not matched"],
  "evidence_to_collect": ["run cargo check"],
  "smallest_safe_action": "add the missing match arm",
  "ask_user": false,
  "questions": [],
  "next_action": "repair"
}"#;

    let analysis = parse_guided_debugging_analysis(content).unwrap();

    assert!(analysis.blocker);
    assert_eq!(analysis.next_action, DebuggingNextAction::Repair);
    assert!(analysis
        .format_for_prompt()
        .contains("Smallest safe action"));
}
