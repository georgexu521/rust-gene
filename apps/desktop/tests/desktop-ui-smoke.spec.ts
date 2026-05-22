import { expect, test, type Page } from "@playwright/test";

test.describe("desktop UI smoke", () => {
  test("desktop layout renders core controls and settings drawer", async ({ page }, testInfo) => {
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "What should we build in rust-agent?" }))
      .toBeVisible();
    await expect(page.getByText("Environment diagnostics")).toBeVisible();
    await expect(page.getByLabel("Provider")).toBeVisible();
    await expect(page.getByLabel("Model")).toBeVisible();
    await expect(page.getByRole("button", { name: "New Chat" })).toBeVisible();
    await expect(page.getByRole("button", { name: "rust-agent" })).toBeVisible();

    await page.getByLabel("Search sessions").fill("Release");
    await expect(page.getByText("Release readiness notes")).toBeVisible();
    await expect(page.getByText("Desktop app Phase 1")).not.toBeVisible();
    await page.locator(".recent-item", { hasText: "Release readiness notes" }).hover();
    await page.getByRole("button", { name: /Archive Release readiness notes/ }).click();
    await expect(page.getByText("No matching sessions")).toBeVisible();
    await page.getByLabel("Search sessions").fill("");

    await page.getByRole("button", { name: /Rename Desktop app Phase 1/ }).click();
    await page.getByLabel("Session name").fill("Daily desktop flow");
    await page.getByRole("button", { name: "Save session title" }).click();
    await expect(page.getByText("Daily desktop flow")).toBeVisible();

    await page.locator(".recent-item-main", { hasText: "Daily desktop flow" }).click();
    await expect(page.getByText("Loaded preview session: web-preview")).toBeVisible();

    await page.getByRole("button", { name: "New Chat" }).click();
    await expect(page.getByText("Loaded preview session: web-preview")).not.toBeVisible();
    await expect(page.getByRole("heading", { name: "Start a focused run in rust-agent" })).toBeVisible();
    await page.locator(".recent-item", { hasText: "Daily desktop flow" }).hover();
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: /Delete Daily desktop flow/ }).click();
    await expect(page.getByText("Daily desktop flow")).not.toBeVisible();

    await page.getByRole("textbox", { name: "Message" }).fill("Inspect the desktop timeline UI");
    await page.getByRole("button", { name: "Send message" }).click();
    await expect(page.locator(".timeline-title", { hasText: "Pnpm Test" })).toBeVisible();
    await expect(
      page.locator(".timeline-summary code", {
        hasText: "corepack pnpm --dir apps/desktop test:ui-smoke",
      }),
    ).toBeVisible();
    await expect(page.getByText("validation pnpm_test")).toBeVisible();
    await expect(page.locator(".timeline-facts span", { hasText: "exit 0" })).toBeVisible();
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
    await expect(page.locator(".timeline-event.permission", { hasText: "Allow git push" })).toBeVisible();
    await page.locator(".timeline-event.permission .timeline-actions button", { hasText: "Approve" }).click();
    await expect(page.locator(".timeline-event.permission", { hasText: "Permission approved" })).toBeVisible();
    await page.locator(".timeline-event", { hasText: "Pnpm Test" }).getByRole("button", { name: "Debug" }).click();
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
    await expect(page.getByText("Provider setup")).toBeVisible();
    await expect(page.getByText("Permission defaults")).toBeVisible();

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
