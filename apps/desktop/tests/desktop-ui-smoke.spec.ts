import { expect, test, type Page } from "@playwright/test";

test.describe("desktop UI smoke", () => {
  test("desktop layout renders core controls and settings drawer", async ({ page }, testInfo) => {
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "What should we build in rust-agent?" }))
      .toBeVisible();
    await expect(page.getByText("Environment diagnostics")).toBeVisible();
    await expect(page.getByLabel("Provider")).toBeVisible();
    await expect(page.getByLabel("Model")).toBeVisible();
    const composer = page.locator(".composer");
    await composer.getByRole("button", { name: "Project" }).click();
    await expect(page.getByRole("dialog", { name: "Project controls" })).toBeVisible();
    await expect(page.getByRole("textbox", { name: "Project path" })).toHaveValue(
      "/Users/georgexu/Desktop/rust-agent",
    );
    await expect(page.getByRole("dialog", { name: "Project controls" })).toContainText("Recent projects");
    await expect(page.getByRole("button", { name: /Use recent project bioclaw/ })).toBeVisible();
    await composer.getByRole("button", { name: "Mode" }).click();
    await expect(page.getByRole("dialog", { name: "Mode details" })).toContainText("Coding");
    await expect(page.getByRole("dialog", { name: "Mode details" })).toContainText("Full access");
    await page.getByRole("button", { name: /Use mode Daily work/ }).click();
    await expect(page.getByRole("dialog", { name: "Mode details" })).toContainText("Daily work");
    await page.getByRole("button", { name: /Use permission Auto low risk/ }).click();
    await expect(page.getByRole("dialog", { name: "Mode details" })).toContainText("Auto low risk");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Mode details" })).not.toBeVisible();
    await composer.getByRole("button", { exact: true, name: "Provider" }).click();
    await expect(page.getByRole("dialog", { name: "Provider controls" })).toContainText("kimi-k2.5");
    await page.getByRole("button", { exact: true, name: "Use model kimi-k2.5" }).click();
    await expect(page.getByRole("dialog", { name: "Provider controls" })).not.toBeVisible();
    await expect(page.getByRole("button", { name: "New Chat" })).toBeVisible();
    await expect(page.getByRole("button", { name: "rust-agent" })).toBeVisible();
    await expect(page.getByLabel("Current session")).toContainText("Continuing");
    await expect(page.getByLabel("Current session")).toContainText("Desktop app Phase 1");
    await expect(page.locator(".startup-state-card")).toContainText("Restored session");
    await expect(page.locator(".startup-state-card")).toContainText("Desktop app Phase 1");
    await page.getByRole("button", { name: "Environment information" }).click();
    await expect(page.getByRole("complementary", { name: "Environment details" })).toBeVisible();
    await expect(page.getByRole("complementary", { name: "Environment details" })).toContainText("rust-agent");
    await expect(page.getByRole("complementary", { name: "Environment details" })).toContainText("Permission mode");
    await page.getByRole("button", { name: "Environment information" }).click();
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
    await expect(page.getByLabel("Current session")).toContainText("Daily desktop flow");

    await page.locator(".recent-item-main", { hasText: "Daily desktop flow" }).click();
    await expect(page.getByText("Loaded preview session: web-preview")).toBeVisible();

    await page.getByRole("button", { name: "New Chat" }).click();
    await expect(page.getByText("Loaded preview session: web-preview")).not.toBeVisible();
    await expect(page.getByLabel("Current session")).toContainText("New conversation");
    await expect(page.getByLabel("Current session")).toContainText("No active session");
    await expect(page.locator(".startup-state-card")).toContainText("New conversation");
    await expect(
      page.locator(".empty-state").getByRole("heading", { name: "What should we build in rust-agent?" }),
    ).toBeVisible();
    await expect(page.locator(".composer")).toHaveClass(/empty-composer/);
    await expect(page.getByPlaceholder("Ask anything")).toBeVisible();
    await page.locator(".recent-item", { hasText: "Daily desktop flow" }).hover();
    await page.getByRole("button", { name: /Delete Daily desktop flow/ }).click();
    await expect(page.getByRole("dialog", { name: "Delete session?" })).toBeVisible();
    await expect(page.getByRole("dialog")).toContainText("Daily desktop flow");
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

    await page.getByRole("textbox", { name: "Message" }).fill("Inspect the desktop timeline UI");
    await page.getByRole("button", { name: "Send message" }).click();
    await expect(page.locator(".composer")).not.toHaveClass(/empty-composer/);
    await expect(page.locator(".timeline-summary.run.completed", { hasText: "Run completed" })).toBeVisible();
    await expect(page.locator(".timeline-section-label", { hasText: "Process" })).toBeVisible();
    await expect(page.locator(".timeline-event.run-group-start", { hasText: "Agent run" })).toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "3 tools" })).toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "1 failed" })).toBeVisible();
    await expect(page.locator(".timeline-run-stats span", { hasText: "1 file changed" })).toBeVisible();
    await expect(page.locator(".message.assistant.final", { hasText: "Final answer" })).toBeVisible();
    await expect(page.locator(".message.assistant.final .message-section-label", { hasText: "Conclusion" })).toBeVisible();
    await expect(page.locator(".message.assistant.final")).toHaveClass(/run-group-final/);
    const pnpmCard = page.locator(".timeline-event.compact-shell", { hasText: "Pnpm Test" });
    await expect(pnpmCard).toBeVisible();
    await expect(pnpmCard).toHaveClass(/run-group-step/);
    await expect(
      pnpmCard.locator(".timeline-summary code", {
        hasText: "corepack pnpm --dir apps/desktop test:ui-smoke",
      }),
    ).toBeVisible();
    await expect(pnpmCard.locator(".timeline-summary-meta", { hasText: "Pnpm Test" })).toBeVisible();
    await expect(pnpmCard.locator(".timeline-summary-meta", { hasText: "exit 0" })).toBeVisible();
    await expect(pnpmCard.locator(".timeline-facts span")).toHaveCount(0);
    await expect(page.locator(".timeline-title", { hasText: "Edited file" })).toBeVisible();
    await expect(
      page.locator(".timeline-summary.file .timeline-summary-meta", {
        hasText: "apps/desktop/src/app/runEventState.ts",
      }),
    ).toBeVisible();
    await expect(page.locator(".timeline-diff-preview", { hasText: "+  diff_preview?: string;" })).toBeVisible();
    await expect(page.locator(".timeline-facts span", { hasText: "2 replacements" })).toBeVisible();
    await expect(
      page.locator(".timeline-summary.failure strong", {
        hasText: "cargo test failed with exit code 101",
      }),
    ).toBeVisible();
    await expect(
      page.locator(".timeline-summary.failure .timeline-summary-meta", {
        hasText: "Inspect the failing test output",
      }),
    ).toBeVisible();
    await expect(page.locator(".timeline-expandable-preview summary", { hasText: "Output preview" })).toBeVisible();
    await page.locator(".timeline-expandable-preview summary", { hasText: "Output preview" }).click();
    await expect(page.locator(".timeline-output-preview", { hasText: "timeline_cards_show_diff_preview" })).toBeVisible();
    await expect(page.locator(".timeline-event.usage", { hasText: "Token usage" })).toBeVisible();
    await expect(page.locator(".timeline-event.permission", { hasText: "Allow git push" })).toBeVisible();
    await page.locator(".timeline-event.permission .timeline-actions button", { hasText: "Approve" }).click();
    await expect(page.locator(".timeline-event.permission", { hasText: "Permission approved" })).toBeVisible();
    await page.locator(".timeline-event", { hasText: "Pnpm Test" }).getByRole("button", { name: "Open trace for Pnpm Test" }).click();
    await expect(page.getByRole("complementary", { name: "Run trace" })).toBeVisible();
    await expect(page.locator(".trace-item.active", { hasText: "Tool completed" })).toBeVisible();
    await page.getByRole("complementary", { name: "Run trace" }).getByRole("button", { name: "Close" }).click();

    await assertNoHorizontalOverflow(page);
    await assertStableVerticalStack(page, [
      ".topbar",
      ".diagnostics-panel",
      ".transcript",
      ".composer",
    ]);

    await page.screenshot({
      path: testInfo.outputPath("desktop-main.png"),
      fullPage: true,
    });

    await page.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByRole("complementary", { name: "Settings" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Back to app" })).toBeVisible();
    const settingsNav = page.getByLabel("Settings categories");
    await expect(settingsNav.getByRole("button", { name: "General" })).toHaveClass(/active/);
    await expect(page.getByText("Work mode")).toBeVisible();
    await expect(page.getByRole("button", { name: /Daily work/ })).toHaveClass(/active/);
    await page.getByRole("button", { name: /Coding/ }).click();
    await expect(page.getByRole("button", { name: /Coding/ })).toHaveClass(/active/);
    await page.getByRole("button", { name: /Daily work/ }).click();
    await expect(page.getByRole("button", { name: /Daily work/ })).toHaveClass(/active/);
    await expect(page.getByText("Active session", { exact: true })).toBeVisible();
    await expect(page.locator(".settings-project-list")).toContainText("rust-agent");
    await expect(page.locator(".settings-project-list")).toContainText("bioclaw");
    await settingsNav.getByRole("button", { name: "Permissions" }).click();
    await expect(settingsNav.getByRole("button", { name: "Permissions" })).toHaveClass(/active/);
    await expect(page.getByText("Permission defaults")).toBeVisible();
    await page.getByRole("button", { name: /Auto low risk/ }).click();
    await expect(page.getByRole("button", { name: /Auto low risk/ })).toHaveClass(/active/);
    await settingsNav.getByRole("button", { name: "Provider" }).click();
    await expect(page.getByText("Provider setup")).toBeVisible();
    await settingsNav.getByRole("button", { name: "Diagnostics" }).click();
    await expect(
      page.getByRole("complementary", { name: "Settings" }).locator(".settings-diagnostic", {
        hasText: "Provider keys",
      }),
    ).toBeVisible();

    await assertNoHorizontalOverflow(page);
    await page.screenshot({
      path: testInfo.outputPath("desktop-settings.png"),
      fullPage: true,
    });
  });

  test("mobile layout keeps composer controls inside viewport", async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "What should we build in rust-agent?" }))
      .toBeVisible();
    await expect(page.getByLabel("Provider")).toBeVisible();
    await expect(page.getByLabel("Model")).toBeVisible();

    await assertNoHorizontalOverflow(page);
    await page.screenshot({
      path: testInfo.outputPath("mobile-main.png"),
      fullPage: true,
    });
  });
});

async function assertNoHorizontalOverflow(page: Page) {
  const result = await page.evaluate(() => {
    const root = document.documentElement;
    const overflowing = Array.from(document.querySelectorAll<HTMLElement>("body *"))
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
