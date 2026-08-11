import { integer, sqliteTable, text } from "drizzle-orm/sqlite-core";
import { relations } from "drizzle-orm";

export const users = sqliteTable("users", {
  id: integer("id").primaryKey(),
  email: text("email").notNull(),
  age: integer("age").notNull(),
});

export const posts = sqliteTable("posts", {
  id: integer("id").primaryKey(),
  authorId: integer("author_id").notNull(),
  title: text("title").notNull(),
});

export const benchBulk = sqliteTable("bench_bulk", {
  id: integer("id").primaryKey(),
  name: text("name").notNull(),
  n: integer("n").notNull(),
});

export const usersRelations = relations(users, ({ many }) => ({
  posts: many(posts),
}));

export const postsRelations = relations(posts, ({ one }) => ({
  author: one(users, { fields: [posts.authorId], references: [users.id] }),
}));
