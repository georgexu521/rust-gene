import { expect, test, type Page } from "@playwright/test";

test.describe("desktop UI smoke", () => {
  test("desktop layout renders core controls and settings drawer", async ({ page }, testInfo) => {
    await page.goto("/");

    await expect(page.getByRole("heading", { name: "What should we build in rust-agent?" }))
      .toBeVisible();
    await expect(page.getByText("Environment diagnostics")).toBeVisible();
    await expect(page.getByLabel("Provider")).toBeVisible();
    await expect(page.getByLabel("Model")).toBeVisible();

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
