import unittest

from scripts.live_eval_report_parser import memory_proposal_metrics_from_trace


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


if __name__ == "__main__":
    unittest.main()
