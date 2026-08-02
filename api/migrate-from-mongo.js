// Одноразовый перенос Mongo -> Postgres. Запустить один раз и удалить.
//   npm i mongoose pg argon2 @aws-sdk/client-s3
//   MONGO=... DATABASE_URL=... S3_BUCKET=... node migrate-from-mongo.js
//
// Пароли в Mongo лежат открытым текстом — здесь они хешируются argon2,
// поэтому никого не придётся заставлять сбрасывать пароль.
// Файлы книг переезжают из S3 в book_files. Из бакета только читаем:
// ничего не удаляется, откат — это truncate в Postgres.

const mongoose = require("mongoose");
const { Client } = require("pg");
const argon2 = require("argon2");
const { S3Client, GetObjectCommand } = require("@aws-sdk/client-s3");
const crypto = require("crypto");

const { MONGO, DATABASE_URL, S3_BUCKET, AWS_REGION = "eu-west-3" } = process.env;
const DRY = process.argv.includes("--dry-run");

const main = async () => {
  for (const k of ["MONGO", "DATABASE_URL", "S3_BUCKET"]) {
    if (!process.env[k]) throw new Error(`missing env ${k}`);
  }

  await mongoose.connect(MONGO);
  const db = mongoose.connection.db;
  const pg = new Client({ connectionString: DATABASE_URL });
  await pg.connect();
  const s3 = new S3Client({ region: AWS_REGION });

  const { rows } = await pg.query("select count(*)::int n from books");
  if (rows[0].n > 0) throw new Error("books не пуста — миграция уже проходила");

  const mongoUsers = await db.collection("lesers").find().toArray();
  const mongoBooks = await db.collection("books").find().toArray();
  console.log(`mongo: ${mongoUsers.length} юзеров, ${mongoBooks.length} книг`);

  // email -> user_id, чтобы владельцы и прогресс сослались на новые id
  const userId = new Map();
  for (const u of mongoUsers) {
    const email = (u.email || "").trim().toLowerCase();
    if (!email) {
      console.warn(`  пропуск юзера ${u._id}: нет email`);
      continue;
    }
    const hash = await argon2.hash(String(u.password ?? crypto.randomUUID()));
    const id = crypto.randomUUID();
    if (!DRY) {
      await pg.query(
        `insert into users (id, username, email, password_hash, is_admin)
         values ($1,$2,$3,$4,$5) on conflict (email) do nothing`,
        [id, u.username || email, email, hash, !!u.isAdmin],
      );
    }
    userId.set(String(u._id), id);
  }

  // старый filename -> новый {uuid}.{ext}
  const bookId = new Map();
  for (const b of mongoBooks) {
    if (!b.filename) {
      console.warn(`  пропуск книги ${b._id}: нет filename`);
      continue;
    }
    const ext = b.filename.split(".").pop().toLowerCase();
    const id = crypto.randomUUID();
    const owner = userId.get(String(b.owner)) ?? null;

    if (!DRY) {
      // Нет файла в бакете — нет и строки: книга без файла ломает читалку.
      let data;
      try {
        const obj = await s3.send(
          new GetObjectCommand({ Bucket: S3_BUCKET, Key: `books/${b.filename}` }),
        );
        data = Buffer.from(await obj.Body.transformToByteArray());
      } catch (e) {
        console.warn(`  пропуск книги ${b.filename}: нет в S3 (${e.name})`);
        continue;
      }
      await pg.query("begin");
      await pg.query(
        `insert into books (id, title, author, published, lang, description, ext, owner_id)
         values ($1,$2,$3,$4,$5,$6,$7,$8)`,
        [id, b.title || "Untitled", b.author || "", b.date || null, b.lang || null, b.desc || null, ext, owner],
      );
      await pg.query("insert into book_files (book_id, data) values ($1,$2)", [id, data]);
      await pg.query("commit");
    }
    bookId.set(b.filename, { id, ext });
  }

  let progress = 0;
  for (const u of mongoUsers) {
    const uid = userId.get(String(u._id));
    for (const p of u.pages || []) {
      const book = bookId.get(p.filename);
      if (!uid || !book) continue; // книгу удалили, а закладка осталась
      if (!DRY) {
        await pg.query(
          `insert into reading_progress (user_id, book_id, position) values ($1,$2,$3)
           on conflict do nothing`,
          [uid, book.id, String(p.page)],
        );
      }
      progress++;
    }
  }

  console.log(
    `${DRY ? "[dry-run] " : ""}перенесено: ${userId.size} юзеров, ${bookId.size} книг, ${progress} закладок`,
  );
  await pg.end();
  await mongoose.disconnect();
};

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
