## Observed Evidence

**Directory:** `fixtures/mva_light_inspection`

| Entry | Type | Size | Notes |
|-------|------|------|-------|
| `.hidden` | hidden file (regular) | 7 bytes | prefixed with `.` |
| `a.txt` | regular file | 6 bytes | — |
| `b.log` | regular file | 5 bytes | — |
| `notes/` | subdirectory | 64 bytes | — |

**Summary:**
- **2 regular files:** `a.txt`, `b.log`
- **1 hidden file:** `.hidden` (starts with `.`)
- **1 subdirectory:** `notes/`
- **0 hidden subdirectories** detected

---

**Closeout:**
- No files changed.
- No validation command required (no required validation for this eval).
- Bounded read-only inspection completed with `ls -la`.