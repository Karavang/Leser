import { execFileSync } from "node:child_process";
import { writeFile } from "node:fs/promises";

const API = process.env.E2E_API ?? "http://localhost:4444";
const ORIGIN = "http://localhost:5199";
// ponytail: fixed test account on the local dev stack only; never point E2E_API at prod.
const USER = { username: "e2e", email: "e2e@leser.test", password: "e2e-password-123" };
const EMPTY = { username: "e2e-empty", email: "e2e-empty@leser.test", password: "e2e-password-123" };
const READER = { username: "e2e-reader", email: "e2e-reader@leser.test", password: "e2e-password-123" };
const DB = process.env.E2E_DB ?? "leser-db-1";

// ponytail: roles go straight into the dev database, since only an admin can grant one and the first admin has to come from somewhere.
const setRole = (email, role) =>
  execFileSync("docker", [
    "exec", DB, "psql", "-U", "leser", "-d", "leser", "-c",
    `update users set role = '${role}' where email = '${email}'`,
  ]);

export const CHAPTERS = ["Глава первая", "Глава вторая", "Глава третья"];

const paragraph = (n) =>
  `<p>Абзац ${n}. Мобильная читалка должна листать страницы, показывать шапку и закрывать оглавление. ` +
  "Этот текст нужен только для того, чтобы глава заняла несколько экранов телефона.</p>";

const fb2 = (title = "E2E мобильная книга") => `<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
<description><title-info><author><first-name>Тест</first-name><last-name>Мобильный</last-name></author>
<book-title>${title}</book-title><lang>ru</lang></title-info></description>
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
  setRole(USER.email, "admin");
  const login = await call("/login", json({ email: USER.email, password: USER.password }));
  const user = login.body?.data?.user;
  if (!user?.token) throw new Error(`e2e login failed: ${login.status} ${JSON.stringify(login.body)}`);

  const auth = { Authorization: `Bearer ${user.token}` };
  const put = async (title, file, headers = auth) => {
    const form = new FormData();
    form.append("file", new Blob([fb2(title)]), file);
    const upload = await call("/putNewOne?force=true", { method: "POST", headers, body: form });
    const book = upload.body?.book;
    if (!book?.id) throw new Error(`e2e upload failed: ${upload.status} ${JSON.stringify(upload.body)}`);
    return book;
  };
  const flip = (book, page) =>
    call("/pageWasFlipped", {
      method: "POST",
      headers: { ...auth, "content-type": "application/json" },
      body: JSON.stringify({ book: book.id, page }),
    });

  const book = await put("E2E мобильная книга", "e2e-mobile.fb2");
  const older = await put("E2E вторая книга", "e2e-second.fb2");
  await flip(older, "0.5");
  await new Promise((r) => setTimeout(r, 50));
  await flip(book, "0");

  await call("/registration", json(EMPTY));
  setRole(EMPTY.email, "full");
  const empty = (await call("/login", json({ email: EMPTY.email, password: EMPTY.password }))).body?.data?.user;
  if (!empty?.token) throw new Error("e2e login of the empty user failed");
  await writeFile("e2e/.empty.json", JSON.stringify(empty));
  const foreign = await put("E2E чужая книга", "e2e-foreign.fb2", { Authorization: `Bearer ${empty.token}` });
  await call(`/myBooks/${foreign.id}`, { method: "DELETE", headers: { Authorization: `Bearer ${empty.token}` } });

  await call(`/admin/books/${older.id}/public`, {
    method: "POST",
    headers: { ...auth, "content-type": "application/json" },
    body: JSON.stringify({ public: true }),
  });
  await call("/registration", json(READER));
  setRole(READER.email, "user");
  const reader = (await call("/login", json({ email: READER.email, password: READER.password }))).body?.data?.user;
  if (!reader?.token) throw new Error("e2e login of the plain reader failed");
  await writeFile("e2e/.reader.json", JSON.stringify(reader));

  await writeFile("e2e/.book.json", JSON.stringify(book));
  await writeFile("e2e/.older.json", JSON.stringify(older));
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
