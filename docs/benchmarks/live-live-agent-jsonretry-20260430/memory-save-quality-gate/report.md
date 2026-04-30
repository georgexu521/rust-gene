# Live Eval Report: memory-save-quality-gate

- Run id: `live-agent-jsonretry-20260430`
- Sample: `evalsets/live_tasks/memory-save-quality-gate.yaml`
- Worktree: `target/live-evals/live-agent-jsonretry-20260430/memory-save-quality-gate/worktree`
- Isolated env: `/Users/georgexu/Desktop/rust-agent/target/live-evals/live-agent-jsonretry-20260430/memory-save-quality-gate/env`
- Test status: `failed`
- Generated: `2026-04-30 10:23:48 +0800`

## Git Status

```text
 M src/memory/quality.rs
```

## Diff Stat

```text
 src/memory/quality.rs | 95 ++++++++++++++++++++++++++++++++++++++-------------
 1 file changed, 72 insertions(+), 23 deletions(-)
```

## Required Commands

```text
$ cargo test -q memory -- --test-threads=1
error: visibility `pub` is not followed by an item
  --> src/memory/quality.rs:69:5
   |
69 |     pub reason: String,
   |     ^^^ the visibility
   |
   = help: you likely meant to define an item, e.g., `pub fn foo() {}`

error: expected identifier, found `:`
  --> src/memory/quality.rs:69:15
   |
69 |     pub reason: String,
   |               ^ expected identifier

error[E0428]: the name `assess_memory_candidate` is defined multiple times
   --> src/memory/quality.rs:78:1
    |
  1 | / pub fn assess_memory_candidate(
  2 | |     content: &str,
  3 | |     category: &str,
  4 | |     existing_content: &str,
...   |
 69 | |     pub reason: String,
 70 | | }
    | |_- previous definition of the value `assess_memory_candidate` here
...
 78 | / pub fn assess_memory_candidate(
 79 | |     content: &str,
 80 | |     category: &str,
 81 | |     existing_content: &str,
...   |
249 | |     })
250 | | }
    | |_^ `assess_memory_candidate` redefined here
    |
    = note: `assess_memory_candidate` must be defined only once in the value namespace of this module

error[E0432]: unresolved import `quality::MemoryQualityAssessment`
  --> src/memory/mod.rs:18:44
   |
18 | pub use quality::{assess_memory_candidate, MemoryQualityAssessment};
   |                                            ^^^^^^^^^^^^^^^^^^^^^^^ no `MemoryQualityAssessment` in `memory::quality`

error[E0425]: cannot find type `MemoryQualityAssessment` in this scope
 --> src/memory/quality.rs:6:13
  |
6 | ) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
  |             ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
  |
help: you might be missing a type parameter
  |
1 | pub fn assess_memory_candidate<MemoryQualityAssessment>(
  |                               +++++++++++++++++++++++++

error[E0425]: cannot find type `MemorySafetyIssue` in this scope
 --> src/memory/quality.rs:6:38
  |
6 | ) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
  |                                      ^^^^^^^^^^^^^^^^^ not found in this scope
  |
help: consider importing this struct through its public re-export
  |
1 + use crate::memory::MemorySafetyIssue;
  |

error[E0425]: cannot find function `scan_memory_content` in this scope
 --> src/memory/quality.rs:7:23
  |
7 |     let sensitivity = scan_memory_content(content)?;
  |                       ^^^^^^^^^^^^^^^^^^^ not found in this scope
  |
help: consider importing this function through its public re-export
  |
1 + use crate::memory::scan_memory_content;
  |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
 --> src/memory/quality.rs:8:16
  |
8 |     let kind = MemoryKind::from_category(category, content);
  |                ^^^^^^^^^^ use of undeclared type `MemoryKind`
  |
help: consider importing this enum through its public re-export
  |
1 + use crate::memory::MemoryKind;
  |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:13:9
   |
13 |         MemoryKind::UserPreference
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:14:11
   |
14 |         | MemoryKind::WorkflowConvention
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:15:11
   |
15 |         | MemoryKind::Decision
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:16:11
   |
16 |         | MemoryKind::ProjectFact => 0.85,
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:17:9
   |
17 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:17:38
   |
17 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |                                      ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:17:66
   |
17 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |                                                                  ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:18:9
   |
18 |         MemoryKind::SkillCandidate => 0.8,
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:19:9
   |
19 |         MemoryKind::Note => {
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:30:9
   |
30 |         MemoryKind::UserPreference
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:31:15
   |
31 |             | MemoryKind::ProjectFact
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:32:15
   |
32 |             | MemoryKind::WorkflowConvention
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:33:15
   |
33 |             | MemoryKind::Decision
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:34:15
   |
34 |             | MemoryKind::FailurePattern
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:35:15
   |
35 |             | MemoryKind::SuccessfulFix
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:36:15
   |
36 |             | MemoryKind::ToolQuirk
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:37:15
   |
37 |             | MemoryKind::SkillCandidate
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0425]: cannot find type `MemoryQualityAssessment` in this scope
  --> src/memory/quality.rs:72:6
   |
72 | impl MemoryQualityAssessment {
   |      ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
  --> src/memory/quality.rs:74:24
   |
74 |         self.status == MemoryStatus::Accepted
   |                        ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryStatus;
   |

error[E0425]: cannot find type `MemoryQualityAssessment` in this scope
  --> src/memory/quality.rs:83:13
   |
83 | ) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
   |             ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
   |
help: you might be missing a type parameter
   |
78 | pub fn assess_memory_candidate<MemoryQualityAssessment>(
   |                               +++++++++++++++++++++++++

error[E0425]: cannot find type `MemorySafetyIssue` in this scope
  --> src/memory/quality.rs:83:38
   |
83 | ) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
   |                                      ^^^^^^^^^^^^^^^^^ not found in this scope
   |
help: consider importing this struct through its public re-export
   |
 1 + use crate::memory::MemorySafetyIssue;
   |

error[E0425]: cannot find function `scan_memory_content` in this scope
  --> src/memory/quality.rs:84:23
   |
84 |     let sensitivity = scan_memory_content(content)?;
   |                       ^^^^^^^^^^^^^^^^^^^ not found in this scope
   |
help: consider importing this function through its public re-export
   |
 1 + use crate::memory::scan_memory_content;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:85:16
   |
85 |     let kind = MemoryKind::from_category(category, content);
   |                ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:90:9
   |
90 |         MemoryKind::UserPreference
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:91:11
   |
91 |         | MemoryKind::WorkflowConvention
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:92:11
   |
92 |         | MemoryKind::Decision
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:93:11
   |
93 |         | MemoryKind::ProjectFact => 0.85,
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:94:9
   |
94 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:94:38
   |
94 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |                                      ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:94:66
   |
94 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |                                                                  ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:95:9
   |
95 |         MemoryKind::SkillCandidate => 0.8,
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:96:9
   |
96 |         MemoryKind::Note => {
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:107:9
    |
107 |         MemoryKind::UserPreference
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:108:15
    |
108 |             | MemoryKind::ProjectFact
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:109:15
    |
109 |             | MemoryKind::WorkflowConvention
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:110:15
    |
110 |             | MemoryKind::Decision
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:111:15
    |
111 |             | MemoryKind::FailurePattern
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:112:15
    |
112 |             | MemoryKind::SuccessfulFix
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:113:15
    |
113 |             | MemoryKind::ToolQuirk
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:114:15
    |
114 |             | MemoryKind::SkillCandidate
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:159:9
    |
159 |         MemoryKind::UserPreference
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:160:15
    |
160 |             | MemoryKind::ProjectFact
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:161:15
    |
161 |             | MemoryKind::WorkflowConvention
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:162:15
    |
162 |             | MemoryKind::Decision
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:163:15
    |
163 |             | MemoryKind::FailurePattern
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:164:15
    |
164 |             | MemoryKind::SuccessfulFix
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:165:15
    |
165 |             | MemoryKind::ToolQuirk
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:166:15
    |
166 |             | MemoryKind::SkillCandidate
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:174:9
    |
174 |         MemoryKind::UserPreference | MemoryKind::ProjectFact | MemoryKind::WorkflowConvention => {
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:174:38
    |
174 |         MemoryKind::UserPreference | MemoryKind::ProjectFact | MemoryKind::WorkflowConvention => {
    |                                      ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:174:64
    |
174 |         MemoryKind::UserPreference | MemoryKind::ProjectFact | MemoryKind::WorkflowConvention => {
    |                                                                ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:177:9
    |
177 |         MemoryKind::Decision | MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => 0.8,
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:177:32
    |
177 |         MemoryKind::Decision | MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => 0.8,
    |                                ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:177:61
    |
177 |         MemoryKind::Decision | MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => 0.8,
    |                                                             ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:178:9
    |
178 |         MemoryKind::ToolQuirk | MemoryKind::SkillCandidate => 0.75,
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:178:33
    |
178 |         MemoryKind::ToolQuirk | MemoryKind::SkillCandidate => 0.75,
    |                                 ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:179:9
    |
179 |         MemoryKind::Note => {
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:208:9
    |
208 |         SensitivityLevel::Public => 0.0,
    |         ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::SensitivityLevel;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:209:9
    |
209 |         SensitivityLevel::LocalOnly => 0.15,
    |         ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::SensitivityLevel;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:210:9
    |
210 |         SensitivityLevel::SecretLike => 0.85,
    |         ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::SensitivityLevel;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:211:9
    |
211 |         SensitivityLevel::Unsafe => 1.0,
    |         ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::SensitivityLevel;
    |

error[E0425]: cannot find function `memory_write_factors_from_signals` in this scope
   --> src/memory/quality.rs:215:25
    |
215 |     let write_factors = memory_write_factors_from_signals(
    |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
    |
help: consider importing this function through its public re-export
    |
  1 + use crate::memory::memory_write_factors_from_signals;
    |

error[E0425]: cannot find value `score` in this scope
   --> src/memory/quality.rs:226:17
    |
226 | let status = if score >= 0.65 {
    |                 ^^^^^ not found in this scope

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:227:9
    |
227 |         MemoryStatus::Accepted
    |         ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryStatus;
    |

error[E0425]: cannot find value `write_decision` in this scope
   --> src/memory/quality.rs:229:9
    |
229 |         write_decision.status
    |         ^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find value `write_decision` in this scope
   --> src/memory/quality.rs:233:9
    |
233 |         write_decision.reason
    |         ^^^^^^^^^^^^^^ not found in this scope

error[E0422]: cannot find struct, variant or union type `MemoryQualityAssessment` in this scope
   --> src/memory/quality.rs:236:8
    |
236 |     Ok(MemoryQualityAssessment {
    |        ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find value `score` in this scope
   --> src/memory/quality.rs:245:9
    |
245 |         score,
    |         ^^^^^ not found in this scope

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:298:39
    |
298 |         assert_eq!(assessment.status, MemoryStatus::Accepted);
    |                                       ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:308:13
    |
308 |             MemoryStatus::Proposed | MemoryStatus::Rejected
    |             ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:308:38
    |
308 |             MemoryStatus::Proposed | MemoryStatus::Rejected
    |                                      ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:316:39
    |
316 |         assert_ne!(assessment.status, MemoryStatus::Accepted);
    |                                       ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:325:39
    |
325 |         assert_ne!(assessment.status, MemoryStatus::Accepted);
    |                                       ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:338:37
    |
338 |         assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    |                                     ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::SensitivityLevel;
    |

Some errors have detailed explanations: E0422, E0425, E0428, E0432, E0433.
For more information about an error, try `rustc --explain E0422`.
error: could not compile `priority-agent` (bin "priority-agent" test) due to 82 previous errors
[exit status: 101]

$ cargo test -q -- --test-threads=1
error: visibility `pub` is not followed by an item
  --> src/memory/quality.rs:69:5
   |
69 |     pub reason: String,
   |     ^^^ the visibility
   |
   = help: you likely meant to define an item, e.g., `pub fn foo() {}`

error: expected identifier, found `:`
  --> src/memory/quality.rs:69:15
   |
69 |     pub reason: String,
   |               ^ expected identifier

error[E0428]: the name `assess_memory_candidate` is defined multiple times
   --> src/memory/quality.rs:78:1
    |
  1 | / pub fn assess_memory_candidate(
  2 | |     content: &str,
  3 | |     category: &str,
  4 | |     existing_content: &str,
...   |
 69 | |     pub reason: String,
 70 | | }
    | |_- previous definition of the value `assess_memory_candidate` here
...
 78 | / pub fn assess_memory_candidate(
 79 | |     content: &str,
 80 | |     category: &str,
 81 | |     existing_content: &str,
...   |
249 | |     })
250 | | }
    | |_^ `assess_memory_candidate` redefined here
    |
    = note: `assess_memory_candidate` must be defined only once in the value namespace of this module

error[E0432]: unresolved import `quality::MemoryQualityAssessment`
  --> src/memory/mod.rs:18:44
   |
18 | pub use quality::{assess_memory_candidate, MemoryQualityAssessment};
   |                                            ^^^^^^^^^^^^^^^^^^^^^^^ no `MemoryQualityAssessment` in `memory::quality`

error[E0425]: cannot find type `MemoryQualityAssessment` in this scope
 --> src/memory/quality.rs:6:13
  |
6 | ) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
  |             ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
  |
help: you might be missing a type parameter
  |
1 | pub fn assess_memory_candidate<MemoryQualityAssessment>(
  |                               +++++++++++++++++++++++++

error[E0425]: cannot find type `MemorySafetyIssue` in this scope
 --> src/memory/quality.rs:6:38
  |
6 | ) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
  |                                      ^^^^^^^^^^^^^^^^^ not found in this scope
  |
help: consider importing this struct through its public re-export
  |
1 + use crate::memory::MemorySafetyIssue;
  |

error[E0425]: cannot find function `scan_memory_content` in this scope
 --> src/memory/quality.rs:7:23
  |
7 |     let sensitivity = scan_memory_content(content)?;
  |                       ^^^^^^^^^^^^^^^^^^^ not found in this scope
  |
help: consider importing this function through its public re-export
  |
1 + use crate::memory::scan_memory_content;
  |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
 --> src/memory/quality.rs:8:16
  |
8 |     let kind = MemoryKind::from_category(category, content);
  |                ^^^^^^^^^^ use of undeclared type `MemoryKind`
  |
help: consider importing this enum through its public re-export
  |
1 + use crate::memory::MemoryKind;
  |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:13:9
   |
13 |         MemoryKind::UserPreference
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:14:11
   |
14 |         | MemoryKind::WorkflowConvention
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:15:11
   |
15 |         | MemoryKind::Decision
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:16:11
   |
16 |         | MemoryKind::ProjectFact => 0.85,
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:17:9
   |
17 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:17:38
   |
17 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |                                      ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:17:66
   |
17 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |                                                                  ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:18:9
   |
18 |         MemoryKind::SkillCandidate => 0.8,
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:19:9
   |
19 |         MemoryKind::Note => {
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:30:9
   |
30 |         MemoryKind::UserPreference
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:31:15
   |
31 |             | MemoryKind::ProjectFact
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:32:15
   |
32 |             | MemoryKind::WorkflowConvention
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:33:15
   |
33 |             | MemoryKind::Decision
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:34:15
   |
34 |             | MemoryKind::FailurePattern
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:35:15
   |
35 |             | MemoryKind::SuccessfulFix
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:36:15
   |
36 |             | MemoryKind::ToolQuirk
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:37:15
   |
37 |             | MemoryKind::SkillCandidate
   |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0425]: cannot find type `MemoryQualityAssessment` in this scope
  --> src/memory/quality.rs:72:6
   |
72 | impl MemoryQualityAssessment {
   |      ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
  --> src/memory/quality.rs:74:24
   |
74 |         self.status == MemoryStatus::Accepted
   |                        ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryStatus;
   |

error[E0425]: cannot find type `MemoryQualityAssessment` in this scope
  --> src/memory/quality.rs:83:13
   |
83 | ) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
   |             ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
   |
help: you might be missing a type parameter
   |
78 | pub fn assess_memory_candidate<MemoryQualityAssessment>(
   |                               +++++++++++++++++++++++++

error[E0425]: cannot find type `MemorySafetyIssue` in this scope
  --> src/memory/quality.rs:83:38
   |
83 | ) -> Result<MemoryQualityAssessment, MemorySafetyIssue> {
   |                                      ^^^^^^^^^^^^^^^^^ not found in this scope
   |
help: consider importing this struct through its public re-export
   |
 1 + use crate::memory::MemorySafetyIssue;
   |

error[E0425]: cannot find function `scan_memory_content` in this scope
  --> src/memory/quality.rs:84:23
   |
84 |     let sensitivity = scan_memory_content(content)?;
   |                       ^^^^^^^^^^^^^^^^^^^ not found in this scope
   |
help: consider importing this function through its public re-export
   |
 1 + use crate::memory::scan_memory_content;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:85:16
   |
85 |     let kind = MemoryKind::from_category(category, content);
   |                ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:90:9
   |
90 |         MemoryKind::UserPreference
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:91:11
   |
91 |         | MemoryKind::WorkflowConvention
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:92:11
   |
92 |         | MemoryKind::Decision
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:93:11
   |
93 |         | MemoryKind::ProjectFact => 0.85,
   |           ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:94:9
   |
94 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:94:38
   |
94 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |                                      ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:94:66
   |
94 |         MemoryKind::FailurePattern | MemoryKind::SuccessfulFix | MemoryKind::ToolQuirk => 0.75,
   |                                                                  ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:95:9
   |
95 |         MemoryKind::SkillCandidate => 0.8,
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
  --> src/memory/quality.rs:96:9
   |
96 |         MemoryKind::Note => {
   |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
   |
help: consider importing this enum through its public re-export
   |
 1 + use crate::memory::MemoryKind;
   |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:107:9
    |
107 |         MemoryKind::UserPreference
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:108:15
    |
108 |             | MemoryKind::ProjectFact
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:109:15
    |
109 |             | MemoryKind::WorkflowConvention
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:110:15
    |
110 |             | MemoryKind::Decision
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:111:15
    |
111 |             | MemoryKind::FailurePattern
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:112:15
    |
112 |             | MemoryKind::SuccessfulFix
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:113:15
    |
113 |             | MemoryKind::ToolQuirk
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:114:15
    |
114 |             | MemoryKind::SkillCandidate
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:159:9
    |
159 |         MemoryKind::UserPreference
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:160:15
    |
160 |             | MemoryKind::ProjectFact
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:161:15
    |
161 |             | MemoryKind::WorkflowConvention
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:162:15
    |
162 |             | MemoryKind::Decision
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:163:15
    |
163 |             | MemoryKind::FailurePattern
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:164:15
    |
164 |             | MemoryKind::SuccessfulFix
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:165:15
    |
165 |             | MemoryKind::ToolQuirk
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:166:15
    |
166 |             | MemoryKind::SkillCandidate
    |               ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:174:9
    |
174 |         MemoryKind::UserPreference | MemoryKind::ProjectFact | MemoryKind::WorkflowConvention => {
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:174:38
    |
174 |         MemoryKind::UserPreference | MemoryKind::ProjectFact | MemoryKind::WorkflowConvention => {
    |                                      ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:174:64
    |
174 |         MemoryKind::UserPreference | MemoryKind::ProjectFact | MemoryKind::WorkflowConvention => {
    |                                                                ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:177:9
    |
177 |         MemoryKind::Decision | MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => 0.8,
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:177:32
    |
177 |         MemoryKind::Decision | MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => 0.8,
    |                                ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:177:61
    |
177 |         MemoryKind::Decision | MemoryKind::FailurePattern | MemoryKind::SuccessfulFix => 0.8,
    |                                                             ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:178:9
    |
178 |         MemoryKind::ToolQuirk | MemoryKind::SkillCandidate => 0.75,
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:178:33
    |
178 |         MemoryKind::ToolQuirk | MemoryKind::SkillCandidate => 0.75,
    |                                 ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryKind`
   --> src/memory/quality.rs:179:9
    |
179 |         MemoryKind::Note => {
    |         ^^^^^^^^^^ use of undeclared type `MemoryKind`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryKind;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:208:9
    |
208 |         SensitivityLevel::Public => 0.0,
    |         ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::SensitivityLevel;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:209:9
    |
209 |         SensitivityLevel::LocalOnly => 0.15,
    |         ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::SensitivityLevel;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:210:9
    |
210 |         SensitivityLevel::SecretLike => 0.85,
    |         ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::SensitivityLevel;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:211:9
    |
211 |         SensitivityLevel::Unsafe => 1.0,
    |         ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::SensitivityLevel;
    |

error[E0425]: cannot find function `memory_write_factors_from_signals` in this scope
   --> src/memory/quality.rs:215:25
    |
215 |     let write_factors = memory_write_factors_from_signals(
    |                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
    |
help: consider importing this function through its public re-export
    |
  1 + use crate::memory::memory_write_factors_from_signals;
    |

error[E0425]: cannot find value `score` in this scope
   --> src/memory/quality.rs:226:17
    |
226 | let status = if score >= 0.65 {
    |                 ^^^^^ not found in this scope

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:227:9
    |
227 |         MemoryStatus::Accepted
    |         ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
  1 + use crate::memory::MemoryStatus;
    |

error[E0425]: cannot find value `write_decision` in this scope
   --> src/memory/quality.rs:229:9
    |
229 |         write_decision.status
    |         ^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find value `write_decision` in this scope
   --> src/memory/quality.rs:233:9
    |
233 |         write_decision.reason
    |         ^^^^^^^^^^^^^^ not found in this scope

error[E0422]: cannot find struct, variant or union type `MemoryQualityAssessment` in this scope
   --> src/memory/quality.rs:236:8
    |
236 |     Ok(MemoryQualityAssessment {
    |        ^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find value `score` in this scope
   --> src/memory/quality.rs:245:9
    |
245 |         score,
    |         ^^^^^ not found in this scope

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:298:39
    |
298 |         assert_eq!(assessment.status, MemoryStatus::Accepted);
    |                                       ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:308:13
    |
308 |             MemoryStatus::Proposed | MemoryStatus::Rejected
    |             ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:308:38
    |
308 |             MemoryStatus::Proposed | MemoryStatus::Rejected
    |                                      ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:316:39
    |
316 |         assert_ne!(assessment.status, MemoryStatus::Accepted);
    |                                       ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `MemoryStatus`
   --> src/memory/quality.rs:325:39
    |
325 |         assert_ne!(assessment.status, MemoryStatus::Accepted);
    |                                       ^^^^^^^^^^^^ use of undeclared type `MemoryStatus`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::MemoryStatus;
    |

error[E0433]: failed to resolve: use of undeclared type `SensitivityLevel`
   --> src/memory/quality.rs:338:37
    |
338 |         assert_eq!(err.sensitivity, SensitivityLevel::SecretLike);
    |                                     ^^^^^^^^^^^^^^^^ use of undeclared type `SensitivityLevel`
    |
help: consider importing this enum through its public re-export
    |
287 +     use crate::memory::SensitivityLevel;
    |

Some errors have detailed explanations: E0422, E0425, E0428, E0432, E0433.
For more information about an error, try `rustc --explain E0422`.
error: could not compile `priority-agent` (bin "priority-agent" test) due to 82 previous errors
[exit status: 101]

```

## Agent Run

- Exit status: `0`
- Output: `docs/benchmarks/live-live-agent-jsonretry-20260430/memory-save-quality-gate/agent-output.md`
- Events: `docs/benchmarks/live-live-agent-jsonretry-20260430/memory-save-quality-gate/agent-events.jsonl`

Event counts:

```text
complete: 1
eval_started: 1
start: 1
text_chunk: 1
tool_execution_complete: 6
tool_execution_progress: 2
tool_execution_start: 6
trace_summary: 1
```

Quality signals:

```text
output_chars: 1766
diff_chars: 3490
tool_executions: 6
tool_errors: 0
tool_failures: 4
has_closeout: true
has_validation_claim: true
trace_status: Completed
trace_events: 84
test_status: failed
verification_passed: false
stage_validation_passed: false
acceptance_accepted: False
closeout_status: failed
trace_event_types: api.start,workflow.fallback,api.done,tool.start,tool.done,verify.done,reflection.pass,stage.validation,guided.debug,acceptance.review,closeout,assistant
warning: required_commands_not_passing
warning: closeout_not_successful
warning: acceptance_review_rejected
warning: stage_validation_failed
warning: verification_failed
```

Agent stderr tail:

```text
2026-04-30T02:21:41.729549Z  WARN priority_agent::tools::file_tool: File 'src/memory/quality.rs' was modified since it was read
```

## Human Review

- accepted: TODO
- task_success: TODO
- mainline_hit: TODO
- plan_coverage: TODO
- rework_count: TODO
- tool_efficiency: TODO
- diff_discipline: TODO
- closeout_accuracy: TODO
- notes: TODO
