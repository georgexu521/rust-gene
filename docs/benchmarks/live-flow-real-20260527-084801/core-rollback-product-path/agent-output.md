

Closeout:
- Status: not_verified
- Evidence: changed_files=0 validation_passed=0 validation_failed=0 validation_partial=0 validation_not_verified=0 acceptance_passed=0 acceptance_rejected=0 acceptance_pending=5
- Changed: none
- Verified:
  - No required file-change validation was recorded for this workflow
  - Adaptive triggers: risk_signal_high, required_validation
  - verification proof: not_run (required validation missing 2/2 commands)
  - verification proof support: not_run (supports_verified=false residual_risk=true kinds=none; verification proof status not_run blocks verified closeout before proof-kind policy)
  - tool evidence: records=144 completed=21 failed=123 denied=0 validation=8 closeout=8 repair=123 changed=0 workflows=code_change commands=cd /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-real-20260527-084801/core-rollback-product-...
- Acceptance:
  - pending: Rollback tests pass
  - pending: Checkpoint tests pass
  - pending: Rollback covers regular file modification path
  - pending: required validation command: cargo test -q rollback -- --test-threads=1
  - pending: required validation command: cargo test -q checkpoint -- --test-threads=1
- Risk:
  - No changed files were recorded for this code-change workflow
  - Required validation was not run or not recorded
  - Acceptance criteria were generated but not reviewed
  - Workflow finished with unresolved validation or acceptance risk
  - Verification proof is not_run: required validation missing 2/2 commands
  - Verification proof support is not_run: verification proof status not_run blocks verified closeout before proof-kind policy
