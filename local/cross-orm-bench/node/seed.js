import Database from 'better-sqlite3';
import fs from 'node:fs';
import path from 'node:path';

const DB_PATH = path.join(import.meta.dirname, 'bench.sqlite3');

// Remove any SQLite side-car files as well as the main database. This keeps
// the seeded file clean even when the drivers switch the database to WAL mode.
for (const suffix of ['', '-wal', '-shm', '-journal']) {
  const p = `${DB_PATH}${suffix}`;
  if (fs.existsSync(p)) {
    fs.unlinkSync(p);
  }
}

const db = new Database(DB_PATH);

db.exec(`
  CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    email TEXT NOT NULL,
    age INTEGER NOT NULL
  );
  CREATE TABLE posts (
    id INTEGER PRIMARY KEY,
    author_id INTEGER NOT NULL,
    title TEXT NOT NULL
  );
  CREATE TABLE bench_bulk (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    n INTEGER NOT NULL
  );
`);

const insertUser = db.prepare('INSERT INTO users (id, email, age) VALUES (?, ?, ?)');
const insertPost = db.prepare('INSERT INTO posts (id, author_id, title) VALUES (?, ?, ?)');

const insertUsers = db.transaction(() => {
  for (let i = 1; i <= 1000; i++) {
    insertUser.run(i, `user-${i}@example.com`, 18 + (i % 50));
  }
});
insertUsers();

const insertPosts = db.transaction(() => {
  for (let i = 1; i <= 10000; i++) {
    const authorId = ((i - 1) % 1000) + 1;
    insertPost.run(i, authorId, `post-${i}`);
  }
});
insertPosts();

db.close();
console.log(`Seeded ${DB_PATH} with 1000 users and 10000 posts.`);
