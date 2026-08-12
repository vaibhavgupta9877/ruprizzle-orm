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
    age INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL
  );
  CREATE TABLE categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
  );
  CREATE TABLE posts (
    id INTEGER PRIMARY KEY,
    author_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    published_at INTEGER NOT NULL,
    views INTEGER NOT NULL
  );
  CREATE TABLE comments (
    id INTEGER PRIMARY KEY,
    post_id INTEGER NOT NULL,
    author_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL
  );
  CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
  );
  CREATE TABLE post_tags (
    post_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (post_id, tag_id)
  );
  CREATE TABLE followers (
    follower_id INTEGER NOT NULL,
    followee_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (follower_id, followee_id)
  );
  CREATE TABLE likes (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    post_id INTEGER NOT NULL,
    created_at INTEGER NOT NULL
  );
  CREATE TABLE bench_bulk (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    n INTEGER NOT NULL
  );
`);

const USER_COUNT = 1000;
const CATEGORY_COUNT = 20;
const POSTS_PER_USER = 10;
const COMMENTS_PER_POST = 5;
const TAG_COUNT = 100;
const TAGS_PER_POST = 3;
const FOLLOWER_COUNT = 5000;
const LIKE_COUNT = 20000;

const insertUser = db.prepare('INSERT INTO users (id, email, age, name, created_at) VALUES (?, ?, ?, ?, ?)');
const insertCategory = db.prepare('INSERT INTO categories (id, name) VALUES (?, ?)');
const insertPost = db.prepare('INSERT INTO posts (id, author_id, category_id, title, published_at, views) VALUES (?, ?, ?, ?, ?, ?)');
const insertComment = db.prepare('INSERT INTO comments (id, post_id, author_id, content, created_at) VALUES (?, ?, ?, ?, ?)');
const insertTag = db.prepare('INSERT INTO tags (id, name) VALUES (?, ?)');
const insertPostTag = db.prepare('INSERT INTO post_tags (post_id, tag_id) VALUES (?, ?)');
const insertFollower = db.prepare('INSERT INTO followers (follower_id, followee_id, created_at) VALUES (?, ?, ?)');
const insertLike = db.prepare('INSERT INTO likes (id, user_id, post_id, created_at) VALUES (?, ?, ?, ?)');

const now = 1700000000;

const insertUsers = db.transaction(() => {
  for (let i = 1; i <= USER_COUNT; i++) {
    insertUser.run(i, `user-${i}@example.com`, 18 + (i % 50), `User ${i}`, now + i);
  }
});
insertUsers();

const insertCategories = db.transaction(() => {
  for (let i = 1; i <= CATEGORY_COUNT; i++) {
    insertCategory.run(i, `category-${i}`);
  }
});
insertCategories();

const insertPosts = db.transaction(() => {
  let postId = 1;
  for (let authorId = 1; authorId <= USER_COUNT; authorId++) {
    for (let j = 0; j < POSTS_PER_USER; j++) {
      const categoryId = ((postId - 1) % CATEGORY_COUNT) + 1;
      const title = `post-${postId}`;
      const publishedAt = now + postId;
      const views = postId * 7;
      insertPost.run(postId, authorId, categoryId, title, publishedAt, views);
      postId++;
    }
  }
});
insertPosts();

const insertComments = db.transaction(() => {
  let commentId = 1;
  for (let postId = 1; postId <= USER_COUNT * POSTS_PER_USER; postId++) {
    for (let j = 0; j < COMMENTS_PER_POST; j++) {
      const authorId = ((commentId - 1) % USER_COUNT) + 1;
      const content = `comment-${commentId}`;
      insertComment.run(commentId, postId, authorId, content, now + commentId);
      commentId++;
    }
  }
});
insertComments();

const insertTags = db.transaction(() => {
  for (let i = 1; i <= TAG_COUNT; i++) {
    insertTag.run(i, `tag-${i}`);
  }
});
insertTags();

const insertPostTags = db.transaction(() => {
  const postCount = USER_COUNT * POSTS_PER_USER;
  for (let postId = 1; postId <= postCount; postId++) {
    for (let j = 0; j < TAGS_PER_POST; j++) {
      const tagId = ((postId + j - 1) % TAG_COUNT) + 1;
      insertPostTag.run(postId, tagId);
    }
  }
});
insertPostTags();

const insertFollowers = db.transaction(() => {
  for (let i = 1; i <= FOLLOWER_COUNT; i++) {
    const followerId = ((i - 1) % USER_COUNT) + 1;
    const floor = Math.floor((i - 1) / USER_COUNT);
    const followeeId = ((floor + i) % USER_COUNT) + 1;
    if (followerId !== followeeId) {
      insertFollower.run(followerId, followeeId, now + i);
    }
  }
});
insertFollowers();

const POST_COUNT = USER_COUNT * POSTS_PER_USER;
const LIKES_PER_USER = LIKE_COUNT / USER_COUNT;

const insertLikes = db.transaction(() => {
  for (let i = 1; i <= LIKE_COUNT; i++) {
    const postId = ((i * 13 - 1) % 1000) + 1;
    const userId = ((i * 17 - 1) % USER_COUNT) + 1;
    const authorId = Math.floor((postId - 1) / POSTS_PER_USER) + 1;
    if (userId !== authorId) {
      insertLike.run(i, userId, postId, now + i);
    }
  }
});
insertLikes();

db.close();
console.log(
  `Seeded ${DB_PATH}: ${USER_COUNT} users, ${CATEGORY_COUNT} categories, ` +
  `${USER_COUNT * POSTS_PER_USER} posts, ${USER_COUNT * POSTS_PER_USER * COMMENTS_PER_POST} comments, ` +
  `${TAG_COUNT} tags, ${USER_COUNT * POSTS_PER_USER * TAGS_PER_POST} post_tags, ` +
  `${FOLLOWER_COUNT} followers, ${LIKE_COUNT} likes.`
);
