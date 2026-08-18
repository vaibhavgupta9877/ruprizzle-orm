# Blog example

A runnable example of a `User` / `Post` schema. It demonstrates `create`,
transactions, `find_many`, `include`, and `.to_sql()`.

## Setup

1. Start a local PostgreSQL database and create `blog_example`.
2. Copy `.env.example` to `.env` and fill in your `DATABASE_URL`.
3. Add `examples/blog` to the workspace `members` in the root `Cargo.toml` if it
   is not already there.
4. Run `ruprizzle migrate dev --name init` in this directory.
5. Run `ruprizzle generate` to create `src/db`.
6. Run `cargo run -p ruprizzle-example-blog`.

The generated `src/db` module is intentionally ignored from git; run
`ruprizzle generate` to regenerate it after schema changes.
