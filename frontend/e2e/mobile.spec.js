import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { CHAPTERS } from "./setup.js";

const API = process.env.E2E_API ?? "http://localhost:4444";
const book = () => JSON.parse(readFileSync("e2e/.book.json", "utf8"));
const older = () => JSON.parse(readFileSync("e2e/.older.json", "utf8"));

const fitsWidth = async (page) =>
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth),
  ).toBeLessThanOrEqual(0);

const openBook = async (page) => {
  await page.goto(`/book/${book().id}?at=0`);
  await expect(page.locator(".readerPages section").first()).toBeVisible();
};

const pageNo = async (page) =>
  Number((await page.locator(".readerFoot span").textContent()).split("/")[0].trim());

const tapAt = async (page, fx, fy) => {
  const box = await page.locator(".readerViewport").boundingBox();
  await page.touchscreen.tap(box.x + box.width * fx, box.y + box.height * fy);
};

const hudShown = (page) => expect(page.locator(".reader")).not.toHaveClass(/\bbare\b/);
const hudHidden = async (page) => {
  await expect(page.locator(".reader")).toHaveClass(/\bbare\b/);
  await expect
    .poll(async () => {
      const bar = await page.locator(".readerBar").boundingBox();
      return bar.y + bar.height;
    })
    .toBeLessThanOrEqual(1);
};

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

  test("profile scrolls down to the settings and past them", async ({ page }) => {
    await page.goto("/me");
    const settings = page.locator("details.settings");
    await expect(settings).toBeAttached();
    const scrollTo = (loc) =>
      loc.evaluate((el) => {
        for (let box = el.parentElement; box; box = box.parentElement) {
          const { overflowY } = getComputedStyle(box);
          if (/auto|scroll/.test(overflowY) && box.scrollHeight > box.clientHeight) {
            box.scrollTop += el.getBoundingClientRect().top - box.getBoundingClientRect().top;
          }
        }
        window.scrollBy(0, el.getBoundingClientRect().top - 40);
        const r = el.getBoundingClientRect();
        return r.top >= 0 && r.bottom <= window.innerHeight;
      });

    expect(await scrollTo(settings.locator("summary"))).toBe(true);
    await settings.locator("summary").click();
    expect(await scrollTo(settings.locator(".settingsRow").last())).toBe(true);
  });

  for (const width of [320, 360, 375, 390, 414]) {
    test(`header icons stay whole at ${width}px`, async ({ page }) => {
      await page.setViewportSize({ width, height: 700 });
      await page.goto("/");
      const parts = [page.locator(".searchingLine"), page.getByTitle("Добавить книги").first()];
      const boxes = [];
      for (const part of parts) {
        await expect(part).toBeVisible();
        boxes.push(await part.boundingBox());
      }
      for (const box of boxes.slice(1)) {
        expect(box.width).toBeGreaterThanOrEqual(28);
        expect(box.height).toBeGreaterThanOrEqual(28);
      }
      for (const box of boxes) {
        expect(box.x).toBeGreaterThanOrEqual(0);
        expect(box.x + box.width).toBeLessThanOrEqual(width);
      }
      for (let i = 1; i < boxes.length; i++) {
        expect(boxes[i].x).toBeGreaterThanOrEqual(boxes[i - 1].x + boxes[i - 1].width);
      }
      const profile = page.getByRole("button", { name: "Профиль" }).filter({ visible: true }).first();
      await expect(profile).toBeVisible();
      const p = await profile.boundingBox();
      expect(p.x + p.width).toBeLessThanOrEqual(width);
      await fitsWidth(page);
    });
  }

  test("an empty shelf opens the library, and search looks through all of it", async ({ page }) => {
    const empty = JSON.parse(readFileSync("e2e/.empty.json", "utf8"));
    await page.addInitScript((user) => {
      localStorage.setItem("token", JSON.stringify(user.token));
      localStorage.setItem("user", JSON.stringify(user));
    }, empty);
    await page.goto("/");
    await expect(page.locator(".bookList li h2").getByText(book().title, { exact: true })).toBeVisible();
    await expect(page.locator(".continue")).toHaveCount(0);
  });

  test("search on my books looks through the whole library", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".bookList li h2").first()).toBeVisible();
    await page.locator(".searchingLine").fill("чужая");
    await expect(page.locator(".bookList li h2").getByText("E2E чужая книга")).toBeVisible();
  });

  test("my books open by default, most recently read first", async ({ page }) => {
    await page.goto("/");
    const titles = page.locator(".bookList li h2");
    await expect(titles.first()).toBeVisible();
    const shown = await titles.allTextContents();
    const mine = shown.filter((t) => t.startsWith("E2E"));
    expect(mine).toEqual([book().title, older().title]);
    await expect(page.locator(".continue button").first()).toContainText(book().title);
  });
});

test.describe("roles", () => {
  const as = async (page, file) => {
    const user = JSON.parse(readFileSync(file, "utf8"));
    await page.addInitScript((user) => {
      localStorage.setItem("token", JSON.stringify(user.token));
      localStorage.setItem("user", JSON.stringify(user));
    }, user);
    return user;
  };

  test("a plain reader sees only public books and can't open the rest", async ({ page }) => {
    const reader = await as(page, "e2e/.reader.json");
    await page.goto("/");
    const titles = page.locator(".bookList li h2");
    await expect(titles.getByText(older().title, { exact: true })).toBeVisible();
    await expect(titles.getByText(book().title, { exact: true })).toHaveCount(0);
    await expect(titles.getByText("E2E чужая книга")).toHaveCount(0);
    const read = await page.request.get(`${API}/read/${book().id}`, {
      headers: { Authorization: `Bearer ${reader.token}` },
    });
    expect(read.status()).toBe(404);
  });

  test("an admin makes a one-time invite that grants the whole library", async ({ page }) => {
    await page.goto("/me");
    await page.locator("details.admin summary").click();
    const created = page.waitForResponse((r) => r.url().endsWith("/admin/invites") && r.request().method() === "POST");
    await page.getByRole("button", { name: "Создать ссылку-приглашение" }).click();
    const { token: invite } = await (await created).json();
    await expect(page.locator(`details.admin input[value$="${invite}"]`)).toBeVisible();

    const email = `invitee-${test.info().project.name}-${Date.now()}@leser.test`;
    const signUp = (address) =>
      page.request.post(`${API}/registration`, {
        data: { username: "invitee", email: address, password: "e2e-password-123", invite },
      });
    const first = await signUp(email);
    expect(first.status()).toBe(201);
    const invitee = (await first.json()).data.user;
    expect(invitee.role).toBe("full");
    expect((await signUp(`again-${email}`)).status()).toBe(400);
    await page.request.delete(`${API}/account`, { headers: { Authorization: `Bearer ${invitee.token}` } });
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
    await expect
      .poll(async () => (await page.locator(".readerBar").boundingBox()).y)
      .toBeGreaterThanOrEqual(0);
    await tapAt(page, 0.5, 0.5);
    await hudHidden(page);
  });

  test("swipes flip pages", async ({ page, browserName }) => {
    // ponytail: WebKit has no `new Touch()`, so swipes are only checked in Chromium; tap zones cover iPhones.
    test.skip(browserName === "webkit", "no synthetic Touch in WebKit");
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
    const items = page.locator(".readerToc ol button span");
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
    await tapAt(page, 0.05, 0.12);
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
