import { expect, test } from "@playwright/test";
import {
  acceptRunReview,
  archiveSession,
  completeDesktopOnboarding,
  deleteSession,
  desktopSettings,
  exportDesktopDiagnosticsBundle,
  listRecentSessions,
  newConversation,
  renameSession,
  resetWorkspaceTrust,
  restoreArchivedSession,
  searchSessions,
  selectProject,
  setDetailLevel,
  setWorkspaceTrust,
} from "../src/runtime/desktopApi";

test.describe("desktop API web preview state", () => {
  test("supports session search, rename, archive, delete, and project selection", async () => {
    const initialSessions = await listRecentSessions();
    expect(initialSessions.map((session) => session.id)).toEqual([
      "web-preview",
      "web-preview-release",
    ]);

    await expect(searchSessions("release")).resolves.toEqual([
      expect.objectContaining({
        id: "web-preview-release",
        title: "Release readiness notes",
      }),
    ]);

    await expect(renameSession("web-preview", "Daily desktop flow")).resolves.toEqual(
      expect.objectContaining({
        id: "web-preview",
        title: "Daily desktop flow",
      }),
    );
    await expect(searchSessions("daily")).resolves.toEqual([
      expect.objectContaining({
        id: "web-preview",
        title: "Daily desktop flow",
      }),
    ]);

    const archivedSettings = await archiveSession("web-preview-release");
    expect(archivedSettings.archived_session_ids).toContain("web-preview-release");
    expect((await listRecentSessions()).map((session) => session.id)).toEqual(["web-preview"]);

    const restoredSettings = await restoreArchivedSession("web-preview-release");
    expect(restoredSettings.archived_session_ids).not.toContain("web-preview-release");
    expect((await listRecentSessions()).map((session) => session.id)).toEqual([
      "web-preview-release",
      "web-preview",
    ]);

    const rearchivedSettings = await archiveSession("web-preview-release");
    expect(rearchivedSettings.archived_session_ids).toContain("web-preview-release");
    expect((await listRecentSessions()).map((session) => session.id)).toEqual(["web-preview"]);

    const clearedSettings = await deleteSession("web-preview");
    expect(clearedSettings.active_session_id).toBeNull();
    expect(clearedSettings.archived_session_ids).toContain("web-preview-release");
    await expect(listRecentSessions()).resolves.toEqual([]);

    const selected = await selectProject("/Users/example/projects/phageGPT");
    expect(selected.path).toBe("/Users/example/projects/phageGPT");
    expect(await desktopSettings()).toEqual(
      expect.objectContaining({
        selected_project: "/Users/example/projects/phageGPT",
        active_session_id: null,
        startup_state: expect.objectContaining({
          status: "new_conversation",
          detail: "Ready for a new conversation in phageGPT",
        }),
      }),
    );

    const newConversationSettings = await newConversation();
    expect(newConversationSettings.active_session_id).toBeNull();
    expect(newConversationSettings.startup_state.detail).toContain("phageGPT");

    await expect(setDetailLevel("daily")).resolves.toEqual(
      expect.objectContaining({ detail_level: "daily" }),
    );
    await expect(setDetailLevel("engineering")).resolves.toEqual(
      expect.objectContaining({ detail_level: "engineering" }),
    );
    await expect(setDetailLevel("labrun")).resolves.toEqual(
      expect.objectContaining({ detail_level: "labrun" }),
    );
  });

  test("supports onboarding, workspace trust, and redacted diagnostics export state", async () => {
    const completed = await completeDesktopOnboarding({
      project_root: "/Users/example/projects/priority-agent-demo",
      permission_mode: "auto_low_risk",
      workspace_trust: {
        package_scripts: "trusted",
        shell_validation: "ask",
        lab_daemon_supervision: false,
        developer_auto_acknowledged: false,
      },
      credential_storage_acknowledged: true,
      starting_mode: "direct",
      skipped: false,
    });
    expect(completed.onboarding_state).toEqual(
      expect.objectContaining({
        onboarding_version: 1,
        credential_storage_acknowledged: true,
        skipped: false,
      }),
    );
    expect(completed.workspace_trust.trusted_capabilities).toContain("allow_package_scripts");

    const trusted = await setWorkspaceTrust({
      package_scripts: "trusted",
      shell_validation: "trusted",
      lab_daemon_supervision: true,
      developer_auto_acknowledged: true,
    });
    expect(trusted.workspace_trust.trusted_capabilities).toEqual(
      expect.arrayContaining([
        "allow_package_scripts",
        "allow_shell_validation",
        "allow_lab_daemon_supervision",
        "allow_developer_auto",
      ]),
    );

    const reset = await resetWorkspaceTrust();
    expect(reset.workspace_trust.trusted_capabilities).toEqual([]);
    await expect(exportDesktopDiagnosticsBundle()).resolves.toEqual(
      expect.objectContaining({
        privacy: "redacted",
        redacted: true,
      }),
    );
    const settings = await desktopSettings();
    expect(settings.credential_storage).toEqual(
      expect.objectContaining({
        active_store: "dotenv_fallback",
        preferred_store: "dotenv_fallback",
        activation_mirror: "dotenv_runtime_env",
        migration_available: false,
      }),
    );

    await expect(
      acceptRunReview({
        run_id: "preview-run",
        session_id: "web-preview",
        changed_files: ["/Users/example/projects/priority-agent-demo/src/main.rs"],
        validation_status: "passed",
        permission_summary: "auto low risk",
        residual_risk_count: 0,
        trace_refs: ["trace-preview"],
        tool_output_refs: ["tool-output-preview"],
      }),
    ).resolves.toEqual(
      expect.objectContaining({
        accepted: true,
        run_id: "preview-run",
      }),
    );
  });
});
