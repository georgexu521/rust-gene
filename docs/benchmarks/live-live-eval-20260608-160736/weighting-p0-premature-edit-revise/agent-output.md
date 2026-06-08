Let me start by reading the target file to understand its current content.The file contains `name = "before"`. Now I'll replace the value to `"after"`.Task contract: TaskContract id=1ff10c0a-ecc6-4dde-a4ac-5589f8be881c type=CodeChange profile=standard assumptions=3 files=2 validation=4 proof_required=true


Closeout:
- Status: passed
- Evidence: changed_files=1 validation_passed=1 validation_failed=0 validation_partial=0 validation_not_verified=0 acceptance_passed=1 acceptance_rejected=0 acceptance_pending=0
- Changed: fixtures/weighting_p0_premature_edit/config.txt
- Verified:
  - 将 fixtures/weighting_p0_premature_edit/config.txt 中的 name 值改为 "after"，并验证修改结果: passed (required command passed: rg -F 'name = "after"' fixtures/weighting_p0_premature_edit/config.txt)
  - Adaptive triggers: risk_signal_high, required_validation, first_code_change
  - verification proof: verified (required validation passed 1/1 commands)
  - verification proof support: verified (supports_verified=true residual_risk=false kinds=command_passed,required_validation_passed; verified by command_passed,required_validation_passed)
  - tool evidence: records=2 completed=2 failed=0 denied=0 validation=0 closeout=1 repair=1 changed=1 workflows=code_change commands=none
- Acceptance:
  - accepted=true confidence=High unresolved=0
- Risk:
  - none recorded
