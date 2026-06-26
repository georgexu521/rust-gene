import { expect, test, type Page } from "@playwright/test";

test.describe("desktop UI smoke", () => {
  test("first-run onboarding can be skipped and completed preview starts cleanly", async ({ page }) => {
    await page.goto("/?previewFixture=onboarding");

    const wizard = page.getByRole("dialog", { name: "First-run setup" });
    await expect(wizard).toBeVisible();
    await expect(wizard).toContainText("Desktop setup");
    await wizard.getByRole("button", { name: "Skip setup" }).click();
    await expect(wizard).not.toBeVisible();

    await page.evaluate(() => window.sessionStorage.removeItem("priority-agent.onboardingFixtureCompleted"));
    await page.goto("/?previewFixture=onboarding");
    const freshWizard = page.getByRole("dialog", { name: "First-run setup" });
    await expect(freshWizard).toBeVisible();
    await freshWizard.getByRole("button", { name: /Credentials/ }).click();
    await freshWizard.getByLabel(/local dotenv fallback/).check();
    await freshWizard.getByRole("button", { name: /Trust/ }).click();
    await freshWizard.getByRole("group", { name: "Package-script validation" }).getByRole("button", { name: "Trusted" }).click();
    await freshWizard.locator(".onboarding-stepper").getByRole("button", { name: /Start/ }).click();
    await freshWizard.locator(".onboarding-actions").getByRole("button", { name: "Start" }).click();
    await expect(freshWizard).not.toBeVisible();

    await page.goto("/?previewFixture=1");
    await expect(page.getByRole("dialog", { name: "First-run setup" })).not.toBeVisible();
  });

  test("project path edits stay draft-only until applied", async ({ page }) => {
    await page.goto("/?previewFixture=1");

    const composer = page.locator(".composer");
    await expect(page.locator(".project-pill")).toContainText("priority-agent-demo");
    await expect(
      page.locator(".empty-state").getByRole("heading", { name: "What should we build in priority-agent-demo?" }),
    ).not.toBeVisible();

    await composer.getByRole("button", { name: "Project" }).click();
    await page.getByRole("textbox", { name: "Project path" }).fill("/Users/example/projects/sample-workspace");

    await expect(page.locator(".project-pill")).toContainText("priority-agent-demo");
    await expect(composer.getByRole("button", { exact: true, name: "Project" })).toContainText("priority-agent-demo");
  });

  test("desktop layout renders core controls and settings drawer", async ({ page }, testInfo) => {
    await page.goto("/?previewFixture=1");

    await expect(page.getByRole("heading", { name: "Desktop app Phase 1" })).toBeVisible();
    await expect(page.getByLabel("Session header")).toContainText("Desktop app Phase 1");
    await expect(page.getByRole("button", { name: /Direct Agent/ })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("button", { name: /LabRun/ })).toBeVisible();
    await expect(page.getByRole("complementary", { name: "Runtime inspector" })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Context" })).toHaveAttribute("aria-selected", "true");
    await expect(page.getByRole("tabpanel", { name: "Context" })).toContainText("Token budget");
    await expect(page.getByRole("tabpanel", { name: "Context" })).toContainText("Runtime estimate");
    await expect(page.getByRole("tabpanel", { name: "Context" })).toContainText("Prompt cache");
    await expect(page.getByRole("tabpanel", { name: "Context" })).toContainText("Provider input");
    await expect(page.getByRole("tabpanel", { name: "Context" })).toContainText("unavailable");
    const statusbar = page.locator(".statusbar");
    await statusbar.getByRole("button", { name: /Open Files inspector for workspace priority-agent-demo/ }).click();
    await expect(page.getByRole("tab", { name: "Files" })).toHaveAttribute("aria-selected", "true");
    await statusbar.getByRole("button", { name: /Open Context inspector for prompt cache/ }).click();
    await expect(page.getByRole("tab", { name: "Context" })).toHaveAttribute("aria-selected", "true");
    await statusbar.getByRole("button", { name: /Open Settings for model sample-model/ }).click();
    await expect(page.getByRole("complementary", { name: "Settings" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Settings" })).not.toBeVisible();
    await page.getByRole("tab", { name: "Files" }).click();
    const filesTab = page.getByRole("tabpanel", { name: "Files" });
    await expect(filesTab).toContainText("Map preview");
    await filesTab.getByRole("button", { name: /request_preparation_controller\.rs/ }).click();
    await expect(filesTab.getByLabel("Selected file preview")).toContainText("request_preparation_controller.rs");
    await expect(filesTab.getByLabel("Selected file preview")).toContainText("inject_project_map_zone");
    await page.getByRole("button", { name: /LabRun/ }).click();
    await expect(page.getByRole("tab", { name: "LabRun" })).toHaveAttribute("aria-selected", "true");
    const labRunTab = page.getByRole("tabpanel", { name: "LabRun" });
    await expect(labRunTab).toContainText("graduate_work");
    await expect(labRunTab).toContainText("Proposal intake");
    await expect(labRunTab).toContainText("Project controls");
    await expect(labRunTab).toContainText("Status board");
    await expect(labRunTab).toContainText("Professor side-channel");
    await expect(labRunTab).toContainText("Reports and artifacts");
    await expect(labRunTab).toContainText("Cost, context, and cache");
    await expect(labRunTab).toContainText("Professor review");
    await expect(labRunTab).toContainText("Graduate implementation result");
    await expect(labRunTab).toContainText("Evidence refs");
    await expect(labRunTab).toContainText("Report previews");
    await expect(labRunTab).toContainText("Decision: revise graduate implementation.");
    await expect(labRunTab).toContainText("Playwright panel action check failed during LabRun desktop validation.");
    await labRunTab.getByRole("button", { name: "Preview full report" }).first().click();
    await expect(labRunTab.getByLabel("Full LabRun report viewer")).toContainText("Full report viewer");
    await expect(labRunTab.getByLabel("Full LabRun report viewer")).toContainText("Professor steering");
    await labRunTab.getByRole("button", { name: "Close preview" }).click();
    await labRunTab.getByRole("button", { name: "Preview artifact body" }).first().click();
    await expect(labRunTab.getByLabel("LabRun artifact body viewer")).toContainText("Artifact body");
    await expect(labRunTab.getByLabel("LabRun artifact body viewer")).toContainText("review_summary");
    await labRunTab.getByRole("button", { name: "Close body" }).click();
    await labRunTab.getByRole("textbox", { name: "Search LabRun artifacts" }).fill("needs_revision");
    await expect(labRunTab).toContainText("Graduate implementation result");
    await expect(labRunTab).toContainText("Graduate Result");
    await expect(labRunTab).not.toContainText("Professor review");
    await labRunTab.getByRole("textbox", { name: "Search LabRun artifacts" }).fill("");
    await labRunTab.getByRole("button", { name: "Approve proposal" }).click();
    const composerInput = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(composerInput).toHaveValue("/lab approve labproposal_preview");
    await labRunTab.getByRole("button", { name: "Pause LabRun" }).click();
    await expect(composerInput).toHaveValue("/lab pause user_pause");
    await labRunTab.getByLabel("Professor message").fill("Please reconsider the blocked Playwright direction.");
    await labRunTab.getByRole("button", { name: "Message professor" }).click();
    await expect(composerInput).toHaveValue(
      "/lab professor Please reconsider the blocked Playwright direction.",
    );
    await labRunTab.getByRole("button", { name: "Report list" }).click();
    await expect(composerInput).toHaveValue("/lab report list");
    await composerInput.fill("");
    await page.getByRole("tab", { name: "Execution" }).click();
    await expect(page.getByRole("tabpanel", { name: "Execution" })).toContainText("Trace evidence");
    await expect(page.getByRole("tabpanel", { name: "Execution" })).toContainText("Stored output");
    await expect(page.locator(".startup-state-card")).toContainText("Restored session");
    await page.getByRole("button", { name: "More conversation actions" }).click();
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Command palette" })).not.toBeVisible();
    await expect(page.getByRole("button", { name: "More conversation actions" })).toBeFocused();
    await expect(page.getByRole("complementary", { name: "Workbench" })).not.toBeVisible();
    await page.getByRole("button", { name: /Workbench/ }).click();
    await expect(page.getByRole("complementary", { name: "Workbench" })).toBeVisible();
    await assertDrawerFocusTrap(page, "Workbench", "Close workbench");
    await expect(page.getByText("Environment diagnostics")).toBeVisible();
    await expect(page.getByRole("region", { name: "Frontend workbench" })).toContainText("Project intelligence");
    await expect(page.getByRole("region", { name: "Frontend workbench" })).toContainText("Project map");
    await expect(page.getByRole("region", { name: "Frontend workbench" })).toContainText("LabRun");
    await expect(page.getByRole("region", { name: "Lab status panel" })).toContainText("graduate_work");
    await expect(page.getByRole("region", { name: "Lab status panel" })).toContainText("recommended");
    await expect(page.getByRole("region", { name: "Lab status panel" })).toContainText("Playwright panel action check failed");
    await expect(page.getByRole("region", { name: "Lab status panel" })).toContainText("2 total");
    await page.getByRole("button", { name: "Open latest Lab report" }).click();
    await page.getByRole("button", { name: "Supervise Lab daemon" }).click();
    await expect(page.getByRole("region", { name: "Lab status panel" })).toContainText("graduate_work");
    await page.getByRole("button", { name: "Stage Lab meeting" }).click();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab meeting open");
    await page.getByRole("button", { name: "Stage Lab intervention" }).click();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab intervene ");
    await page.getByRole("button", { name: "Stage Lab continue" }).click();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab continue ");
    await page.getByRole("button", { name: "Stage Lab closeout" }).click();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab closeout auto");
    await page.getByRole("textbox", { name: "Message" }).fill("");
    await expect(page.getByRole("region", { name: "Frontend workbench" })).toContainText("Symbol index");
    await expect(page.getByRole("region", { name: "Symbol index preview" })).toContainText(
      "src/engine/conversation_loop/request_preparation_controller.rs",
    );
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Workbench" })).not.toBeVisible();
    await expect(page.getByRole("button", { name: /Workbench/ })).toBeFocused();
    await expect(page.getByLabel("Provider", { exact: true })).toBeVisible();
    await expect(page.getByLabel("Model", { exact: true })).toBeVisible();
    const composer = page.locator(".composer");
    await expect(page.getByLabel("Attached context", { exact: true })).toContainText("Project");
    await expect(page.getByLabel("Attached context", { exact: true })).toContainText("priority-agent-demo");
    await expect(page.getByLabel("Attached context", { exact: true })).toContainText("Add files or current diff");
    await page.getByRole("textbox", { name: "Message" }).fill("/lab");
    await expect(page.getByRole("listbox", { name: "Slash commands" })).toContainText("/lab dashboard");
    await page.getByRole("option", { name: /Use slash command \/lab dashboard/ }).click();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab dashboard");
    await page.getByRole("textbox", { name: "Message" }).fill("/");
    await expect(page.getByRole("listbox", { name: "Slash commands" })).toContainText("/help");
    await page.getByRole("textbox", { name: "Message" }).press("ArrowDown");
    await page.getByRole("textbox", { name: "Message" }).press("Enter");
    await expect(page.getByRole("textbox", { name: "Message" })).not.toHaveValue("/");
    await page.getByRole("textbox", { name: "Message" }).fill("");
    await composer.getByRole("button", { name: "Add context" }).click();
    await expect(page.getByRole("dialog", { name: "Add context options" })).toContainText("Current diff");
    await expect(page.getByRole("button", { name: "Attach file" })).toBeEnabled();
    await expect(page.getByLabel("Screenshot context unavailable")).toContainText(
      "Screen capture context is not connected yet.",
    );
    await expect(page.getByRole("button", { name: "Add screenshot" })).toHaveCount(0);
    await page.getByRole("button", { name: "Reference current diff" }).click();
    await expect(page.getByLabel("Attached context", { exact: true })).toContainText("Current diff");
    await expect(page.getByLabel("Attached context", { exact: true })).toContainText("2 files changed");
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("");
    await expect(page.getByRole("dialog", { name: "Add context options" })).not.toBeVisible();
    await page.getByRole("button", { name: "Open context Current diff" }).click();
    await expect(page.getByRole("complementary", { name: "Context details" })).toContainText("Changed files");
    await expect(page.getByRole("complementary", { name: "Context details" })).toContainText("Patch preview");
    await assertDrawerFocusTrap(page, "Context details", "Close context details");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Context details" })).not.toBeVisible();
    await composer.getByRole("button", { name: "Add context" }).click();
    await page.getByRole("button", { name: "Attach file" }).click();
    await expect(page.getByLabel("Attached context", { exact: true })).toContainText("App.tsx");
    await expect(page.getByLabel("Attached context", { exact: true })).toContainText("584 lines");
    await page.getByRole("button", { name: "Open context App.tsx" }).click();
    await expect(page.getByRole("complementary", { name: "Context details" })).toContainText("File preview");
    await expect(page.getByRole("complementary", { name: "Context details" })).toContainText(
      "apps/desktop/src/app/App.tsx",
    );
    await assertDrawerFocusTrap(page, "Context details", "Close context details");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Context details" })).not.toBeVisible();
    await page.getByRole("button", { name: "Remove context App.tsx" }).click();
    await expect(page.getByLabel("Attached context", { exact: true })).not.toContainText("584 lines");
    await composer.getByRole("button", { name: "Project" }).click();
    await expect(page.getByRole("dialog", { name: "Project controls" })).toBeVisible();
    await expect(page.getByRole("textbox", { name: "Project path" })).toHaveValue(
      "/Users/example/projects/priority-agent-demo",
    );
    await expect(page.getByRole("dialog", { name: "Project controls" })).toContainText("Recent projects");
    await expect(page.getByRole("button", { name: /Use recent project sample-workspace/ })).toBeVisible();
    await composer.getByRole("button", { name: "Mode" }).click();
    await expect(page.getByRole("dialog", { name: "Mode details" })).toContainText("Coding");
    await expect(page.getByRole("dialog", { name: "Mode details" })).toContainText("Auto low risk");
    await page.getByRole("button", { name: /Use mode Daily work/ }).click();
    await expect(page.getByRole("dialog", { name: "Mode details" })).toContainText("Daily work");
    await page.getByRole("button", { name: /Use permission Auto low risk/ }).click();
    await expect(page.getByRole("dialog", { name: "Mode details" })).toContainText("Auto low risk");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Mode details" })).not.toBeVisible();
    await composer.getByRole("button", { exact: true, name: "Provider" }).click();
    const providerDialog = page.getByRole("dialog", { name: "Provider controls" });
    await expect(providerDialog).toContainText("sample-model");
    await expect(providerDialog).toContainText("Provider setup repair");
    await providerDialog.getByRole("button", { name: "Repair provider Kimi Code" }).click();
    await providerDialog.getByLabel("Repair provider", { exact: true }).selectOption("kimi-code");
    await providerDialog.getByLabel("Provider API key").fill("sk-preview-kimi-code-12345678");
    await providerDialog.getByRole("button", { name: "Save key" }).click();
    await expect(providerDialog).toContainText("Saved preview credential for kimi-code");
    await page.getByRole("button", { exact: true, name: "Use model sample-model-fast" }).click();
    await expect(page.getByRole("dialog", { name: "Provider controls" })).not.toBeVisible();
    await expect(page.getByRole("button", { name: "New Chat" })).toBeVisible();
    await expect(page.locator(".project-pill")).toContainText("priority-agent-demo");
    await expect(page.getByLabel("Current session", { exact: true })).toContainText("Continuing");
    await expect(page.getByLabel("Current session", { exact: true })).toContainText("Desktop app Phase 1");
    await expect(page.locator(".startup-state-card")).toContainText("Restored session");
    await expect(page.locator(".startup-state-card")).toContainText("Desktop app Phase 1");
    await page.getByRole("button", { name: "Environment information" }).click();
    await expect(page.getByRole("complementary", { name: "Environment details" })).toBeVisible();
    await expect(page.getByRole("complementary", { name: "Environment details" })).toContainText("priority-agent-demo");
    await expect(page.getByRole("complementary", { name: "Environment details" })).toContainText("Permission mode");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Environment details" })).not.toBeVisible();
    await expect(page.getByRole("button", { name: "Environment information" })).toHaveAttribute("aria-expanded", "false");
    await page.getByRole("button", { name: "Environment information" }).click();
    await expect(page.getByRole("complementary", { name: "Environment details" })).toBeVisible();
    await page.getByRole("heading", { name: "Desktop app Phase 1" }).click();
    await expect(page.getByRole("complementary", { name: "Environment details" })).not.toBeVisible();

    await page.getByLabel("Search sessions").fill("Release");
    await expect(page.getByText("Release readiness notes")).toBeVisible();
    await expect(page.locator(".sidebar-section-row small", { hasText: "1 result" })).toBeVisible();
    await expect(page.locator(".recent-title mark", { hasText: "Release" })).toBeVisible();
    await expect(
      page.locator(".recent-list .recent-item", { hasText: "Desktop app Phase 1" }),
    ).not.toBeVisible();
    await page.locator(".recent-item", { hasText: "Release readiness notes" }).hover();
    await page.getByRole("button", { name: /Archive Release readiness notes/ }).click();
    await expect(page.locator(".session-undo-banner")).toContainText("Archived Release readiness notes");
    await page.getByRole("button", { name: "Undo" }).click();
    await expect(page.getByText("Release readiness notes")).toBeVisible();
    await page.locator(".recent-item", { hasText: "Release readiness notes" }).hover();
    await page.getByRole("button", { name: /Archive Release readiness notes/ }).click();
    await expect(page.getByText("No matching sessions")).toBeVisible();
    await page.getByRole("button", { name: "Clear session search" }).click();
    await expect(
      page.locator(".recent-list .recent-item", { hasText: "Desktop app Phase 1" }),
    ).toBeVisible();

    await page.getByRole("button", { name: /Rename Desktop app Phase 1/ }).click();
    await page.getByLabel("Session name").fill("Daily desktop flow");
    await page.getByRole("button", { name: "Save session title" }).click();
    await expect(
      page.locator(".recent-list .recent-item", { hasText: "Daily desktop flow" }),
    ).toBeVisible();
    await expect(page.getByLabel("Current session", { exact: true })).toContainText("Daily desktop flow");

    await page.locator(".recent-item-main", { hasText: "Daily desktop flow" }).click();
    await expect(page.getByText("Loaded preview session: web-preview")).toBeVisible();
    await page.getByRole("button", { name: "Export current session" }).click();
    await expect(page.getByRole("status", { name: "Export complete" })).toContainText(
      "Exported redacted markdown",
    );
    await expect(page.getByRole("button", { name: "Open export" })).toBeVisible();
    await page.getByRole("button", { name: "Dismiss" }).click();
    await expect(page.getByRole("status", { name: "Export complete" })).not.toBeVisible();

    await page.getByRole("button", { name: "New Chat" }).click();
    await expect(page.getByText("Loaded preview session: web-preview")).not.toBeVisible();
    await expect(page.getByLabel("Current session", { exact: true })).toContainText("New conversation");
    await expect(page.getByLabel("Current session", { exact: true })).toContainText("No active session");
    await expect(page.locator(".startup-state-card")).not.toBeVisible();
    await expect(
      page.locator(".empty-state").getByRole("heading", { name: "What should we build in priority-agent-demo?" }),
    ).toBeVisible();
    await expect(page.locator(".composer")).toHaveClass(/empty-composer/);
    await expect(page.getByPlaceholder("Ask anything")).toBeVisible();
    await page.getByRole("textbox", { name: "Message" }).fill("你好");
    await page.getByRole("button", { name: "Send message" }).click();
    await expect(page.getByText("这条消息没有发送给 LLM")).toBeVisible();
    await page.getByRole("textbox", { name: "Message" }).focus();
    await page.getByRole("textbox", { name: "Message" }).press("ArrowUp");
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("你好");
    await page.getByRole("textbox", { name: "Message" }).press("ArrowDown");
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("");
    await expect(page.locator(".timeline-run-row")).not.toBeVisible();
    await expect(page.locator(".timeline-event", { hasText: "Pnpm Test" })).not.toBeVisible();
    await page.locator(".recent-item", { hasText: "Daily desktop flow" }).hover();
    await page.getByRole("button", { name: /Delete Daily desktop flow/ }).click();
    await expect(page.getByRole("dialog", { name: "Delete session?" })).toBeVisible();
    await expect(page.getByRole("dialog")).toContainText("Daily desktop flow");
    await expect(page.getByRole("button", { name: "Cancel" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("dialog").getByRole("button", { name: "Delete" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Cancel" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Delete session?" })).not.toBeVisible();
    await expect(
      page.locator(".recent-list .recent-item", { hasText: "Daily desktop flow" }),
    ).toBeVisible();
    await page.locator(".recent-item", { hasText: "Daily desktop flow" }).hover();
    await page.getByRole("button", { name: /Delete Daily desktop flow/ }).click();
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(
      page.locator(".recent-list .recent-item", { hasText: "Daily desktop flow" }),
    ).toBeVisible();
    await page.locator(".recent-item", { hasText: "Daily desktop flow" }).hover();
    await page.getByRole("button", { name: /Delete Daily desktop flow/ }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Delete" }).click();
    await expect(
      page.locator(".recent-list .recent-item", { hasText: "Daily desktop flow" }),
    ).not.toBeVisible();

    await composer.getByRole("button", { name: "Add context" }).click();
    await page.getByRole("button", { name: "Reference current diff" }).click();
    await composer.getByRole("button", { name: "Add context" }).click();
    await page.getByRole("button", { name: "Attach file" }).click();
    await page.getByRole("textbox", { name: "Message" }).fill("Inspect the desktop timeline UI");
    await page.getByRole("button", { name: "Send message" }).click();
    await expect(page.locator(".composer-attachment")).not.toBeVisible();
    await expect(page.getByLabel("Attached context", { exact: true })).toContainText("Project");
    await expect(page.locator(".composer")).not.toHaveClass(/empty-composer/);
    await expect(page.locator(".timeline-run-row.completed", { hasText: "Done" })).toBeVisible();
    await expect(page.locator(".timeline-event", { hasText: "Agent run" })).not.toBeVisible();
    await expect(page.locator(".timeline-run-row")).toContainText("Current diff");
    await expect(page.locator(".timeline-run-row")).toContainText("App.tsx");
    await expect(page.getByLabel("Run summary panel")).toContainText("Validation");
    await expect(page.getByLabel("Run summary panel")).toContainText("Diff");
    await expect(page.getByLabel("Run summary panel")).toContainText("Permission");
    await expect(page.getByLabel("Run summary panel")).toContainText("Needs attention");
    await expect(page.getByLabel("Run summary panel")).toContainText("Pnpm Test");
    await expect(page.getByLabel("Run summary panel")).toContainText("Edited file");
    await expect(page.getByLabel("Run summary panel")).toContainText("Run review");
    await page.getByLabel("Run review actions").getByRole("button", { name: "Continue with fix" }).click();
    await expect(page.getByRole("textbox", { name: "Message", exact: true })).toHaveValue(
      /Please continue from the run review/,
    );
    await page.getByLabel("Run review actions").getByRole("button", { name: "Dismiss review" }).click();
    await expect(page.getByLabel("Run summary panel")).not.toBeVisible();
    await page.getByRole("textbox", { name: "Message", exact: true }).fill("");
    await page.locator(".timeline-run-row").getByRole("button", { name: "Open run context Current diff" }).click();
    await expect(page.getByRole("complementary", { name: "Context details" })).toContainText("Changed files");
    await assertDrawerFocusTrap(page, "Context details", "Close context details");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Context details" })).not.toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "3 tools" })).toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "bash x2" })).toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "file_edit" })).toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "1 failed" })).toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "1 file changed" })).toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "spine 7/7" })).toBeVisible();
    await expect(page.locator(".timeline-event.tool", { hasText: "Pnpm Test" })).toBeVisible();
    await expect(page.locator(".timeline-event.tool", { hasText: "Edited file" })).toBeVisible();
    await expect(page.locator(".timeline-event.tool", { hasText: "Cargo Test" })).toBeVisible();
    await page.getByRole("tab", { name: "Execution" }).click();
    await expect(page.getByRole("tabpanel", { name: "Execution" })).toContainText("Pnpm Test");
    await expect(page.getByRole("tabpanel", { name: "Execution" })).toContainText("Cargo Test");
    await expect(page.getByRole("tabpanel", { name: "Execution" })).toContainText("Stored output");
    await assertInspectorLongEvidenceWraps(page);
    await expect(page.locator(".message.assistant.final", { hasText: "Web preview received" })).toBeVisible();
    await expect(page.locator(".message.assistant.final")).toHaveClass(/run-group-final/);
    await expect(page.locator(".timeline-event", { hasText: "Pnpm Test" })).toBeVisible();
    await expect(page.locator(".timeline-event", { hasText: "Edited file" })).toBeVisible();
    await expect(page.locator(".timeline-event", { hasText: "cargo test failed" })).toBeVisible();
    await expect(page.locator(".timeline-event.usage", { hasText: "Token usage" })).not.toBeVisible();
    await expect(page.locator(".timeline-event.permission", { hasText: "Allow git push" })).toBeVisible();
    await expect(page.locator(".timeline-event.permission", { hasText: "checkpoint unavailable" })).not.toBeVisible();
    await page.locator(".timeline-event.permission").getByRole("button", { name: "Approve" }).click();
    await expect(page.locator(".timeline-event.permission", { hasText: "Permission approved" })).not.toBeVisible();
    await page.locator(".timeline-run-row").getByRole("button", { name: "Open trace for current run" }).click();
    await expect(page.getByRole("complementary", { name: "Run trace" })).toBeVisible();
    await assertDrawerFocusTrap(page, "Run trace", "Close");
    await expect(page.locator(".trace-item.active", { hasText: "Run started" })).toContainText("Attached context");
    await expect(page.locator(".trace-item.active", { hasText: "Run started" })).toContainText("Current diff");
    await expect(page.locator(".trace-item.active", { hasText: "Run started" })).toContainText("App.tsx");
    await expect(page.locator(".trace-item.tool", { hasText: "Pnpm Test" })).toContainText(
      "pnpm --dir apps/desktop test:ui-smoke",
    );
    await expect(page.locator(".trace-item.tool", { hasText: "Edited file" })).toContainText(
      "+  diff_preview?: string;",
    );
    await expect(page.locator(".trace-item.tool.failed", { hasText: "Cargo Test" })).toContainText(
      "timeline_cards_show_diff_preview",
    );
    await expect(page.locator(".trace-item.permission", { hasText: "Permission requested" })).toContainText(
      "checkpoint unavailable",
    );
    await expect(page.locator(".trace-item.runtime", { hasText: "Runtime diagnostic" })).toContainText("Proof summary");
    await expect(page.locator(".trace-item.runtime", { hasText: "Runtime diagnostic" })).toContainText("validation passed");
    await page.locator(".trace-item.active", { hasText: "Run started" }).getByRole("button", { name: "Open trace context Current diff" }).click();
    await expect(page.getByRole("complementary", { name: "Context details" })).toContainText("Patch preview");
    await assertDrawerFocusTrap(page, "Context details", "Close context details");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Context details" })).not.toBeVisible();
    await expect(page.getByRole("complementary", { name: "Run trace" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Run trace" })).not.toBeVisible();

    await page.locator(".topbar").getByRole("button", { name: "Output" }).click();
    await expect(page.getByRole("complementary", { name: "Tool output" })).toBeVisible();
    await assertDrawerFocusTrap(page, "Tool output", "Close");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Tool output" })).not.toBeVisible();
    await expect(page.locator(".topbar").getByRole("button", { name: "Output" })).toBeFocused();
    await page.getByRole("tab", { name: "Context" }).click();
    const contextTabAfterRun = page.getByRole("tabpanel", { name: "Context" });
    await expect(contextTabAfterRun).toContainText("Provider input");
    await expect(contextTabAfterRun).toContainText("128");
    await expect(contextTabAfterRun).toContainText("Provider output");
    await expect(contextTabAfterRun).toContainText("42");
    await expect(contextTabAfterRun).toContainText("Provider total");
    await expect(contextTabAfterRun).toContainText("170");
    await expect(contextTabAfterRun).toContainText("Cache write");
    await expect(contextTabAfterRun).toContainText("12");

    await assertNoHorizontalOverflow(page);
    await assertStableVerticalStack(page, [
      ".topbar",
      ".transcript",
      ".composer",
    ]);

    await page.screenshot({
      path: testInfo.outputPath("desktop-main.png"),
      fullPage: true,
    });

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await expect(page.getByRole("complementary", { name: "Settings" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Back to app" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Back to app" })).toBeFocused();
    await assertSettingsFocusTrap(page);
    const settingsNav = page.getByLabel("Settings categories");
    await expect(settingsNav.getByRole("button", { name: "General" })).toHaveClass(/active/);
    await expect(page.getByText("Work mode")).toBeVisible();
    await expect(page.getByRole("button", { name: /Daily work/ })).toHaveClass(/active/);
    await page.getByRole("button", { name: /Coding/ }).click();
    await expect(page.getByRole("button", { name: /Coding/ })).toHaveClass(/active/);
    await page.getByRole("button", { name: /Daily work/ }).click();
    await expect(page.getByRole("button", { name: /Daily work/ })).toHaveClass(/active/);
    await expect(page.getByText("Active session", { exact: true })).toBeVisible();
    await expect(page.getByText("Diagnostic log", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Open diagnostics folder" })).toBeVisible();
    await expect(page.getByText("Lab daemon supervision")).toBeVisible();
    await expect(page.getByLabel("Run automatic supervision while the desktop app is open")).not.toBeChecked();
    await expect(page.getByText("Next supervision")).toBeVisible();
    await expect(page.getByText("Not scheduled")).toBeVisible();
    await expect(page.getByText("Last result")).toBeVisible();
    await expect(page.getByText("Preview supervision completed.")).toBeVisible();
    await expect(page.locator(".settings-project-list")).toContainText("priority-agent-demo");
    await expect(page.locator(".settings-project-list")).toContainText("sample-workspace");
    await settingsNav.getByRole("button", { name: "Permissions" }).click();
    await expect(settingsNav.getByRole("button", { name: "Permissions" })).toHaveClass(/active/);
    await expect(page.getByText("Permission defaults")).toBeVisible();
    await page.getByRole("button", { name: /Auto low risk/ }).click();
    await expect(page.getByRole("button", { name: /Auto low risk/ })).toHaveClass(/active/);
    await settingsNav.getByRole("button", { name: "Provider" }).click();
    await expect(page.getByText("Provider setup")).toBeVisible();
    await expect(page.getByText("not the system keychain")).toBeVisible();
    await expect(page.getByRole("button", { name: "Open settings folder" })).toBeVisible();
    await settingsNav.getByRole("button", { name: "Diagnostics" }).click();
    await expect(
      page.getByRole("complementary", { name: "Settings" }).locator(".settings-diagnostic", {
        hasText: "Provider keys",
      }),
    ).toBeVisible();
    await expect(
      page.getByRole("complementary", { name: "Settings" }).locator(".settings-diagnostic", {
        hasText: "Diagnostic logs",
      }),
    ).toBeVisible();

    await assertNoHorizontalOverflow(page);
    await page.screenshot({
      path: testInfo.outputPath("desktop-settings.png"),
      fullPage: true,
    });
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Settings" })).not.toBeVisible();
    await expect(page.getByRole("button", { name: "Settings", exact: true })).toBeFocused();
  });

  test("command palette stages Lab slash commands", async ({ page }) => {
    await page.goto("/?previewFixture=1");

    await page.keyboard.press("Control+K");
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    const commandSearch = page.getByRole("combobox", { name: "Command search" });
    await expect(commandSearch).toBeFocused();
    await assertCommandPaletteFocusTrap(page);
    await commandSearch.fill("lab ");
    await expect(page.getByRole("listbox", { name: "Command results" })).toBeVisible();
    await expect(page.getByRole("option", { name: /Lab Dashboard/ })).toHaveAttribute("aria-selected", "true");
    await commandSearch.press("ArrowDown");
    await expect(page.getByRole("option", { name: /Lab Meeting/ })).toHaveAttribute("aria-selected", "true");
    await commandSearch.press("Enter");

    await expect(page.getByRole("dialog", { name: "Command palette" })).not.toBeVisible();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab meeting open");

    await page.keyboard.press("Control+K");
    await page.getByRole("combobox", { name: "Command search" }).fill("daemon");
    await page.getByRole("option", { name: /Lab Daemon Health/ }).click();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab daemon health");

    await page.getByRole("button", { name: "More conversation actions" }).click();
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    await page.getByRole("combobox", { name: "Command search" }).fill("trace");
    await page.getByRole("option", { name: /Open Trace/ }).click();
    await expect(page.getByRole("complementary", { name: "Run trace" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Run trace" })).not.toBeVisible();

    await page.getByRole("button", { name: "More conversation actions" }).click();
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    await page.getByRole("combobox", { name: "Command search" }).fill("diagnostics");
    await page.getByRole("option", { name: /Show Diagnostics/ }).click();
    await expect(page.getByRole("tab", { name: "Diagnostics" })).toHaveAttribute("aria-selected", "true");

    await page.getByRole("button", { name: "More conversation actions" }).click();
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    await page.getByRole("combobox", { name: "Command search" }).fill("files");
    await page.getByRole("option", { name: /Show Files/ }).click();
    await expect(page.getByRole("tab", { name: "Files" })).toHaveAttribute("aria-selected", "true");

    await page.getByRole("textbox", { name: "Message" }).fill("");
    await page.getByLabel("Session header").click();
    await page.keyboard.press("/");
    await expect(page.getByRole("textbox", { name: "Message" })).toBeFocused();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/");
    await expect(page.getByRole("listbox", { name: "Slash commands" })).toContainText("/help");
  });

  test("startup Lab recovery card stages safe actions", async ({ page }) => {
    await page.goto("/?previewFixture=labRecovery");

    const card = page.locator(".startup-state-card.lab_recovery");
    await expect(card).toContainText("Lab recovery");
    await expect(card).toContainText("labrun_preview");
    await expect(card).toContainText("graduate_work");

    await page.getByRole("button", { name: "Resume" }).click();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab resume");

    await page.getByRole("button", { name: "Dashboard" }).click();
    await expect(page.getByRole("textbox", { name: "Message" })).toHaveValue("/lab dashboard");
    await expect(page.getByRole("complementary", { name: "Workbench" })).toBeVisible();
    await page.getByRole("button", { name: "Close workbench" }).click();
    await expect(page.getByRole("complementary", { name: "Workbench" })).not.toBeVisible();

    await page.getByRole("button", { name: "Keep paused" }).click();
    await expect(card).not.toBeVisible();
  });

  test("mobile layout keeps composer controls inside viewport", async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "Desktop app Phase 1" })).toBeVisible();
    await expect(page.getByRole("button", { name: /Workbench/ })).toBeVisible();
    await page.getByRole("button", { name: /Workbench/ }).click();
    await expect(page.getByRole("region", { name: "Frontend workbench" })).toContainText("Project intelligence");
    await page.getByRole("button", { name: "Close workbench" }).click();
    const mobileModeSwitcher = page.getByLabel("Agent workspace mode");
    await expect(mobileModeSwitcher.getByRole("button", { name: /Direct Agent/ })).toHaveAttribute("aria-pressed", "true");
    await mobileModeSwitcher.getByRole("button", { name: /LabRun/ }).click();
    const mobileInspector = page.getByRole("complementary", { name: "Runtime inspector drawer" });
    await expect(mobileModeSwitcher.getByRole("button", { name: /LabRun/ })).toHaveAttribute("aria-pressed", "true");
    await expect(mobileInspector).toBeVisible();
    await assertOnlyPrimaryDrawerOpen(page, "Runtime inspector drawer");
    await expect(mobileInspector.getByRole("tab", { name: "LabRun" })).toHaveAttribute("aria-selected", "true");
    await expect(mobileInspector.getByRole("tabpanel", { name: "LabRun" })).toContainText("Project controls");
    await page.getByRole("button", { name: "Close runtime inspector" }).click();
    await mobileModeSwitcher.getByRole("button", { name: /Direct Agent/ }).click();
    await expect(mobileModeSwitcher.getByRole("button", { name: /Direct Agent/ })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByLabel("Provider", { exact: true })).toBeVisible();
    await expect(page.getByLabel("Model", { exact: true })).toBeVisible();
    await expect(page.locator(".topbar").getByRole("button", { name: "Open settings" })).toBeVisible();
    await page.locator(".topbar").getByRole("button", { name: "Open settings" }).click();
    await expect(page.getByRole("complementary", { name: "Settings" })).toBeVisible();
    await assertOnlyPrimaryDrawerOpen(page, "Settings");
    await expect(page.getByRole("button", { name: "Back to app" })).toBeFocused();
    await assertSettingsFocusTrap(page);
    await assertMobileSettingsDrawerReadable(page);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Settings" })).not.toBeVisible();
    await expect(page.locator(".topbar").getByRole("button", { name: "Open settings" })).toBeFocused();
    await expect(page.locator(".topbar").getByRole("button", { name: "Output" })).toBeVisible();
    await expect(page.locator(".topbar").getByRole("button", { name: "Trace" })).toBeVisible();
    await expect(page.locator(".topbar").getByRole("button", { name: "Trace" })).toHaveAttribute("aria-expanded", "false");
    await page.locator(".topbar").getByRole("button", { name: "Trace" }).click();
    await expect(page.locator(".topbar").getByRole("button", { name: "Trace" })).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("complementary", { name: "Run trace" })).toBeVisible();
    await assertOnlyPrimaryDrawerOpen(page, "Run trace");
    await assertDrawerFocusTrap(page, "Run trace", "Close");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Run trace" })).not.toBeVisible();
    await expect(page.locator(".topbar").getByRole("button", { name: "Trace" })).toBeFocused();
    await expect(page.locator(".topbar").getByRole("button", { name: "Trace" })).toHaveAttribute("aria-expanded", "false");
    await expect(page.locator(".topbar").getByRole("button", { name: "Output" })).toHaveAttribute("aria-expanded", "false");
    await page.locator(".topbar").getByRole("button", { name: "Output" }).click();
    await expect(page.locator(".topbar").getByRole("button", { name: "Output" })).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("complementary", { name: "Tool output" })).toBeVisible();
    await assertOnlyPrimaryDrawerOpen(page, "Tool output");
    await assertDrawerFocusTrap(page, "Tool output", "Close");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Tool output" })).not.toBeVisible();
    await expect(page.locator(".topbar").getByRole("button", { name: "Output" })).toBeFocused();
    await expect(page.locator(".topbar").getByRole("button", { name: "Output" })).toHaveAttribute("aria-expanded", "false");

    await assertNoHorizontalOverflow(page);
    await assertComposerContextHintReadable(page);
    await assertSessionHeaderMetaReadable(page);
    await assertStartupStateCardReadable(page);
    await assertMobileStatusBarReadable(page);
    await page.locator(".statusbar").getByRole("button", { name: /Open Context inspector/ }).first().click();
    await expect(mobileInspector).toBeVisible();
    await assertOnlyPrimaryDrawerOpen(page, "Runtime inspector drawer");
    await expect(mobileInspector.getByRole("tab", { name: "Context" })).toHaveAttribute("aria-selected", "true");
    await expect(page.getByRole("button", { name: "Close runtime inspector" })).toBeFocused();
    await assertDrawerFocusTrap(page, "Runtime inspector drawer", "Close runtime inspector");
    await assertNoDuplicateIds(page);
    await page.keyboard.press("Escape");
    await expect(mobileInspector).not.toBeVisible();
    await expect(page.locator(".statusbar").getByRole("button", { name: /Open Context inspector/ }).first()).toBeFocused();
    await assertMobileTopbarActionsNotClipped(page);
    await page.screenshot({
      path: testInfo.outputPath("mobile-main.png"),
      fullPage: true,
    });
    await page.getByRole("button", { name: "More conversation actions" }).click();
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    await assertCommandPaletteFitsViewport(page);
    await assertCommandPaletteFocusTrap(page);
    await page.getByRole("dialog", { name: "Command palette" }).getByRole("combobox", { name: "Command search" }).fill("files");
    await page.getByRole("dialog", { name: "Command palette" }).getByRole("option", { name: /Show Files/ }).click();
    await expect(mobileInspector).toBeVisible();
    await assertOnlyPrimaryDrawerOpen(page, "Runtime inspector drawer");
    await expect(mobileInspector.getByRole("tab", { name: "Files" })).toHaveAttribute("aria-selected", "true");
    await assertNoDuplicateIds(page);
    await page.getByRole("button", { name: "Close runtime inspector" }).click();
    await page.getByRole("button", { name: "More conversation actions" }).click();
    await expect(page.getByRole("dialog", { name: "Command palette" })).toBeVisible();
    await page.getByRole("dialog", { name: "Command palette" }).getByRole("option", { name: /New Chat/ }).click();
    await expect(page.getByLabel("Current session", { exact: true })).toContainText("No active session");
  });

  test("web preview does not fake agent replies and remains responsive", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: "New Chat" }).click();
    const composer = page.locator(".composer");
    const textbox = page.getByRole("textbox", { name: "Message" });
    const send = page.getByRole("button", { name: "Send message" });

    await textbox.fill("你好");
    await send.click();
    await expect(page.getByText("这条消息没有发送给 LLM")).toBeVisible();
    await expect(page.getByText("你好，我在。")).not.toBeVisible();
    await expect(composer).not.toHaveClass(/running/);

    await textbox.fill("请帮我看看桌面上有什么东西");
    await expect(send).toBeEnabled();
    await send.click();
    await expect(page.getByText("你的消息还在输入框历史里：请帮我看看桌面上有什么东西")).toBeVisible();
    await expect(page.locator(".timeline-run-row")).not.toBeVisible();
    await expect(composer).not.toHaveClass(/running/);
  });

  test("runtime error banner exposes trace and diagnostics actions", async ({ page }) => {
    await page.goto("/?previewFixture=1");

    await page.getByRole("button", { name: "New Chat" }).click();
    const textbox = page.getByRole("textbox", { name: "Message" });
    await textbox.fill("fixture run error");
    await page.getByRole("button", { name: "Send message" }).click();

    const alert = page.getByRole("alert", { name: "Runtime issue" });
    await expect(alert).toContainText("Runtime issue");
    await expect(alert).toContainText("Simulated desktop runtime error for web preview validation.");
    await alert.getByRole("button", { name: "Open trace" }).click();
    await expect(page.getByRole("complementary", { name: "Run trace" })).toBeVisible();
    await expect(page.getByRole("complementary", { name: "Run trace" })).toContainText("Run error");
    await page.keyboard.press("Escape");

    await alert.getByRole("button", { name: "Diagnostics" }).click();
    await expect(page.getByRole("tab", { name: "Diagnostics" })).toHaveAttribute("aria-selected", "true");
    await alert.getByRole("button", { name: "Dismiss" }).click();
    await expect(alert).not.toBeVisible();
  });

  test("mobile runtime error actions open visible drawers", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/?previewFixture=1");

    const textbox = page.getByRole("textbox", { name: "Message", exact: true });
    await textbox.fill("fixture run error");
    await page.getByRole("button", { name: "Send message" }).click();

    const alert = page.getByRole("alert", { name: "Runtime issue" });
    await expect(alert).toContainText("Simulated desktop runtime error for web preview validation.");
    await alert.getByRole("button", { name: "Open trace" }).click();
    await expect(page.getByRole("complementary", { name: "Run trace" })).toBeVisible();
    await assertOnlyPrimaryDrawerOpen(page, "Run trace");
    await expect(page.getByRole("complementary", { name: "Run trace" })).toContainText("Run error");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Run trace" })).not.toBeVisible();

    await alert.getByRole("button", { name: "Diagnostics" }).click();
    const mobileInspector = page.getByRole("complementary", { name: "Runtime inspector drawer" });
    await expect(mobileInspector).toBeVisible();
    await assertOnlyPrimaryDrawerOpen(page, "Runtime inspector drawer");
    await expect(mobileInspector.getByRole("tab", { name: "Diagnostics" })).toHaveAttribute("aria-selected", "true");
    await page.getByRole("button", { name: "Close runtime inspector" }).click();
    await expect(mobileInspector).not.toBeVisible();
  });
});

async function assertNoHorizontalOverflow(page: Page) {
  const result = await page.evaluate(() => {
    const root = document.documentElement;
    const overflowing = Array.from(document.querySelectorAll<HTMLElement>("body *"))
      .filter((element) => !element.closest(".statusbar"))
      .map((element) => {
        const rect = element.getBoundingClientRect();
        return {
          className: element.className.toString(),
          tagName: element.tagName.toLowerCase(),
          left: rect.left,
          right: rect.right,
          width: rect.width,
        };
      })
      .filter((rect) => rect.width > 1 && (rect.left < -1 || rect.right > window.innerWidth + 1));

    return {
      ok: root.scrollWidth <= window.innerWidth + 1 && overflowing.length === 0,
      viewportWidth: window.innerWidth,
      scrollWidth: root.scrollWidth,
      overflowing: overflowing.slice(0, 5),
    };
  });

  expect(result).toEqual({
    ok: true,
    viewportWidth: result.viewportWidth,
    scrollWidth: result.scrollWidth,
    overflowing: [],
  });
}

async function assertCommandPaletteFitsViewport(page: Page) {
  const result = await page.evaluate(() => {
    const palette = document.querySelector<HTMLElement>(".cmd-palette");
    if (!palette) {
      return { ok: false, reason: "missing command palette" };
    }
    const rect = palette.getBoundingClientRect();
    const overflowingItems = Array.from(palette.querySelectorAll<HTMLElement>(".cmd-palette-item, .cmd-palette-item-label, .cmd-palette-item-hint"))
      .map((element) => {
        const itemRect = element.getBoundingClientRect();
        const style = window.getComputedStyle(element);
        return {
          className: element.className.toString(),
          overflowX: style.overflowX,
          right: itemRect.right,
          textOverflow: style.textOverflow,
          whiteSpace: style.whiteSpace,
        };
      })
      .filter((item) => item.right > rect.right + 1);
    return {
      ok: rect.left >= 0 && rect.right <= window.innerWidth && rect.width <= window.innerWidth && overflowingItems.length === 0,
      left: rect.left,
      overflowingItems,
      right: rect.right,
      viewportWidth: window.innerWidth,
      width: rect.width,
    };
  });

  expect(result).toMatchObject({
    ok: true,
    overflowingItems: [],
  });
}

async function assertCommandPaletteFocusTrap(page: Page) {
  const palette = page.getByRole("dialog", { name: "Command palette" });
  const search = page.getByRole("combobox", { name: "Command search" });
  await expect(search).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await assertActiveElementInsideCommandPalette(page);
  await page.keyboard.press("Tab");
  await expect(search).toBeFocused();
  await expect(palette).toBeVisible();
}

async function assertActiveElementInsideCommandPalette(page: Page) {
  const result = await page.evaluate(() => {
    const palette = document.querySelector<HTMLElement>(".cmd-palette");
    const active = document.activeElement;
    return {
      activeText: active?.textContent?.trim() || "",
      ok: Boolean(palette && active && palette.contains(active)),
      tagName: active?.tagName || "",
    };
  });

  expect(result).toMatchObject({ ok: true });
}

async function assertStableVerticalStack(page: Page, selectors: string[]) {
  const boxes = await page.evaluate((inputSelectors) => {
    return inputSelectors.map((selector) => {
      const element = document.querySelector(selector);
      if (!element) {
        return null;
      }
      const rect = element.getBoundingClientRect();
      return {
        selector,
        top: rect.top,
        bottom: rect.bottom,
      };
    });
  }, selectors);

  expect(boxes.every(Boolean)).toBe(true);
  for (let index = 1; index < boxes.length; index += 1) {
    const previous = boxes[index - 1];
    const current = boxes[index];
    expect(current!.top).toBeGreaterThanOrEqual(previous!.bottom - 1);
  }
}

async function assertInspectorLongEvidenceWraps(page: Page) {
  const result = await page.evaluate(() => {
    const detail = Array.from(document.querySelectorAll<HTMLElement>(".inspector-list-item p"))
      .find((element) => element.textContent?.includes("Allow git push"));
    if (!detail) {
      return { ok: false, reason: "missing long inspector evidence detail" };
    }
    const style = window.getComputedStyle(detail);
    return {
      ok:
        style.whiteSpace === "normal" &&
        style.overflowWrap === "anywhere" &&
        detail.scrollWidth <= detail.clientWidth + 1,
      overflowWrap: style.overflowWrap,
      scrollWidth: detail.scrollWidth,
      clientWidth: detail.clientWidth,
      whiteSpace: style.whiteSpace,
    };
  });

  expect(result).toMatchObject({
    ok: true,
    overflowWrap: "anywhere",
    whiteSpace: "normal",
  });
}

async function assertComposerContextHintReadable(page: Page) {
  const result = await page.evaluate(() => {
    const hint = document.querySelector<HTMLElement>(".composer-context-empty span");
    if (!hint) {
      return { ok: false, reason: "missing composer context hint" };
    }
    const style = window.getComputedStyle(hint);
    return {
      ok:
        style.whiteSpace === "normal" &&
        style.overflowWrap === "anywhere" &&
        hint.scrollWidth <= hint.clientWidth + 1,
      clientWidth: hint.clientWidth,
      overflowWrap: style.overflowWrap,
      scrollWidth: hint.scrollWidth,
      text: hint.textContent?.trim(),
      whiteSpace: style.whiteSpace,
    };
  });

  expect(result).toMatchObject({
    ok: true,
    overflowWrap: "anywhere",
    text: "Add files or current diff when the task needs sharper context.",
    whiteSpace: "normal",
  });
}

async function assertStartupStateCardReadable(page: Page) {
  const result = await page.evaluate(() => {
    const card = document.querySelector<HTMLElement>(".startup-state-card");
    const detail = card?.querySelector<HTMLElement>("strong");
    if (!card || !detail) {
      return { ok: false, reason: "missing startup state card detail" };
    }
    const cardBox = card.getBoundingClientRect();
    const detailStyle = window.getComputedStyle(detail);
    return {
      ok:
        detailStyle.whiteSpace === "normal" &&
        detailStyle.overflowWrap === "anywhere" &&
        detail.scrollWidth <= detail.clientWidth + 1 &&
        cardBox.left >= -1 &&
        cardBox.right <= window.innerWidth + 1,
      cardRight: cardBox.right,
      clientWidth: detail.clientWidth,
      overflowWrap: detailStyle.overflowWrap,
      scrollWidth: detail.scrollWidth,
      text: detail.textContent?.trim(),
      viewportWidth: window.innerWidth,
      whiteSpace: detailStyle.whiteSpace,
    };
  });

  expect(result).toMatchObject({
    ok: true,
    overflowWrap: "anywhere",
    text: "Continuing Desktop app Phase 1 in priority-agent-demo",
    whiteSpace: "normal",
  });
}

async function assertSessionHeaderMetaReadable(page: Page) {
  const result = await page.evaluate(() => {
    const provider = Array.from(document.querySelectorAll<HTMLElement>(".session-header-meta span"))
      .find((element) => element.textContent?.includes("sample-model"));
    if (!provider) {
      return { ok: false, reason: "missing provider meta" };
    }
    const style = window.getComputedStyle(provider);
    return {
      ok:
        style.whiteSpace === "normal" &&
        style.overflowWrap === "anywhere" &&
        provider.scrollWidth <= provider.clientWidth + 1,
      clientWidth: provider.clientWidth,
      overflowWrap: style.overflowWrap,
      scrollWidth: provider.scrollWidth,
      text: provider.textContent?.replace(/\s+/g, " ").trim(),
      whiteSpace: style.whiteSpace,
    };
  });

  expect(result).toMatchObject({
    ok: true,
    overflowWrap: "anywhere",
    text: "sample-provider / sample-model",
    whiteSpace: "normal",
  });
}

async function assertMobileSettingsDrawerReadable(page: Page) {
  const settings = page.getByRole("complementary", { name: "Settings" });
  const settingsNav = page.getByRole("navigation", { name: "Settings categories" });

  await settingsNav.getByRole("button", { name: "Provider" }).click();
  await expect(settings).toContainText("Provider key required");
  await expect(settings.getByRole("combobox", { name: "Provider" })).toBeVisible();
  await expect(settings.getByPlaceholder(/Paste .* API key here/)).toBeVisible();
  await expect(settings.getByRole("button", { name: "Save key" })).toBeVisible();
  await assertSettingsDrawerContentFitsViewport(page);

  await settingsNav.getByRole("button", { name: "Permissions" }).click();
  await expect(settings).toContainText("Permission defaults");
  await expect(settings.locator(".permission-option")).toHaveCount(4);
  await assertSettingsDrawerContentFitsViewport(page);
}

async function assertSettingsFocusTrap(page: Page) {
  await assertDrawerFocusTrap(page, "Settings", "Back to app");
}

async function assertDrawerFocusTrap(page: Page, drawerLabel: string, firstControlName: string | RegExp) {
  const drawer = page.getByRole("complementary", { name: drawerLabel });
  const firstControl = drawer.getByRole("button", { name: firstControlName });
  await expect(firstControl).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await assertActiveElementInsideDrawer(page, drawerLabel);
  await page.keyboard.press("Tab");
  await assertActiveElementInsideDrawer(page, drawerLabel);
  await page.keyboard.press("Tab");
  await assertActiveElementInsideDrawer(page, drawerLabel);
}

async function assertActiveElementInsideDrawer(page: Page, drawerLabel: string) {
  const result = await page.evaluate((label) => {
    const drawer = Array.from(document.querySelectorAll<HTMLElement>("aside[aria-label]"))
      .find((element) => element.getAttribute("aria-label") === label);
    const active = document.activeElement;
    return {
      activeText: active?.textContent?.trim() || "",
      ok: Boolean(drawer && active && drawer.contains(active)),
      tagName: active?.tagName || "",
    };
  }, drawerLabel);

  expect(result).toMatchObject({ ok: true });
}

async function assertNoDuplicateIds(page: Page) {
  const result = await page.evaluate(() => {
    const seen = new Set<string>();
    const duplicates: string[] = [];
    for (const element of Array.from(document.querySelectorAll<HTMLElement>("[id]"))) {
      if (seen.has(element.id)) {
        duplicates.push(element.id);
      }
      seen.add(element.id);
    }
    return {
      duplicates,
      ok: duplicates.length === 0,
    };
  });

  expect(result).toMatchObject({ ok: true, duplicates: [] });
}

async function assertOnlyPrimaryDrawerOpen(page: Page, expectedLabel: string) {
  const result = await page.evaluate(() => {
    const primaryLabels = new Set([
      "Settings",
      "Workbench",
      "Run trace",
      "Tool output",
      "Runtime inspector drawer",
    ]);
    const labels = Array.from(document.querySelectorAll<HTMLElement>("aside[aria-label]"))
      .map((element) => element.getAttribute("aria-label") || "")
      .filter((label) => primaryLabels.has(label));
    return {
      labels,
      ok: labels.length === 1,
    };
  });

  expect(result).toMatchObject({ labels: [expectedLabel], ok: true });
}

async function assertSettingsDrawerContentFitsViewport(page: Page) {
  const result = await page.evaluate(() => {
    const drawer = document.querySelector<HTMLElement>(".settings-drawer");
    if (!drawer) {
      return { ok: false, reason: "missing settings drawer" };
    }
    const drawerRect = drawer.getBoundingClientRect();
    const selectors = [
      ".settings-page",
      ".settings-page-header",
      ".settings-content",
      ".provider-credential-row",
      ".provider-select",
      ".provider-key-input",
      ".provider-save-button",
      ".permission-options",
      ".permission-option",
    ];
    const overflowing = selectors.flatMap((selector) =>
      Array.from(drawer.querySelectorAll<HTMLElement>(selector))
        .map((element) => {
          const rect = element.getBoundingClientRect();
          return {
            selector,
            left: rect.left,
            right: rect.right,
            width: rect.width,
          };
        })
        .filter((item) => item.left < drawerRect.left - 1 || item.right > drawerRect.right + 1),
    );
    return {
      ok:
        drawerRect.left >= 0 &&
        drawerRect.right <= window.innerWidth &&
        drawerRect.width <= window.innerWidth &&
        overflowing.length === 0,
      drawerLeft: drawerRect.left,
      drawerRight: drawerRect.right,
      overflowing,
      viewportWidth: window.innerWidth,
    };
  });

  expect(result).toMatchObject({
    ok: true,
    overflowing: [],
  });
}

async function assertMobileStatusBarReadable(page: Page) {
  const result = await page.evaluate(() => {
    const statusbar = document.querySelector<HTMLElement>(".statusbar");
    if (!statusbar) {
      return { ok: false, reason: "missing statusbar" };
    }
    const rect = statusbar.getBoundingClientRect();
    const style = window.getComputedStyle(statusbar);
    const text = statusbar.textContent?.replace(/\s+/g, " ").trim() || "";
    const required = ["api.example.com", "Cache", "Tokens", "Context", "sample-model", "priority-agent-demo"];
    return {
      ok:
        rect.left >= -1 &&
        rect.right <= window.innerWidth + 1 &&
        statusbar.clientWidth >= window.innerWidth - 1 &&
        statusbar.scrollWidth >= statusbar.clientWidth &&
        (style.overflowX === "auto" || style.overflowX === "scroll") &&
        required.every((item) => text.includes(item)),
      clientWidth: statusbar.clientWidth,
      overflowX: style.overflowX,
      requiredFound: required.filter((item) => text.includes(item)),
      right: rect.right,
      scrollWidth: statusbar.scrollWidth,
      text,
      viewportWidth: window.innerWidth,
    };
  });

  expect(result).toMatchObject({
    ok: true,
    overflowX: "auto",
  });
}

async function assertMobileTopbarActionsNotClipped(page: Page) {
  const result = await page.evaluate(() => {
    const topbar = document.querySelector<HTMLElement>(".topbar");
    const sessionHeader = document.querySelector<HTMLElement>(".session-header");
    if (!topbar || !sessionHeader) {
      return { ok: false, reason: "missing topbar or session header" };
    }
    const topbarBox = topbar.getBoundingClientRect();
    const sessionBox = sessionHeader.getBoundingClientRect();
    const buttons = Array.from(topbar.querySelectorAll<HTMLButtonElement>("button"))
      .filter((button) => {
        const label = button.textContent?.trim();
        return label === "Output" || label === "Trace";
      })
      .map((button) => {
        const rect = button.getBoundingClientRect();
        return {
          label: button.textContent?.trim() || "",
          bottom: rect.bottom,
          topbarBottom: topbarBox.bottom,
        };
      });
    const clippedButtons = buttons.filter((button) => button.bottom > topbarBox.bottom + 1);
    return {
      ok: buttons.length === 2 && clippedButtons.length === 0 && topbarBox.bottom <= sessionBox.top + 1,
      buttons,
      clippedButtons,
      topbarBottom: topbarBox.bottom,
      sessionTop: sessionBox.top,
    };
  });

  expect(result).toMatchObject({
    ok: true,
    clippedButtons: [],
  });
}

test.describe("goal progress row", () => {
  test("shows goal progress when a goal is active", async ({ page }) => {
    await page.goto("/?previewFixture=goal");
    const row = page.locator(".goal-progress-row");
    await expect(row).toBeVisible();
    await expect(row).toContainText("Preview daily desktop goal");
    await expect(row).toContainText("Active");
    await expect(row).toContainText("3/10");
    await expect(row.getByRole("button", { name: "Edit goal objective" })).toBeVisible();
    await expect(row.getByRole("button", { name: "Pause goal" })).toBeVisible();
    await expect(row.getByRole("button", { name: "Clear goal" })).toBeVisible();

    await row.getByRole("button", { name: "Edit goal objective" }).click();
    const editInput = row.getByRole("textbox", { name: "Goal objective" });
    await expect(editInput).toBeFocused();
    await expect(editInput).toHaveValue("Preview daily desktop goal");
    await editInput.fill("Do not save this draft");
    await editInput.press("Escape");
    await expect(editInput).not.toBeVisible();
    await expect(row).toContainText("Preview daily desktop goal");
    await expect(row).not.toContainText("Do not save this draft");
  });

  test("goal progress row does not overlap composer", async ({ page }) => {
    await page.goto("/?previewFixture=goal");
    await expect(page.locator(".goal-progress-row")).toBeVisible();
    await expect(page.locator(".composer")).toBeVisible();
    await expect(page.locator(".goal-progress-row")).not.toHaveCSS("position", "absolute");
  });
});
