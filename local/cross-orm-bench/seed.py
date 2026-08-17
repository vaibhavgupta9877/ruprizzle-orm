#!/usr/bin/env python3
"""Seed PostgreSQL or MySQL with the cross-ORM benchmark dataset.

Reproduces the same data as local/cross-orm-bench/node/seed.js using the psql
or mysql CLI. Falls back gracefully (with a warning) if the CLI is not installed.

Usage:
    python seed.py <postgres|mysql> [URL]

URL may also be supplied via BENCH_PG_URL or BENCH_MYSQL_URL. The default
Postgres URL is postgres://ruprizzle_test:ruprizzle_test@127.0.0.1:5433/ruprizzle_test.
The default MySQL URL is mysql://ruprizzle_test:ruprizzle_test@127.0.0.1:3307/ruprizzle_test.
"""

import os
import shutil
import subprocess
import sys
import urllib.parse

USER_COUNT = 1000
CATEGORY_COUNT = 20
POSTS_PER_USER = 10
COMMENTS_PER_POST = 5
TAG_COUNT = 100
TAGS_PER_POST = 3
FOLLOWER_COUNT = 5000
LIKE_COUNT = 20000
NOW = 1700000000

BATCH_SIZE = 1000


def default_url(backend: str) -> str:
    if backend == "postgres":
        return "postgres://ruprizzle_test:ruprizzle_test@127.0.0.1:5433/ruprizzle_test"
    if backend == "mysql":
        return "mysql://ruprizzle_test:ruprizzle_test@127.0.0.1:3307/ruprizzle_test"
    raise ValueError(f"unknown backend: {backend}")


def parse_url(url: str) -> urllib.parse.ParseResult:
    if url.startswith("postgres://") or url.startswith("postgresql://"):
        return urllib.parse.urlparse(url)
    if url.startswith("mysql://"):
        return urllib.parse.urlparse(url)
    raise ValueError(f"unsupported URL scheme: {url}")


def sql_string(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def build_schema(dialect: str) -> str:
    if dialect == "postgres":
        text = "TEXT"
        drop = "DROP TABLE IF EXISTS {name} CASCADE"
    else:
        text = "VARCHAR(255)"
        drop = "DROP TABLE IF EXISTS {name}"

    tables = [
        ("users", f"""CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    email {text} NOT NULL,
    age BIGINT NOT NULL,
    name {text} NOT NULL,
    created_at BIGINT NOT NULL
);"""),
        ("categories", f"""CREATE TABLE categories (
    id BIGINT PRIMARY KEY,
    name {text} NOT NULL
);"""),
        ("posts", f"""CREATE TABLE posts (
    id BIGINT PRIMARY KEY,
    author_id BIGINT NOT NULL,
    category_id BIGINT NOT NULL,
    title {text} NOT NULL,
    published_at BIGINT NOT NULL,
    views BIGINT NOT NULL
);"""),
        ("comments", f"""CREATE TABLE comments (
    id BIGINT PRIMARY KEY,
    post_id BIGINT NOT NULL,
    author_id BIGINT NOT NULL,
    content {text} NOT NULL,
    created_at BIGINT NOT NULL
);"""),
        ("tags", f"""CREATE TABLE tags (
    id BIGINT PRIMARY KEY,
    name {text} NOT NULL
);"""),
        ("post_tags", f"""CREATE TABLE post_tags (
    post_id BIGINT NOT NULL,
    tag_id BIGINT NOT NULL,
    PRIMARY KEY (post_id, tag_id)
);"""),
        ("followers", f"""CREATE TABLE followers (
    follower_id BIGINT NOT NULL,
    followee_id BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (follower_id, followee_id)
);"""),
        ("likes", f"""CREATE TABLE likes (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    post_id BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);"""),
        ("bench_bulk", f"""CREATE TABLE bench_bulk (
    id BIGINT PRIMARY KEY,
    name {text} NOT NULL,
    n BIGINT NOT NULL
);"""),
    ]

    order = [
        "bench_bulk",
        "likes",
        "followers",
        "post_tags",
        "tags",
        "comments",
        "posts",
        "categories",
        "users",
    ]
    by_name = {name: ddl for name, ddl in tables}
    lines = [drop.format(name=name) + ";" for name in order]
    lines.append("")
    for name in order[::-1]:  # create in reverse drop order
        lines.append(by_name[name])
    return "\n".join(lines) + "\n"


def batched_insert(table: str, columns: list[str], rows: list[tuple]) -> list[str]:
    col_list = ", ".join(columns)
    chunks = [rows[i : i + BATCH_SIZE] for i in range(0, len(rows), BATCH_SIZE)]
    statements = []
    for chunk in chunks:
        values = ",\n    ".join(str(row) for row in chunk)
        statements.append(f"INSERT INTO {table} ({col_list}) VALUES\n    {values};")
    return statements


def build_inserts(dialect: str) -> list[str]:
    _ = dialect  # same SQL for both; quote differences handled by sql_string
    statements = []

    users = [
        (
            i,
            sql_string(f"user-{i}@example.com"),
            18 + (i % 50),
            sql_string(f"User {i}"),
            NOW + i,
        )
        for i in range(1, USER_COUNT + 1)
    ]
    statements.extend(batched_insert("users", ["id", "email", "age", "name", "created_at"], users))

    categories = [(i, sql_string(f"category-{i}")) for i in range(1, CATEGORY_COUNT + 1)]
    statements.extend(batched_insert("categories", ["id", "name"], categories))

    post_rows = []
    post_id = 1
    for author_id in range(1, USER_COUNT + 1):
        for _ in range(POSTS_PER_USER):
            category_id = ((post_id - 1) % CATEGORY_COUNT) + 1
            title = sql_string(f"post-{post_id}")
            published_at = NOW + post_id
            views = post_id * 7
            post_rows.append(
                (post_id, author_id, category_id, title, published_at, views)
            )
            post_id += 1
    statements.extend(batched_insert("posts", ["id", "author_id", "category_id", "title", "published_at", "views"], post_rows))

    comment_rows = []
    comment_id = 1
    for post_id in range(1, USER_COUNT * POSTS_PER_USER + 1):
        for _ in range(COMMENTS_PER_POST):
            author_id = ((comment_id - 1) % USER_COUNT) + 1
            content = sql_string(f"comment-{comment_id}")
            comment_rows.append((comment_id, post_id, author_id, content, NOW + comment_id))
            comment_id += 1
    statements.extend(batched_insert("comments", ["id", "post_id", "author_id", "content", "created_at"], comment_rows))

    tags = [(i, sql_string(f"tag-{i}")) for i in range(1, TAG_COUNT + 1)]
    statements.extend(batched_insert("tags", ["id", "name"], tags))

    post_tags = []
    post_count = USER_COUNT * POSTS_PER_USER
    for post_id in range(1, post_count + 1):
        for j in range(TAGS_PER_POST):
            tag_id = ((post_id + j - 1) % TAG_COUNT) + 1
            post_tags.append((post_id, tag_id))
    statements.extend(batched_insert("post_tags", ["post_id", "tag_id"], post_tags))

    followers = []
    for i in range(1, FOLLOWER_COUNT + 1):
        follower_id = ((i - 1) % USER_COUNT) + 1
        floor = (i - 1) // USER_COUNT
        followee_id = ((floor + i) % USER_COUNT) + 1
        if follower_id != followee_id:
            followers.append((follower_id, followee_id, NOW + i))
    statements.extend(batched_insert("followers", ["follower_id", "followee_id", "created_at"], followers))

    post_count = USER_COUNT * POSTS_PER_USER
    likes = []
    for i in range(1, LIKE_COUNT + 1):
        post_id = ((i * 13 - 1) % 1000) + 1
        user_id = ((i * 17 - 1) % USER_COUNT) + 1
        author_id = (post_id - 1) // POSTS_PER_USER + 1
        if user_id != author_id:
            likes.append((i, user_id, post_id, NOW + i))
    statements.extend(batched_insert("likes", ["id", "user_id", "post_id", "created_at"], likes))

    return statements


def generate_sql(dialect: str) -> str:
    tx_start = "BEGIN;" if dialect == "postgres" else "START TRANSACTION;"
    tx_end = "COMMIT;" if dialect == "postgres" else "COMMIT;"
    parts = [build_schema(dialect), tx_start]
    parts.extend(build_inserts(dialect))
    parts.append(tx_end)
    return "\n".join(parts)


def seed_postgres(url: str) -> int:
    if not shutil.which("psql"):
        print("warning: psql not found; skipping PostgreSQL seed", file=sys.stderr)
        return 2
    env = os.environ.copy()
    env["PGPASSWORD"] = urllib.parse.urlparse(url).password or ""
    cmd = ["psql", url, "-v", "ON_ERROR_STOP=1", "-f", "-"]
    try:
        subprocess.run(cmd, input=generate_sql("postgres"), text=True, check=True, env=env)
    except subprocess.CalledProcessError as e:
        print(f"error: PostgreSQL seed failed: {e}", file=sys.stderr)
        return 1
    print(f"Seeded PostgreSQL at {url}")
    return 0


def seed_mysql(url: str) -> int:
    if not shutil.which("mysql"):
        print("warning: mysql not found; skipping MySQL seed", file=sys.stderr)
        return 2
    parsed = urllib.parse.urlparse(url)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or 3307
    user = parsed.username or "ruprizzle_test"
    password = parsed.password or "ruprizzle_test"
    database = parsed.path.lstrip("/") or "ruprizzle_test"
    env = os.environ.copy()
    # mysql CLI reads the password from MYSQL_PWD in non-interactive mode.
    env["MYSQL_PWD"] = password
    cmd = [
        "mysql",
        "--protocol=TCP",
        "-h",
        host,
        "-P",
        str(port),
        "-u",
        user,
        database,
    ]
    try:
        subprocess.run(cmd, input=generate_sql("mysql"), text=True, check=True, env=env)
    except subprocess.CalledProcessError as e:
        print(f"error: MySQL seed failed: {e}", file=sys.stderr)
        return 1
    print(f"Seeded MySQL at {url}")
    return 0


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 1
    backend = argv[1]
    if backend not in ("postgres", "mysql"):
        print(f"error: backend must be 'postgres' or 'mysql', got {backend!r}", file=sys.stderr)
        return 1

    env_var = "BENCH_PG_URL" if backend == "postgres" else "BENCH_MYSQL_URL"
    url = argv[2] if len(argv) > 2 else os.environ.get(env_var, default_url(backend))

    if backend == "postgres":
        return seed_postgres(url)
    return seed_mysql(url)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
