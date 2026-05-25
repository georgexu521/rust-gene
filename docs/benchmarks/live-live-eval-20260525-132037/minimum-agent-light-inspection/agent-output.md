## Observed Evidence

From the `ls -la` output of `fixtures/mva_light_inspection`:

| Entry | Type | Size | Name |
|-------|------|------|------|
| -rw-r--r-- | Regular file | 6 bytes | `a.txt` |
| -rw-r--r-- | Regular file | 5 bytes | `b.log` |
| drwxr-xr-x | **Subdirectory** | — | `notes` |
| -rw-r--r-- | Hidden file (starts with `.`) | 7 bytes | `.hidden` |

---

## Summary

The directory contains **4 observable items** (excluding `.` and `..`):

1. **Regular files (2):**
   - `a.txt` — 6 bytes
   - `b.log` — 5 bytes

2. **Subdirectory (1):**
   - `notes/` — directory

3. **Hidden file (1):**
   - `.hidden` — 7 bytes

---

## No Changes

- No files were read, written, edited, or modified.
- No validation command was required for this read-only inspection task.

**Closeout:** Read-only inspection completed. Evidence grounded in the `ls -la` output above. No changes made to the filesystem.