# Security Release Checklist
Status: Current

Use this checklist before tagging a formal Priority Agent release.

## Required Gates

- [ ] `cargo fmt --check`
- [ ] `git diff --check`
- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features -- --test-threads=1`
- [ ] `bash scripts/validate_docs.sh`
- [ ] `cargo doc --workspace --all-features --no-deps`
- [ ] `bash scripts/security_dependency_audit.sh`

## Security Review

- [ ] Review `SECURITY.md` for current reporting instructions and known limits.
- [ ] Review `docs/THREAT_MODEL.md` for changed trust boundaries.
- [ ] Review CodeQL results on the release commit.
- [ ] Review dependency updates manually before release.
- [ ] Review `cargo audit` and `cargo deny check` output from
      `scripts/security_dependency_audit.sh`.
- [ ] Confirm no provider keys, bearer tokens, dotenv files, private keys, or
      local LabRun artifacts are staged.
- [ ] Run targeted LabRun security tests:

```bash
cargo test -q lab::audit_redaction --lib -- --test-threads=1
cargo test -q lab::validation --lib -- --test-threads=1
cargo test -q lab::orchestrator --lib -- --test-threads=1
cargo test -q action_review --lib -- --test-threads=1
```

## Release Artifact Review

- [ ] Build release artifacts from the exact release commit.
- [ ] Smoke test the packaged binary.
- [ ] Generate and verify checksums.
- [ ] Record whether SBOM generation, artifact signing, and provenance
      attestation were completed or intentionally deferred.
- [ ] Record any skipped gates or platform limitations in release notes.

## Future Hardening

- [ ] Add release signing and provenance attestations.
- [ ] Add SBOM generation.
- [ ] Add automated secret scanning if GitHub Advanced Security or an approved
      open-source scanner is available.
