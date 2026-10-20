import { writeFile } from "node:fs/promises";

const API = process.env.E2E_API ?? "http://localhost:4444";
const ORIGIN = "http://localhost:5199";
// ponytail: fixed test account on the local dev stack only; never point E2E_API at prod.
const USER = { username: "e2e", email: "e2e@leser.test", password: "e2e-password-123" };

export const CHAPTERS = ["Глава первая", "Глава вторая", "Глава третья"];

const paragraph = (n) =>
  `<p>Абзац ${n}. Мобильная читалка должна листать страницы, показывать шапку и закрывать оглавление. ` +
  "Этот текст нужен только для того, чтобы глава заняла несколько экранов телефона.</p>";

const fb2 = () => `<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
<description><title-info><author><first-name>Тест</first-name><last-name>Мобильный</last-name></author>
<book-title>E2E мобильная книга</book-title><lang>ru</lang></title-info></description>
<body>${CHAPTERS.map(
  (title, c) =>
    `<section><title><p>${title}</p></title>${Array.from({ length: 40 }, (_, i) => paragraph(c * 40 + i)).join("")}</section>`,
).join("")}</body></FictionBook>`;

const call = async (path, init) => {
  const res = await fetch(`${API}${path}`, init);
  return { status: res.status, body: await res.json().catch(() => null) };
};

const json = (body) => ({
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
});

export default async function setup() {
  await call("/registration", json(USER));
  const login = await call("/login", json({ email: USER.email, password: USER.password }));
  const user = login.body?.data?.user;
  if (!user?.token) throw new Error(`e2e login failed: ${login.status} ${JSON.stringify(login.body)}`);

  const form = new FormData();
  form.append("file", new Blob([fb2()]), "e2e-mobile.fb2");
  const upload = await call("/putNewOne?force=true", {
    method: "POST",
    headers: { Authorization: `Bearer ${user.token}` },
    body: form,
  });
  const book = upload.body?.book;
  if (!book?.id) throw new Error(`e2e upload failed: ${upload.status} ${JSON.stringify(upload.body)}`);

  await writeFile("e2e/.book.json", JSON.stringify(book));
  await writeFile(
    "e2e/.auth.json",
    JSON.stringify({
      cookies: [],
      origins: [
        {
          origin: ORIGIN,
          localStorage: [
            { name: "token", value: JSON.stringify(user.token) },
            { name: "user", value: JSON.stringify(user) },
            { name: "lang", value: "ru" },
          ],
        },
      ],
    }),
  );
}
