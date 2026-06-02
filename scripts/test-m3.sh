#!/bin/bash
export MINIMAX_MODEL=MiniMax-M3
PROMPT_FILE="${1:-tests/fixtures/test-basic-read.txt}"
exec cargo run -- --eval-run --prompt-file "$PROMPT_FILE"
