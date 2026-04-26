//! Model-led programming workflow contracts.
//!
//! This module defines the structured prompt contract used to ask the model to
//! judge programming-task completeness, risk, priority, guided reasoning needs,
//! and acceptance criteria. The software supplies the structure and records the
//! result; the model supplies the judgment.

use crate::engine::intent_router::{IntentRoute, RiskLevel, WorkflowKind};
use crate::services::api::{ChatRequest, LlmProvider, Message};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityLabel {
    P0,
    P1,
    P2,
    P3,
}

impl PriorityLabel {
    pub fn sort_rank(self) -> u8 {
        match self {
            Self::P0 => 0,
            Self::P1 => 1,
            Self::P2 => 2,
            Self::P3 => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidedReasoningTrigger {
    AmbiguousRequirement,
    CompetingApproaches,
    HighRiskArea,
    UnfamiliarCodePath,
    ToolFailure,
    TestFailure,
    UnexpectedDiff,
    RepeatedRepair,
    GoalDrift,
    ContextConflict,
    BroadProductRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceStatus {
    Pending,
    Passed,
    Failed,
    NotVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceNextAction {
    Finish,
    ContinueRepair,
    AskUser,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebuggingNextAction {
    InspectMore,
    Repair,
    AskUser,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPlanStep {
    pub description: String,
    pub priority: PriorityLabel,
    pub weight: Option<f32>,
    pub reason: String,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

impl WorkflowPlanStep {
    pub fn normalized_weight(&self) -> f32 {
        self.weight.unwrap_or_else(|| match self.priority {
            PriorityLabel::P0 => 1.0,
            PriorityLabel::P1 => 0.75,
            PriorityLabel::P2 => 0.5,
            PriorityLabel::P3 => 0.25,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    pub criterion: String,
    pub status: AcceptanceStatus,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceContract {
    pub original_user_goal: String,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub unresolved_items: Vec<String>,
    #[serde(default)]
    pub residual_risks: Vec<String>,
}

impl AcceptanceContract {
    pub fn pending(
        goal: impl Into<String>,
        criteria: Vec<String>,
        assumptions: Vec<String>,
    ) -> Self {
        Self {
            original_user_goal: goal.into(),
            assumptions,
            criteria: criteria
                .into_iter()
                .filter(|criterion| !criterion.trim().is_empty())
                .map(|criterion| AcceptanceCriterion {
                    criterion,
                    status: AcceptanceStatus::Pending,
                    evidence: None,
                })
                .collect(),
            unresolved_items: Vec::new(),
            residual_risks: Vec::new(),
        }
    }

    pub fn incomplete_count(&self) -> usize {
        self.criteria
            .iter()
            .filter(|criterion| !matches!(criterion.status, AcceptanceStatus::Passed))
            .count()
            + self.unresolved_items.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgrammingWorkflowJudgment {
    pub task_type: String,
    pub complexity: TaskComplexity,
    pub risk: RiskLevel,
    pub requirement_complete_enough: bool,
    pub needs_user_questions: bool,
    pub question_reason: Option<String>,
    #[serde(default)]
    pub questions: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub guided_reasoning_required: bool,
    #[serde(default)]
    pub guided_reasoning_triggers: Vec<GuidedReasoningTrigger>,
    #[serde(default)]
    pub plan: Vec<WorkflowPlanStep>,
    pub acceptance: AcceptanceContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceReview {
    pub accepted: bool,
    pub confidence: AcceptanceConfidence,
    #[serde(default)]
    pub criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub unresolved_items: Vec<String>,
    #[serde(default)]
    pub residual_risks: Vec<String>,
    pub next_action: AcceptanceNextAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidedDebuggingAnalysis {
    pub blocker: bool,
    pub symptom: String,
    #[serde(default)]
    pub likely_causes: Vec<String>,
    #[serde(default)]
    pub evidence_to_collect: Vec<String>,
    pub smallest_safe_action: String,
    pub ask_user: bool,
    #[serde(default)]
    pub questions: Vec<String>,
    pub next_action: DebuggingNextAction,
}

impl GuidedDebuggingAnalysis {
    pub fn format_for_prompt(&self) -> String {
        let mut out = format!(
            "Guided debugging analysis: blocker={} next_action={:?}\nSymptom: {}\nSmallest safe action: {}\n",
            self.blocker, self.next_action, self.symptom, self.smallest_safe_action
        );
        if !self.likely_causes.is_empty() {
            out.push_str("Likely causes:\n");
            for cause in &self.likely_causes {
                out.push_str(&format!("- {}\n", cause));
            }
        }
        if !self.evidence_to_collect.is_empty() {
            out.push_str("Evidence to collect:\n");
            for item in &self.evidence_to_collect {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if self.ask_user && !self.questions.is_empty() {
            out.push_str("Questions for user if blocked:\n");
            for question in &self.questions {
                out.push_str(&format!("- {}\n", question));
            }
        }
        out
    }
}

impl AcceptanceReview {
    pub fn unresolved_count(&self) -> usize {
        self.criteria
            .iter()
            .filter(|criterion| !matches!(criterion.status, AcceptanceStatus::Passed))
            .count()
            + self.unresolved_items.len()
    }

    pub fn format_for_prompt(&self) -> String {
        let mut out = format!(
            "Acceptance review: accepted={} confidence={:?} next_action={:?}\n",
            self.accepted, self.confidence, self.next_action
        );
        if !self.criteria.is_empty() {
            out.push_str("Criteria:\n");
            for criterion in &self.criteria {
                out.push_str(&format!(
                    "- [{:?}] {}{}{}\n",
                    criterion.status,
                    criterion.criterion,
                    if criterion.evidence.is_some() {
                        " -- "
                    } else {
                        ""
                    },
                    criterion.evidence.as_deref().unwrap_or("")
                ));
            }
        }
        if !self.unresolved_items.is_empty() {
            out.push_str("Unresolved items:\n");
            for item in &self.unresolved_items {
                out.push_str(&format!("- {}\n", item));
            }
        }
        if !self.residual_risks.is_empty() {
            out.push_str("Residual risks:\n");
            for risk in &self.residual_risks {
                out.push_str(&format!("- {}\n", risk));
            }
        }
        out
    }
}

impl ProgrammingWorkflowJudgment {
    pub fn sorted_plan(&self) -> Vec<WorkflowPlanStep> {
        let mut steps = self.plan.clone();
        steps.sort_by(|a, b| {
            a.priority
                .sort_rank()
                .cmp(&b.priority.sort_rank())
                .then_with(|| {
                    b.normalized_weight()
                        .partial_cmp(&a.normalized_weight())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        steps
    }

    pub fn acceptance_checks(&self) -> Vec<String> {
        self.acceptance
            .criteria
            .iter()
            .map(|criterion| criterion.criterion.clone())
            .collect()
    }

    pub fn risk_notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        notes.push(format!("model-judged risk: {:?}", self.risk));
        if self.guided_reasoning_required {
            notes.push(format!(
                "guided reasoning triggers: {:?}",
                self.guided_reasoning_triggers
            ));
        }
        notes.extend(self.acceptance.residual_risks.clone());
        notes
    }

    pub fn to_turn_context(&self) -> String {
        let mut out = String::from("Model-led programming workflow judgment for this turn:\n");
        out.push_str(&format!(
            "- task_type: {}\n- complexity: {:?}\n- risk: {:?}\n",
            self.task_type, self.complexity, self.risk
        ));
        out.push_str(&format!(
            "- requirement_complete_enough: {}\n- needs_user_questions: {}\n",
            self.requirement_complete_enough, self.needs_user_questions
        ));
        if let Some(reason) = &self.question_reason {
            out.push_str(&format!("- question_reason: {}\n", reason));
        }
        if !self.questions.is_empty() {
            out.push_str("- questions_to_ask_before_execution:\n");
            for question in self.questions.iter().take(5) {
                out.push_str(&format!("  - {}\n", question));
            }
        }
        if !self.assumptions.is_empty() {
            out.push_str("- assumptions_if_proceeding:\n");
            for assumption in self.assumptions.iter().take(6) {
                out.push_str(&format!("  - {}\n", assumption));
            }
        }
        if !self.plan.is_empty() {
            out.push_str("- prioritized_plan:\n");
            for step in self.sorted_plan().iter().take(8) {
                out.push_str(&format!(
                    "  - [{:?} {:.2}] {} -- {}\n",
                    step.priority,
                    step.normalized_weight(),
                    step.description,
                    step.reason
                ));
            }
        }
        if !self.acceptance.criteria.is_empty() {
            out.push_str("- acceptance_criteria:\n");
            for criterion in self.acceptance.criteria.iter().take(8) {
                out.push_str(&format!("  - {}\n", criterion.criterion));
            }
        }
        out.push_str(
            "Use this as operating context. Ask the listed questions if they block correctness; otherwise proceed under the assumptions and verify against the acceptance criteria before final response.",
        );
        out
    }
}

#[derive(Debug, Clone)]
pub struct WorkflowContractPrompt {
    pub user_request: String,
    pub route: IntentRoute,
    pub working_dir: String,
}

impl WorkflowContractPrompt {
    pub fn new(
        user_request: impl Into<String>,
        route: IntentRoute,
        working_dir: impl Into<String>,
    ) -> Self {
        Self {
            user_request: user_request.into(),
            route,
            working_dir: working_dir.into(),
        }
    }

    pub fn should_ask_model(&self) -> bool {
        matches!(
            self.route.workflow,
            WorkflowKind::CodeChange
                | WorkflowKind::BugFix
                | WorkflowKind::Planning
                | WorkflowKind::Delegation
        )
    }

    pub fn render(&self) -> String {
        format!(
            r#"You are producing a model-led programming workflow judgment for Priority Agent.

Important principle:
- The software provides structure.
- You provide judgment.
- Do not assume the user must fill in numeric weights.
- Weight/priority is only a way to decide which plan step matters more.
- Use guided reasoning only when the task is complex, ambiguous, risky, or failing.
- Keep the output compact and operational.

User request:
{user_request}

Working directory:
{working_dir}

Advisory route from the runtime:
- intent: {intent:?}
- workflow: {workflow:?}
- retrieval: {retrieval:?}
- reasoning: {reasoning:?}
- risk: {risk:?}
- reason: {reason}

Return only valid JSON with this shape:
{{
  "task_type": "bug_fix | feature | refactor | website | investigation | review | other",
  "complexity": "low | medium | high",
  "risk": "low | medium | high",
  "requirement_complete_enough": true,
  "needs_user_questions": false,
  "question_reason": null,
  "questions": [],
  "assumptions": [],
  "guided_reasoning_required": false,
  "guided_reasoning_triggers": [],
  "plan": [
    {{
      "description": "short action",
      "priority": "p0 | p1 | p2 | p3",
      "weight": 0.86,
      "reason": "why this comes before other work",
      "acceptance_criteria": ["concrete check"]
    }}
  ],
  "acceptance": {{
    "original_user_goal": "restated user goal",
    "assumptions": [],
    "criteria": [
      {{
        "criterion": "what must be true before closeout",
        "status": "pending",
        "evidence": null
      }}
    ],
    "unresolved_items": [],
    "residual_risks": []
  }}
}}

Guidance:
- Ask user questions only when missing information affects architecture, data, permissions, deployment, UX, or acceptance criteria.
- If a conservative default is safe, proceed and record the assumption.
- For simple tasks, keep the plan short.
- For complex or high-risk tasks, include acceptance criteria and guided reasoning triggers.
- Prioritize using P0/P1/P2/P3 and optionally a weight. The explanation matters more than the number.
"#,
            user_request = self.user_request,
            working_dir = self.working_dir,
            intent = self.route.intent,
            workflow = self.route.workflow,
            retrieval = self.route.retrieval,
            reasoning = self.route.reasoning,
            risk = self.route.risk,
            reason = self.route.reason
        )
    }
}

#[derive(Debug, Clone)]
pub struct AcceptanceReviewPrompt {
    pub contract: AcceptanceContract,
    pub changed_files: Vec<String>,
    pub verification_passed: bool,
    pub evidence: Vec<String>,
}

impl AcceptanceReviewPrompt {
    pub fn new(
        contract: AcceptanceContract,
        changed_files: Vec<String>,
        verification_passed: bool,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            contract,
            changed_files,
            verification_passed,
            evidence,
        }
    }

    pub fn render(&self) -> String {
        format!(
            r#"You are performing a model-led acceptance review for a programming task.

Judge whether the implementation satisfies the original user goal and acceptance criteria.
Do not pass criteria just because the intent was good. Use the evidence.
If evidence is missing, mark that criterion as not_verified.
If the task should continue, choose continue_repair. If a human choice is needed, choose ask_user.

Original goal:
{goal}

Assumptions:
{assumptions}

Original acceptance criteria:
{criteria}

Changed files:
{changed_files}

Verification passed:
{verification_passed}

Evidence:
{evidence}

Return only valid JSON with this shape:
{{
  "accepted": true,
  "confidence": "low | medium | high",
  "criteria": [
    {{
      "criterion": "criterion text",
      "status": "passed | failed | not_verified | pending",
      "evidence": "short evidence or null"
    }}
  ],
  "unresolved_items": [],
  "residual_risks": [],
  "next_action": "finish | continue_repair | ask_user | stop"
}}
"#,
            goal = self.contract.original_user_goal,
            assumptions = bullet_list(&self.contract.assumptions),
            criteria = bullet_list(
                &self
                    .contract
                    .criteria
                    .iter()
                    .map(|criterion| criterion.criterion.clone())
                    .collect::<Vec<_>>()
            ),
            changed_files = bullet_list(&self.changed_files),
            verification_passed = self.verification_passed,
            evidence = bullet_list(&self.evidence),
        )
    }
}

#[derive(Debug, Clone)]
pub struct GuidedDebuggingPrompt {
    pub user_request: String,
    pub workflow_context: Option<String>,
    pub failed_tools: Vec<String>,
    pub evidence: Vec<String>,
}

impl GuidedDebuggingPrompt {
    pub fn new(
        user_request: impl Into<String>,
        workflow_context: Option<String>,
        failed_tools: Vec<String>,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            user_request: user_request.into(),
            workflow_context,
            failed_tools,
            evidence,
        }
    }

    pub fn render(&self) -> String {
        format!(
            r#"You are performing guided debugging for a programming-agent workflow.

The agent hit a failure. Do not guess. Decide whether this is a blocker, what evidence resolves it fastest, and what the next safest action is.
Ask the user only when the next step requires a human product/permission/architecture decision.

User request:
{user_request}

Workflow context:
{workflow_context}

Failed tools:
{failed_tools}

Evidence:
{evidence}

Return only valid JSON with this shape:
{{
  "blocker": false,
  "symptom": "exact failure in one sentence",
  "likely_causes": ["cause"],
  "evidence_to_collect": ["focused check"],
  "smallest_safe_action": "next action",
  "ask_user": false,
  "questions": [],
  "next_action": "inspect_more | repair | ask_user | stop"
}}
"#,
            user_request = self.user_request,
            workflow_context = self
                .workflow_context
                .as_deref()
                .unwrap_or("No structured workflow context was available."),
            failed_tools = bullet_list(&self.failed_tools),
            evidence = bullet_list(&self.evidence),
        )
    }
}

pub struct WorkflowContractAnalyzer<'a> {
    provider: &'a dyn LlmProvider,
    model: String,
}

impl<'a> WorkflowContractAnalyzer<'a> {
    pub fn new(provider: &'a dyn LlmProvider, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }

    pub async fn analyze(
        &self,
        prompt: WorkflowContractPrompt,
    ) -> anyhow::Result<ProgrammingWorkflowJudgment> {
        let request = ChatRequest::new(self.model.clone())
            .with_temperature(0.1)
            .with_messages(vec![
                Message::system("Return only valid JSON. Do not include markdown fences."),
                Message::user(prompt.render()),
            ]);
        let response = self.provider.chat(request).await?;
        parse_workflow_judgment(&response.content)
    }

    pub async fn review_acceptance(
        &self,
        prompt: AcceptanceReviewPrompt,
    ) -> anyhow::Result<AcceptanceReview> {
        let request = ChatRequest::new(self.model.clone())
            .with_temperature(0.1)
            .with_messages(vec![
                Message::system("Return only valid JSON. Do not include markdown fences."),
                Message::user(prompt.render()),
            ]);
        let response = self.provider.chat(request).await?;
        parse_acceptance_review(&response.content)
    }

    pub async fn analyze_debugging(
        &self,
        prompt: GuidedDebuggingPrompt,
    ) -> anyhow::Result<GuidedDebuggingAnalysis> {
        let request = ChatRequest::new(self.model.clone())
            .with_temperature(0.1)
            .with_messages(vec![
                Message::system("Return only valid JSON. Do not include markdown fences."),
                Message::user(prompt.render()),
            ]);
        let response = self.provider.chat(request).await?;
        parse_guided_debugging_analysis(&response.content)
    }
}

pub fn parse_workflow_judgment(content: &str) -> anyhow::Result<ProgrammingWorkflowJudgment> {
    let json = extract_json_object(content)
        .ok_or_else(|| anyhow::anyhow!("workflow judgment response did not contain JSON"))?;
    let mut judgment: ProgrammingWorkflowJudgment = serde_json::from_str(json)?;
    normalize_judgment(&mut judgment);
    Ok(judgment)
}

pub fn parse_acceptance_review(content: &str) -> anyhow::Result<AcceptanceReview> {
    let json = extract_json_object(content)
        .ok_or_else(|| anyhow::anyhow!("acceptance review response did not contain JSON"))?;
    Ok(serde_json::from_str(json)?)
}

pub fn parse_guided_debugging_analysis(content: &str) -> anyhow::Result<GuidedDebuggingAnalysis> {
    let json = extract_json_object(content)
        .ok_or_else(|| anyhow::anyhow!("guided debugging response did not contain JSON"))?;
    Ok(serde_json::from_str(json)?)
}

fn normalize_judgment(judgment: &mut ProgrammingWorkflowJudgment) {
    for step in &mut judgment.plan {
        if let Some(weight) = step.weight.as_mut() {
            *weight = weight.clamp(0.0, 1.0);
        }
    }
    if judgment.acceptance.original_user_goal.trim().is_empty() {
        judgment.acceptance.original_user_goal = judgment.task_type.clone();
    }
    if judgment.acceptance.criteria.is_empty() {
        judgment.acceptance.criteria = judgment
            .plan
            .iter()
            .flat_map(|step| step.acceptance_criteria.clone())
            .filter(|criterion| !criterion.trim().is_empty())
            .map(|criterion| AcceptanceCriterion {
                criterion,
                status: AcceptanceStatus::Pending,
                evidence: None,
            })
            .collect();
    }
}

fn extract_json_object(content: &str) -> Option<&str> {
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }

    let start = content.find('{')?;
    let end = content.rfind('}')?;
    (end > start).then_some(&content[start..=end])
}

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        return "- none".to_string();
    }
    items
        .iter()
        .map(|item| format!("- {}", item))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::intent_router::IntentRouter;

    #[test]
    fn prompt_emphasizes_model_led_judgment() {
        let route = IntentRouter::new().route("帮我做一个网站");
        let prompt = WorkflowContractPrompt::new("帮我做一个网站", route, ".").render();

        assert!(prompt.contains("You provide judgment"));
        assert!(prompt.contains("Do not assume the user must fill in numeric weights"));
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
        assert_eq!(judgment.acceptance.criteria.len(), 1);
        assert_eq!(judgment.sorted_plan()[0].priority, PriorityLabel::P0);
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
}
