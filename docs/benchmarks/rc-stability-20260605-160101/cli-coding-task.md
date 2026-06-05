# Lane C1: CLI Coding Smoke — read, edit, validate

**Fixture:** .rc-fixture/src/main.rs (add function with subtraction bug)
**Model:** MiniMax
**Command:** `PRIORITY_AGENT_ROUTE_SCOPED_TOOLS=0 cargo run -- --eval-run --prompt-file /tmp/rc-prompt.txt --output /tmp/rc-c1-output.txt`

## Result: PASSED

- **Bug identified**: Agent correctly found `a - b` on line 2
- **Edit applied**: file_edit changed `-` to `+`
- **Diff verified**: correct one-character change
- **Test result**: 1 passed, 0 failed
- **Mutation result**: produced via file_edit
- **Closeout**: verified
## Lane C2: Stale anchor recovery — PASSED
Edit correct, test passes, closeout=not_verified (acceptance pending — correct behavior)
## Lane C3: Todo continuity
Steps 1-2 completed (fix add, add subtract). Todo_write used. Missing subtract test (iteration limit).
