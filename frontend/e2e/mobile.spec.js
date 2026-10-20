import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { CHAPTERS } from "./setup.js";

const book = () => JSON.parse(readFileSync("e2e/.book.json", "utf8"));

const fitsWidth = async (page) =>
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth),
  ).toBeLessThanOrEqual(0);

const openBook = async (page) => {
  await page.goto(`/book/${book().id}`);
  await expect(page.locator(".readerPages section").first()).toBeVisible();
};

const pageNo = async (page) =>
  Number((await page.locator(".readerFoot span").textContent()).split("/")[0].trim());

const tapAt = async (page, fx, fy) => {
  const box = await page.locator(".readerViewport").boundingBox();
  await page.touchscreen.tap(box.x + box.width * fx, box.y + box.height * fy);
};

const hudShown = (page) => expect(page.locator(".reader")).not.toHaveClass(/\bbare\b/);
const hudHidden = (page) => expect(page.locator(".reader")).toHaveClass(/\bbare\b/);

const swipe = (page, dx) =>
  page.locator(".readerViewport").evaluate((el, dx) => {
    const box = el.getBoundingClientRect();
    const x = box.left + box.width / 2;
    const y = box.top + box.height / 2;
    const touch = (clientX) => new Touch({ identifier: 1, target: el, clientX, clientY: y });
    el.dispatchEvent(
      new TouchEvent("touchstart", { bubbles: true, changedTouches: [touch(x)], touches: [touch(x)] }),
    );
    el.dispatchEvent(new TouchEvent("touchend", { bubbles: true, changedTouches: [touch(x + dx)] }));
  }, dx);

test.describe("library", () => {
  test("fits the screen and opens a book", async ({ page }) => {
    await page.goto("/");
    const card = page.getByText(book().title).first();
    await expect(card).toBeVisible();
    await fitsWidth(page);
    await card.click();
    await expect(page).toHaveURL(/\/book\//);
  });

  test("upload dialog fits the screen and closes", async ({ page }) => {
    await page.goto("/");
    await page.getByTitle("Добавить книги").first().click();
    const card = page.locator(".modalAdd .modalCard");
    await expect(card).toBeVisible();
    const box = await card.boundingBox();
    const view = page.viewportSize();
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(view.width);
    await page.getByRole("button", { name: "Закрыть" }).click();
    await expect(card).toBeHidden();
  });

  test("profile page fits the screen", async ({ page }) => {
    await page.goto("/me");
    await expect(page.locator("body")).not.toBeEmpty();
    await fitsWidth(page);
  });
});

test.describe("reader", () => {
  test("tap zones flip pages and toggle the HUD", async ({ page }) => {
    await openBook(page);
    await fitsWidth(page);
    await hudHidden(page);

    await tapAt(page, 0.85, 0.5);
    await expect.poll(() => pageNo(page)).toBe(2);
    await tapAt(page, 0.1, 0.5);
    await expect.poll(() => pageNo(page)).toBe(1);

    await tapAt(page, 0.5, 0.05);
    await hudShown(page);
    expect((await page.locator(".readerBar").boundingBox()).y).toBeGreaterThanOrEqual(0);
    await tapAt(page, 0.5, 0.5);
    await hudHidden(page);
  });

  test("swipes flip pages", async ({ page }) => {
    await openBook(page);
    await swipe(page, -120);
    await expect.poll(() => pageNo(page)).toBe(2);
    await swipe(page, 120);
    await expect.poll(() => pageNo(page)).toBe(1);
  });

  test("contents list real chapter names", async ({ page }) => {
    await openBook(page);
    await tapAt(page, 0.5, 0.05);
    await page.getByTitle("Оглавление").click();
    const items = page.locator(".readerToc ol button");
    await expect(items).toHaveText(CHAPTERS);
  });

  test("contents sit above the HUD and always close", async ({ page }) => {
    await openBook(page);
    const toc = page.locator(".readerToc");
    const openToc = async () => {
      await hudShown(page);
      await page.getByTitle("Оглавление").click();
      await expect(toc).toBeVisible();
    };

    await tapAt(page, 0.5, 0.05);
    await openToc();
    const head = await toc.locator("header").boundingBox();
    const onTop = await page.evaluate(
      ([x, y]) => Boolean(document.elementFromPoint(x, y)?.closest(".readerToc")),
      [head.x + head.width / 2, head.y + head.height / 2],
    );
    expect(onTop).toBe(true);

    await toc.getByRole("button", { name: "Закрыть" }).click();
    await expect(toc).toBeHidden();

    await openToc();
    await tapAt(page, 0.05, 0.5);
    await expect(toc).toBeHidden();
    await expect.poll(() => pageNo(page)).toBe(1);

    await openToc();
    await toc.getByRole("button", { name: CHAPTERS[2] }).click();
    await expect(toc).toBeHidden();
    await expect(page.locator(".readerWhat span")).toHaveText(CHAPTERS[2]);
    await hudHidden(page);

    await tapAt(page, 0.5, 0.05);
    await hudShown(page);
  });

  test("font and theme controls work", async ({ page }) => {
    await openBook(page);
    await tapAt(page, 0.5, 0.05);
    const size = () =>
      page.locator(".readerPages").evaluate((el) => parseFloat(el.style.getPropertyValue("--fs")));
    const before = await size();
    await page.getByTitle("Больше шрифт").click();
    await expect.poll(size).toBeGreaterThan(before);
    await page.getByTitle("Ночь").click();
    await expect(page.locator(".reader")).toHaveClass(/\bnight\b/);
    await fitsWidth(page);
  });
});
