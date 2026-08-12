import { integer, sqliteTable, text, primaryKey } from "drizzle-orm/sqlite-core";
import { relations } from "drizzle-orm";

export const users = sqliteTable("users", {
  id: integer("id").primaryKey(),
  email: text("email").notNull(),
  age: integer("age").notNull(),
  name: text("name").notNull(),
  createdAt: integer("created_at").notNull(),
});

export const categories = sqliteTable("categories", {
  id: integer("id").primaryKey(),
  name: text("name").notNull(),
});

export const posts = sqliteTable("posts", {
  id: integer("id").primaryKey(),
  authorId: integer("author_id").notNull(),
  categoryId: integer("category_id").notNull(),
  title: text("title").notNull(),
  publishedAt: integer("published_at").notNull(),
  views: integer("views").notNull(),
});

export const comments = sqliteTable("comments", {
  id: integer("id").primaryKey(),
  postId: integer("post_id").notNull(),
  authorId: integer("author_id").notNull(),
  content: text("content").notNull(),
  createdAt: integer("created_at").notNull(),
});

export const tags = sqliteTable("tags", {
  id: integer("id").primaryKey(),
  name: text("name").notNull(),
});

export const postTags = sqliteTable(
  "post_tags",
  {
    postId: integer("post_id").notNull(),
    tagId: integer("tag_id").notNull(),
  },
  (t) => ({
    pk: primaryKey({ columns: [t.postId, t.tagId] }),
  })
);

export const followers = sqliteTable(
  "followers",
  {
    followerId: integer("follower_id").notNull(),
    followeeId: integer("followee_id").notNull(),
    createdAt: integer("created_at").notNull(),
  },
  (t) => ({
    pk: primaryKey({ columns: [t.followerId, t.followeeId] }),
  })
);

export const likes = sqliteTable("likes", {
  id: integer("id").primaryKey(),
  userId: integer("user_id").notNull(),
  postId: integer("post_id").notNull(),
  createdAt: integer("created_at").notNull(),
});

export const benchBulk = sqliteTable("bench_bulk", {
  id: integer("id").primaryKey(),
  name: text("name").notNull(),
  n: integer("n").notNull(),
});

export const usersRelations = relations(users, ({ many }) => ({
  posts: many(posts),
  comments: many(comments),
  likes: many(likes),
  following: many(followers, { relationName: "following" }),
  followers: many(followers, { relationName: "followers" }),
}));

export const categoriesRelations = relations(categories, ({ many }) => ({
  posts: many(posts),
}));

export const postsRelations = relations(posts, ({ one, many }) => ({
  author: one(users, { fields: [posts.authorId], references: [users.id] }),
  category: one(categories, { fields: [posts.categoryId], references: [categories.id] }),
  comments: many(comments),
  likes: many(likes),
  postTags: many(postTags),
}));

export const commentsRelations = relations(comments, ({ one }) => ({
  post: one(posts, { fields: [comments.postId], references: [posts.id] }),
  author: one(users, { fields: [comments.authorId], references: [users.id] }),
}));

export const tagsRelations = relations(tags, ({ many }) => ({
  postTags: many(postTags),
}));

export const postTagsRelations = relations(postTags, ({ one }) => ({
  post: one(posts, { fields: [postTags.postId], references: [posts.id] }),
  tag: one(tags, { fields: [postTags.tagId], references: [tags.id] }),
}));

export const followersRelations = relations(followers, ({ one }) => ({
  follower: one(users, { relationName: "followers", fields: [followers.followerId], references: [users.id] }),
  followee: one(users, { relationName: "following", fields: [followers.followeeId], references: [users.id] }),
}));

export const likesRelations = relations(likes, ({ one }) => ({
  user: one(users, { fields: [likes.userId], references: [users.id] }),
  post: one(posts, { fields: [likes.postId], references: [posts.id] }),
}));
