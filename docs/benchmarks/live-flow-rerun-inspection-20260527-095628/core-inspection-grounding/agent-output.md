
[Stopped noisy retries after repeated failures: bash]


Closeout:
- Status: failed
- Evidence: changed_files=0 validation_passed=0 validation_failed=2 validation_partial=0 validation_not_verified=0 acceptance_passed=0 acceptance_rejected=0 acceptance_pending=8
- Changed: none
- Verified:
  - No required file-change validation was recorded for this workflow
  - Adaptive triggers: risk_signal_high, required_validation
  - verification proof: failed (required validation failed 4/4 commands)
  - verification proof support: failed (supports_verified=false residual_risk=true kinds=none; verification proof status failed blocks verified closeout before proof-kind policy)
  - tool evidence: records=5 completed=1 failed=4 denied=0 validation=4 closeout=4 repair=4 changed=0 workflows=code_change commands=ls -la fixtures/core_quality/inspection_target/gex | test -d fixtures/core_quality/inspection_target/gex && ech...
- Acceptance:
  - pending: Directory fixtures/core_quality/inspection_target/gex exists
  - pending: File a.txt exists in gex directory
  - pending: Hidden file .hidden exists in gex directory
  - pending: Subdirectory notes exists in gex directory
  - pending: required validation command: test -d fixtures/core_quality/inspection_target/gex
  - pending: required validation command: test -f fixtures/core_quality/inspection_target/gex/a.txt
  - pending: required validation command: test -f fixtures/core_quality/inspection_target/gex/.hidden
  - pending: required validation command: test -d fixtures/core_quality/inspection_target/gex/notes
- Risk:
  - No changed files were recorded for this code-change workflow
  - Required validation was not run or not recorded
  - Acceptance criteria were generated but not reviewed
  - Workflow finished with unresolved validation or acceptance risk
  - Verification proof is failed: required validation failed 4/4 commands
  - Verification proof support is failed: verification proof status failed blocks verified closeout before proof-kind policy
