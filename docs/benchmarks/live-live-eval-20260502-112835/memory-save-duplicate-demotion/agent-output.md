

Closeout:
- Status: passed
- Changed: src/memory/quality.rs, src/memory/manager.rs
- Verified:
  - Fix duplicate memory candidate detection and demotion logic: passed
  - Explore memory save implementation to understand current duplicate detection: passed
  - Run cargo test to validate the fix: passed
  - Add tests for duplicate, near-duplicate, and cross-scope candidates: passed
- Acceptance:
  - accepted=false confidence=High unresolved=1
  - accepted=true confidence=High unresolved=0
- Risk:
  - none recorded
