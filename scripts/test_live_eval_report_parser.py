import unittest
from textwrap import dedent

from scripts.live_eval_report_parser import (
    memory_proposal_metrics_from_trace,
    normalized_runtime_spine_assertions,
    runtime_spine_metrics_from_events,
)


class MemoryProposalMetricsFromTraceTest(unittest.TestCase):
    def test_uses_memory_proposal_prepared_event_as_primary_signal(self):
        trace_items = [
            {
                "type": "memory_proposal_prepared",
                "status": "prepared",
                "candidates": 2,
                "candidate_kinds": ["preference", "project_fact"],
                "evidence_items": 3,
                "write_policy": "review_only",
                "write_performed": "yes",
            }
        ]

        metrics = memory_proposal_metrics_from_trace(trace_items, signal_text="")

        self.assertEqual(metrics["memory_candidate_typed"], "true")
        self.assertEqual(metrics["memory_candidate_has_evidence"], "true")
        self.assertEqual(metrics["memory_proposal_recorded"], "true")
        self.assertEqual(metrics["memory_proposal_status"], "prepared")
        self.assertEqual(metrics["memory_proposal_candidates"], "2")
        self.assertEqual(metrics["memory_proposal_kinds"], "preference,project_fact")
        self.assertEqual(metrics["memory_proposal_evidence_items"], "3")
        self.assertEqual(metrics["memory_proposal_write_policy"], "review_only")
        self.assertEqual(metrics["memory_proposal_write_performed"], "true")

    def test_zero_candidate_event_keeps_metrics_false(self):
        trace_items = [
            {
                "type": "memory_proposal_prepared",
                "status": "not_applicable",
                "candidates": "0",
                "candidate_kinds": "none",
                "evidence_items": "0",
                "write_policy": "review_only",
                "write_performed": "false",
            }
        ]

        metrics = memory_proposal_metrics_from_trace(trace_items, signal_text="")

        self.assertEqual(metrics["memory_candidate_typed"], "false")
        self.assertEqual(metrics["memory_candidate_has_evidence"], "false")
        self.assertEqual(metrics["memory_proposal_recorded"], "true")
        self.assertEqual(metrics["memory_proposal_status"], "not_applicable")
        self.assertEqual(metrics["memory_proposal_candidates"], "0")
        self.assertEqual(metrics["memory_proposal_kinds"], "none")
        self.assertEqual(metrics["memory_proposal_evidence_items"], "0")
        self.assertEqual(metrics["memory_proposal_write_policy"], "review_only")
        self.assertEqual(metrics["memory_proposal_write_performed"], "false")

    def test_legacy_signal_text_still_backfills_old_metrics(self):
        metrics = memory_proposal_metrics_from_trace(
            [],
            signal_text=(
                "memory_candidate_typed=true "
                "memory_candidate_has_evidence=true "
                "typed memory record"
            ),
        )

        self.assertEqual(metrics["memory_candidate_typed"], "true")
        self.assertEqual(metrics["memory_candidate_has_evidence"], "true")
        self.assertEqual(metrics["memory_proposal_recorded"], "false")


class RuntimeSpineProofAssertionTest(unittest.TestCase):
    def test_subagent_claim_assertions_can_pass_from_report_text_without_trace(self):
        assertions = normalized_runtime_spine_assertions(
            {
                "runtime_spine_assertions": {
                    "verification_proof_kind": "subagent_claim_only",
                    "verification_proof_support_status": "partial",
                    "verification_proof_supports_verified": "false",
                }
            }
        )
        report_text = dedent("""
        verification_proof_status: verified
        verification_proof_kinds: subagent_claim_only
        verification_proof_support_status: partial
        verification_proof_supports_verified: false
        """)

        metrics = runtime_spine_metrics_from_events(
            [], report_text=report_text, assertions=assertions
        )

        self.assertEqual(metrics["runtime_spine_status"], "passed")
        self.assertEqual(metrics["runtime_spine_missing"], "none")

    def test_parent_verified_subagent_assertions_fail_when_only_child_claim_exists(self):
        assertions = normalized_runtime_spine_assertions(
            {
                "runtime_spine_assertions": {
                    "verification_proof_kind": "parent_verified_subagent_result",
                    "verification_proof_support_status": "verified",
                    "verification_proof_supports_verified": "true",
                }
            }
        )
        report_text = dedent("""
        verification_proof_status: verified
        verification_proof_kinds: subagent_claim_only
        verification_proof_support_status: partial
        verification_proof_supports_verified: false
        """)

        metrics = runtime_spine_metrics_from_events(
            [], report_text=report_text, assertions=assertions
        )

        self.assertEqual(metrics["runtime_spine_status"], "failed")
        self.assertIn(
            "verification_proof_kind:parent_verified_subagent_result",
            metrics["runtime_spine_missing"],
        )
        self.assertIn(
            "verification_proof_support_status:verified",
            metrics["runtime_spine_missing"],
        )


class RuntimeSpineRouteRecoveryAssertionTest(unittest.TestCase):
    def test_route_recovery_read_search_assertions_pass_from_trace(self):
        assertions = normalized_runtime_spine_assertions(
            {
                "runtime_spine_assertions": {
                    "route_recovery_plan": True,
                    "route_recovery_read_search_expanded": True,
                    "route_recovery_safety_monotonic": True,
                    "route_recovery_kind": "expand_read_search_only",
                }
            }
        )
        events = [
            {
                "event": "trace_summary",
                "trace": {
                    "events": [
                        {
                            "type": "recovery_plan",
                            "source": "route_recovery",
                            "failure_type": "hidden_read_search_tool_requested",
                            "recovery_kind": "expand_read_search_only",
                            "safe_retry": True,
                            "allowed_alternatives": ["project_list", "grep", "file_read"],
                            "status": "Applied",
                        }
                    ]
                },
            }
        ]

        metrics = runtime_spine_metrics_from_events(events, assertions=assertions)

        self.assertEqual(metrics["runtime_spine_status"], "passed")
        self.assertEqual(metrics["runtime_spine_missing"], "none")
        self.assertEqual(metrics["route_recovery_events"], "1")
        self.assertEqual(metrics["route_recovery_read_search_expanded"], "true")
        self.assertEqual(metrics["route_recovery_mutation_blocked"], "false")
        self.assertEqual(metrics["route_recovery_safety_monotonic"], "true")
        self.assertEqual(metrics["route_recovery_unsafe_mutation_expansion"], "false")

    def test_route_recovery_mutation_block_preserves_safety_monotonicity(self):
        assertions = normalized_runtime_spine_assertions(
            {
                "runtime_spine_assertions": {
                    "route_recovery_plan": True,
                    "route_recovery_mutation_blocked": True,
                    "route_recovery_safety_monotonic": True,
                    "route_recovery_kind": "no_silent_mutation_expansion",
                }
            }
        )
        events = [
            {
                "event": "trace_summary",
                "trace": {
                    "events": [
                        {
                            "type": "recovery_plan",
                            "source": "route_recovery",
                            "failure_type": "hidden_mutation_tool_requested",
                            "recovery_kind": "no_silent_mutation_expansion",
                            "safe_retry": False,
                            "allowed_alternatives": ["project_list", "grep", "file_read"],
                            "requires_user_decision": True,
                            "status": "Planned",
                        }
                    ]
                },
            }
        ]

        metrics = runtime_spine_metrics_from_events(events, assertions=assertions)

        self.assertEqual(metrics["runtime_spine_status"], "passed")
        self.assertEqual(metrics["route_recovery_mutation_blocked"], "true")
        self.assertEqual(metrics["route_recovery_safety_monotonic"], "true")
        self.assertEqual(metrics["route_recovery_unsafe_mutation_expansion"], "false")

    def test_route_recovery_no_diff_replan_is_safety_monotonic(self):
        assertions = normalized_runtime_spine_assertions(
            {
                "runtime_spine_assertions": {
                    "route_recovery_plan": True,
                    "route_recovery_safety_monotonic": True,
                    "route_recovery_kind": "code_change_no_diff_replan",
                }
            }
        )
        events = [
            {
                "event": "trace_summary",
                "trace": {
                    "events": [
                        {
                            "type": "recovery_plan",
                            "source": "route_recovery",
                            "failure_type": "code_change_no_diff_after_repeated_progress",
                            "recovery_kind": "code_change_no_diff_replan",
                            "safe_retry": True,
                            "allowed_alternatives": [
                                "replan_under_code_change_contract",
                                "targeted_lookup_if_missing_anchor",
                                "honest_not_verified_closeout",
                            ],
                            "status": "Applied",
                        }
                    ]
                },
            }
        ]

        metrics = runtime_spine_metrics_from_events(events, assertions=assertions)

        self.assertEqual(metrics["runtime_spine_status"], "passed")
        self.assertEqual(metrics["route_recovery_events"], "1")
        self.assertEqual(
            metrics["route_recovery_failure_types"],
            "code_change_no_diff_after_repeated_progress",
        )
        self.assertEqual(metrics["route_recovery_kinds"], "code_change_no_diff_replan")
        self.assertEqual(metrics["route_recovery_read_search_expanded"], "false")
        self.assertEqual(metrics["route_recovery_mutation_blocked"], "false")
        self.assertEqual(metrics["route_recovery_safety_monotonic"], "true")
        self.assertEqual(metrics["route_recovery_unsafe_mutation_expansion"], "false")

    def test_route_recovery_safety_assertion_fails_on_mutation_alternatives(self):
        assertions = normalized_runtime_spine_assertions(
            {
                "runtime_spine_assertions": {
                    "route_recovery_plan": True,
                    "route_recovery_safety_monotonic": True,
                }
            }
        )
        events = [
            {
                "event": "trace_summary",
                "trace": {
                    "events": [
                        {
                            "type": "recovery_plan",
                            "source": "route_recovery",
                            "failure_type": "hidden_read_search_tool_requested",
                            "recovery_kind": "expand_read_search_only",
                            "safe_retry": True,
                            "allowed_alternatives": ["file_read", "file_edit"],
                            "status": "Applied",
                        }
                    ]
                },
            }
        ]

        metrics = runtime_spine_metrics_from_events(events, assertions=assertions)

        self.assertEqual(metrics["runtime_spine_status"], "failed")
        self.assertEqual(metrics["route_recovery_unsafe_mutation_expansion"], "true")
        self.assertEqual(metrics["route_recovery_safety_monotonic"], "false")
        self.assertIn(
            "special:route_recovery_safety_monotonic",
            metrics["runtime_spine_missing"],
        )


if __name__ == "__main__":
    unittest.main()
