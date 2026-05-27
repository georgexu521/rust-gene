

Closeout:
- Status: failed
- Evidence: changed_files=0 validation_passed=0 validation_failed=2 validation_partial=0 validation_not_verified=0 acceptance_passed=0 acceptance_rejected=0 acceptance_pending=5
- Changed: none
- Verified:
  - No required file-change validation was recorded for this workflow
  - Adaptive triggers: risk_signal_high, required_validation
  - verification proof: failed (required validation failed 1/2 commands)
  - verification proof support: failed (supports_verified=false residual_risk=true kinds=none; verification proof status failed blocks verified closeout before proof-kind policy)
  - tool evidence: records=174 completed=24 failed=150 denied=0 validation=0 closeout=0 repair=150 changed=0 workflows=code_change commands=which python3 && python3 --version | ls -la /Users/georgexu/Desktop/rust-agent/target/live-evals/flow-fi...
- Acceptance:
  - pending: Default Python can run core_terminal_demo --self-test
  - pending: .venv/bin/python exists and is executable
  - pending: Installed package passes self-test with 'core-terminal-demo-ok' output
  - pending: required validation command: test -x .venv/bin/python
  - pending: required validation command: . .venv/bin/activate && python -m core_terminal_demo --self-test | rg '^core-terminal-demo-ok$'
- Risk:
  - No changed files were recorded for this code-change workflow
  - Required validation was not run or not recorded
  - Acceptance criteria were generated but not reviewed
  - Workflow finished with unresolved validation or acceptance risk
  - Verification proof is failed: required validation failed 1/2 commands
  - Verification proof support is failed: verification proof status failed blocks verified closeout before proof-kind policy
