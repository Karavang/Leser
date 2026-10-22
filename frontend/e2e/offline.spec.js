import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";

const book = () => JSON.parse(readFileSync("e2e/.book.json", "utf8"));
const older = () => JSON.parse(readFileSync("e2e/.older.json", "utf8"));

test("losing the network switches to offline by itself, a downloaded book opens, the network coming back switches back", async ({
  page,
  context,
}, info) => {
  test.skip(info.project.name !== "android", "service worker flow is the same on every screen");
  await page.goto("/");
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.reload();

  const titles = page.locator(".bookList li h2");
  await page.locator(".bookList li", { hasText: book().title }).locator(".detailsButton").click();
  await page.getByRole("button", { name: "Скачать" }).click();
  await expect(page.getByRole("button", { name: "Скачана" })).toBeVisible();
  await page.getByRole("button", { name: "Закрыть" }).click();

  await context.setOffline(true);
  await page.evaluate(() => window.dispatchEvent(new Event("offline")));
  await expect(page.locator(".offlineBar")).toContainText("Связь пропала");
  await expect(page.locator(".offlineBar")).toContainText("Нет сети", { timeout: 25000 });
  await expect(titles.getByText(book().title, { exact: true })).toBeVisible();
  await expect(titles.getByText(older().title, { exact: true })).toHaveCount(0);

  await titles.getByText(book().title, { exact: true }).click();
  await expect(page.locator(".readerPages section").first()).toBeVisible();

  await expect(page.locator(".readerFoot")).toContainText("офлайн");

  await context.setOffline(false);
  await page.evaluate(() => window.dispatchEvent(new Event("online")));
  await expect(page.locator(".offlineBar")).toContainText("Снова в сети");
  await expect(page.locator(".offlineBar")).toHaveCount(0, { timeout: 5000 });
});
