import { drizzle } from 'drizzle-orm/better-sqlite3';
import Database from 'better-sqlite3';
import { eq, gt, asc, desc, and, like, between, inArray, sql, exists, lte, fillPlaceholders } from 'drizzle-orm';
import { performance } from 'node:perf_hooks';
import { writeFileSync } from 'node:fs';
import * as schema from './schema.js';

const DB_PATH = process.env.BENCH_SQLITE_PATH
  || 'D:\\SaaS\\rust\\ruprizzle-orm\\local\\cross-orm-bench\\node\\bench.sqlite3';
const client = new Database(DB_PATH);
const db = drizzle(client, { schema });

const { users, posts, benchBulk } = schema;

const ids50 = Array.from({ length: 50 }, (_, i) => i + 1);

function makeBulkRows() {
  return Array.from({ length: 1000 }, (_, i) => ({
    id: i + 1,
    name: `bulk-${i}`,
    n: i * 3,
  }));
}

function rowsFromResult(result, rowsFnOrConst) {
  if (rowsFnOrConst !== undefined) {
    return typeof rowsFnOrConst === 'function' ? rowsFnOrConst(result) : rowsFnOrConst;
  }
  if (Array.isArray(result)) return result.length;
  if (typeof result === 'number') return Math.floor(result);
  if (typeof result === 'boolean') return result ? 1 : 0;
  if (result && typeof result === 'object') {
    if ('count' in result) return Number(result.count);
    if ('changes' in result) return Number(result.changes);
    return 1;
  }
  return result ? 1 : 0;
}

async function runMaybe(fn) {
  const r = fn();
  return r && typeof r.then === 'function' ? await r : r;
}

async function bench(name, fn, iters, { setup, rows: rowsFnOrConst, queries } = {}) {
  // Warm up
  for (let i = 0; i < 3; i++) {
    if (setup) await runMaybe(setup);
    const r = await fn();
    if (i === 2) {
      if (name === 'find_many_1000') console.assert(Array.isArray(r) && r.length === 1000, `${name}: unexpected result`);
      if (name === 'find_filtered_paginated') console.assert(Array.isArray(r) && r.length === 20, `${name}: unexpected result`);
      if (name === 'find_in_list') console.assert(Array.isArray(r) && r.length === 50, `${name}: unexpected result`);
      if (name === 'find_complex_filter') console.assert(Array.isArray(r) && r.length === 100, `${name}: unexpected result`);
      if (name === 'find_popular_posts') console.assert(Array.isArray(r) && r.length === 100, `${name}: unexpected result`);
      if (name === 'count_filtered') console.assert(typeof r === 'number' && r >= 980, `${name}: unexpected result`);
      if (name === 'exists_filtered') console.assert(r === true, `${name}: unexpected result`);
      if (name === 'select_by_pk') console.assert(r && r.id === 500, `${name}: unexpected result`);
      if (name === 'prepared_select_by_pk') console.assert(r && r.id === 500, `${name}: unexpected result`);
      if (name === 'stream_find_many_1000') console.assert(typeof r === 'number' && r === 1000, `${name}: unexpected result`);
      if (name === 'include_posts') console.assert(Array.isArray(r) && r.length === 1000 && r[0]?.posts?.length === 10, `${name}: unexpected result`);
      if (name === 'include_author') console.assert(Array.isArray(r) && r.length === 10000 && r[0]?.author != null, `${name}: unexpected result`);
      if (name === 'include_posts_and_comments') console.assert(Array.isArray(r) && r.length === 1000 && r[0]?.posts?.[0]?.comments?.length === 5, `${name}: unexpected result`);
      if (name === 'include_posts_with_tags') console.assert(Array.isArray(r) && r.length === 10000 && r[0]?.postTags?.length > 0, `${name}: unexpected result`);
      if (name === 'bulk_insert_1000') console.assert(r && r.changes === 1000, `${name}: unexpected result`);
    }
  }

  const memBefore = process.memoryUsage().rss;
  const cpuBefore = process.cpuUsage();
  const start = performance.now();
  let last;
  for (let i = 0; i < iters; i++) {
    if (setup) await runMaybe(setup);
    last = await fn();
  }
  const total = performance.now() - start;
  const memAfter = process.memoryUsage().rss;
  const cpuAfter = process.cpuUsage(cpuBefore);

  const avg = total / iters;
  const qps = total > 0 ? (iters * 1000) / total : 0;
  const peakRssMb = Math.max(memBefore, memAfter) / (1024 * 1024);
  const cpuTimeMs = (cpuAfter.user + cpuAfter.system) / 1000;
  const rowsReturned = rowsFromResult(last, rowsFnOrConst);

  console.log(`${name}: ${avg.toFixed(3)} ms/op (total ${total.toFixed(1)} ms, ${iters} iters)`);
  return {
    orm: 'drizzle',
    operation: name,
    iters,
    total_ms: total,
    avg_ms: avg,
    qps,
    rows_returned: rowsReturned,
    queries_issued: queries,
    peak_rss_mb: peakRssMb,
    cpu_time_ms: cpuTimeMs,
  };
}

const results = [];

// Query construction (no I/O): Drizzle exposes .toSQL()
results.push(await bench('to_sql_select_by_pk', () => {
  return db.select().from(users).where(eq(users.id, 500)).limit(1).toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_filter_order', () => {
  return db.select().from(users)
    .where(and(gt(users.age, 18), like(users.email, '%@example.com%')))
    .orderBy(asc(users.age), asc(users.email))
    .limit(1000)
    .offset(0)
    .toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_in_list', () => {
  return db.select().from(users)
    .where(inArray(users.id, ids50))
    .orderBy(asc(users.id))
    .limit(50)
    .toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_complex_filter', () => {
  return db.select().from(users)
    .where(and(
      gt(users.age, 18),
      like(users.email, '%example.com%'),
      between(users.id, 100, 900)
    ))
    .orderBy(asc(users.age), asc(users.email))
    .limit(100)
    .toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_paginated', () => {
  return db.select().from(users)
    .where(and(gt(users.age, 18), like(users.email, '%example.com%')))
    .orderBy(asc(users.age), asc(users.email))
    .limit(20)
    .offset(500)
    .toSQL();
}, 100000, { rows: 0, queries: 0 }));

// New query-construction benchmarks for recent ruprizzle enhancements.

results.push(await bench('to_sql_prepared_select_by_pk', () => {
  return db.select().from(users)
    .where(eq(users.id, sql.placeholder('id')))
    .limit(1)
    .toSQL();
}, 100000, { rows: 0, queries: 0 }));

const preparedQueryForRebind = db.select().from(users)
  .where(eq(users.id, sql.placeholder('id')))
  .limit(1)
  .toSQL();
results.push(await bench('prepared_rebind_select_by_pk', () => {
  return fillPlaceholders(preparedQueryForRebind.params, { id: 123 });
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_conditional_filter', () => {
  let q = db.select().from(users);
  if (1) q = q.where(gt(users.age, 18));
  if (1) q = q.orderBy(asc(users.age), asc(users.email));
  if (1) q = q.limit(100);
  return q.toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_with_cte', () => {
  const active = db.$with('active').as(
    db.select().from(users).where(gt(users.age, 18))
  );
  return db.with(active).select().from(active).where(gt(users.id, 0)).toSQL();
}, 100000, { rows: 0, queries: 0 }));

// Drizzle's query builder does not expose a recursive CTE helper, so we
// construct the recursive CTE with the raw `sql` template for this benchmark.
results.push(await bench('to_sql_select_with_recursive_cte', () => {
  return sql`WITH RECURSIVE nums(n) AS (SELECT 1 AS n UNION ALL SELECT n+1 FROM nums WHERE n < 5) SELECT * FROM nums`;
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_set_union', () => {
  return db.select().from(users).where(gt(users.age, 18))
    .unionAll(db.select().from(users).where(lte(users.age, 18)))
    .toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_with_join', () => {
  return db.select().from(posts)
    .innerJoin(users, eq(posts.authorId, users.id))
    .toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_exists_subquery', () => {
  const sub = db.select().from(posts).where(eq(posts.authorId, users.id));
  return db.select().from(users).where(exists(sub)).toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_in_subquery', () => {
  const sub = db.select({ authorId: posts.authorId }).from(posts).where(gt(posts.authorId, 0));
  return db.select().from(users).where(inArray(users.id, sub)).toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_nested_insert', () => {
  const newUser = db.$with('new_user').as(
    db.insert(users).values({
      id: 9999,
      email: 'nested@example.com',
      age: 30,
      name: 'Nested',
      createdAt: 0,
    }).returning({ id: users.id })
  );
  // Drizzle cannot reference CTE columns directly in `.values()`, so we keep
  // the CTE in the statement and use a concrete value for the child row.
  return db.with(newUser).insert(posts).values({
    id: 10001,
    authorId: 9999,
    categoryId: 1,
    title: 'nested post',
    publishedAt: 0,
    views: 0,
  }).toSQL();
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_nested_update', () => {
  const updatedUser = db.$with('updated_user').as(
    db.update(users).set({ name: 'updated' }).where(eq(users.id, 1)).returning({ id: users.id })
  );
  // Drizzle cannot set a column to a CTE column reference, so the CTE is
  // preserved and the SET clause uses a concrete value.
  return db.with(updatedUser).update(posts).set({ authorId: 9999 })
    .where(inArray(posts.id, [10001, 10002])).toSQL();
}, 100000, { rows: 0, queries: 0 }));

// End-to-end: select by PK
results.push(await bench('select_by_pk', () => {
  return db.select().from(users).where(eq(users.id, 500)).limit(1).get();
}, 1000, { rows: 1, queries: 1 }));

// End-to-end: find many 1000 rows
results.push(await bench('find_many_1000', () => {
  return db.select().from(users).all();
}, 50, { queries: 1 }));

// End-to-end: filtered + ordered
results.push(await bench('find_filtered_ordered', () => {
  return db.select().from(users)
    .where(gt(users.age, 18))
    .orderBy(asc(users.age), asc(users.email))
    .all();
}, 50, { queries: 1 }));

// End-to-end: filtered + ordered + paginated
results.push(await bench('find_filtered_paginated', () => {
  return db.select().from(users)
    .where(gt(users.age, 18))
    .orderBy(asc(users.age), asc(users.email))
    .limit(20)
    .offset(500)
    .all();
}, 50, { queries: 1 }));

// End-to-end: IN list with 50 ids
results.push(await bench('find_in_list', () => {
  return db.select().from(users)
    .where(inArray(users.id, ids50))
    .orderBy(asc(users.id))
    .all();
}, 100, { queries: 1 }));

// End-to-end: complex filter with multiple parameters
results.push(await bench('find_complex_filter', () => {
  return db.select().from(users)
    .where(and(
      gt(users.age, 18),
      like(users.email, '%example.com%'),
      between(users.id, 100, 900)
    ))
    .orderBy(asc(users.age), asc(users.email))
    .limit(100)
    .all();
}, 50, { queries: 1 }));

// End-to-end: count with filter
results.push(await bench('count_filtered', () => {
  const r = db.select({ count: sql`count(*)` }).from(users).where(gt(users.age, 18)).get();
  return r ? Number(r.count) : 0;
}, 100, { queries: 1 }));

// End-to-end: exists with filter
results.push(await bench('exists_filtered', () => {
  return db.select().from(users).where(gt(users.age, 18)).limit(1).get() != null;
}, 100, { rows: 1, queries: 1 }));

// End-to-end: include posts for all users
results.push(await bench('include_posts', () => {
  return db.query.users.findMany({ with: { posts: true } }).execute();
}, 10, { queries: 2 }));

// End-to-end: include author for all posts
results.push(await bench('include_author', () => {
  return db.query.posts.findMany({ with: { author: true } }).execute();
}, 10, { queries: 2 }));

// End-to-end: include posts and their comments
results.push(await bench('include_posts_and_comments', () => {
  return db.query.users.findMany({
    with: { posts: { with: { comments: true } } },
  }).execute();
}, 10, { queries: 3 }));

// End-to-end: posts with tags (many-to-many through post_tags)
results.push(await bench('include_posts_with_tags', () => {
  return db.query.posts.findMany({
    with: { postTags: { with: { tag: true } } },
  }).execute();
}, 10, { queries: 3 }));

// End-to-end: find popular posts (views > 1000) with author
results.push(await bench('find_popular_posts', () => {
  return db.query.posts.findMany({
    where: (t, { gt }) => gt(t.views, 1000),
    orderBy: (t, { desc }) => desc(t.views),
    limit: 100,
    with: { author: true },
  }).execute();
}, 50, { queries: 2 }));

// End-to-end: reusable prepared statement for single-row PK lookup.
const preparedSelectByPk = db.select().from(users)
  .where(eq(users.id, sql.placeholder('id')))
  .limit(1)
  .prepare();
results.push(await bench('prepared_select_by_pk', () => {
  return preparedSelectByPk.get({ id: 500 });
}, 1000, { rows: 1, queries: 1 }));

// End-to-end: stream all users using the underlying better-sqlite3 driver.
const streamQuery = db.select().from(users).toSQL();
const streamStmt = client.prepare(streamQuery.sql);
results.push(await bench('stream_find_many_1000', () => {
  let rows = 0;
  for (const _ of streamStmt.iterate()) rows += 1;
  if (rows !== 1000) throw new Error(`stream_find_many_1000: expected 1000, got ${rows}`);
  return rows;
}, 50, { rows: 1000, queries: 1 }));

// Bulk insert 1000 rows into bench_bulk
results.push(await bench('bulk_insert_1000', () => {
  return db.insert(benchBulk).values(makeBulkRows()).run();
}, 10, {
  setup: () => client.prepare('DELETE FROM bench_bulk').run(),
  rows: 1000,
  queries: 2,
}));

client.close();

writeFileSync('drizzle-results.json', JSON.stringify(results, null, 2));
console.log('Wrote drizzle-results.json');
