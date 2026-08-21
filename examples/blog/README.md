# Blog example

A runnable example of a `User` / `Post` schema. It demonstrates `create`,
transactions, `find_many`, `include`, and `.to_sql()`.

## Setup

This crate is standalone -- it is not a member of the root workspace, because
its generated `src/db` module is gitignored and a fresh clone would otherwise
fail `cargo test --workspace`. Run every command below from `examples/blog`.

1. Start a local PostgreSQL database and create `blog_example`.
2. Copy `.env.example` to `.env` and fill in your `DATABASE_URL`.
3. Run `ruprizzle migrate dev --name init` in this directory.
4. Run `ruprizzle generate` to create `src/db`.
5. Run `cargo run` (equivalently, `cargo run -p ruprizzle-example-blog`).

Steps 3 and 4 are required before the crate compiles: `cargo build` fails with
`file not found for module 'db'` until `ruprizzle generate` has run.

The generated `src/db` module is intentionally ignored from git; run
`ruprizzle generate` to regenerate it after schema changes.
