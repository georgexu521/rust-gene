I see the issue: `settings.py` has `DEFAULT_TIMEOUT = 3`, but the test expects it to be `10`. Let me make the minimal single-line edit.

Closeout:
- Status: passed
- Evidence: changed_files=1 validation_passed=4 validation_failed=0 validation_partial=0 validation_not_verified=0 acceptance_passed=1 acceptance_rejected=0 acceptance_pending=0
- Changed: fixtures/core_quality/simple_edit/settings.py
- Verified:
  - Read fixtures/core_quality/simple_edit/settings.py to understand current default timeout: passed (required command passed: python3 fixtures/core_quality/simple_edit/test_settings.py)
  - Edit DEFAULT_TIMEOUT value to 10 in settings.py: passed (clean acceptance review completed the remaining plan)
  - Run python3 fixtures/core_quality/simple_edit/test_settings.py: passed (clean acceptance review completed the remaining plan)
  - Run rg 'DEFAULT_TIMEOUT = 10' fixtures/core_quality/simple_edit/settings.py: passed (clean acceptance review completed the remaining plan)
  - Adaptive triggers: risk_signal_high, required_validation, first_code_change
  - verification proof: verified (required validation passed 2/2 commands)
  - verification proof support: verified (supports_verified=true residual_risk=false kinds=command_passed,required_validation_passed; verified by command_passed,required_validation_passed)
  - tool evidence: records=3 completed=3 failed=0 denied=0 validation=0 closeout=1 repair=1 changed=1 workflows=code_change commands=none
- Acceptance:
  - accepted=true confidence=High unresolved=0
- Risk:
  - none recorded
