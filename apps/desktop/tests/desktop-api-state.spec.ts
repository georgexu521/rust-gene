import { expect, test } from "@playwright/test";
import {
  archiveSession,
  deleteSession,
  desktopSettings,
  listRecentSessions,
  newConversation,
  renameSession,
  searchSessions,
  selectProject,
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

    const clearedSettings = await deleteSession("web-preview");
    expect(clearedSettings.active_session_id).toBeNull();
    expect(clearedSettings.archived_session_ids).toContain("web-preview-release");
    await expect(listRecentSessions()).resolves.toEqual([]);

    const selected = await selectProject("/Users/georgexu/Desktop/phageGPT");
    expect(selected.path).toBe("/Users/georgexu/Desktop/phageGPT");
    expect(await desktopSettings()).toEqual(
      expect.objectContaining({
        selected_project: "/Users/georgexu/Desktop/phageGPT",
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
  });
});
