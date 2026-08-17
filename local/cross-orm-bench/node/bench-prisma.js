import { PrismaClient, Prisma } from '@prisma/client';
import { performance } from 'node:perf_hooks';
import { writeFileSync } from 'node:fs';

const clientOptions = {};
const dbPath = process.env.BENCH_SQLITE_PATH;
if (dbPath) {
  clientOptions.datasourceUrl = 'file:' + dbPath.replace(/\\/g, '/');
}
const prisma = new PrismaClient(clientOptions);

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
      if (name === 'stream_find_many_1000') console.assert(Array.isArray(r) && r.length === 1000, `${name}: unexpected result`);
      if (name === 'include_posts') console.assert(Array.isArray(r) && r.length === 1000 && r[0]?.posts?.length === 10, `${name}: unexpected result`);
      if (name === 'include_author') console.assert(Array.isArray(r) && r.length === 10000 && r[0]?.author != null, `${name}: unexpected result`);
      if (name === 'include_posts_and_comments') console.assert(Array.isArray(r) && r.length === 1000 && r[0]?.posts?.[0]?.comments?.length === 5, `${name}: unexpected result`);
      if (name === 'include_posts_with_tags') console.assert(Array.isArray(r) && r.length === 10000 && r[0]?.postTags?.length > 0, `${name}: unexpected result`);
      if (name === 'bulk_insert_1000') console.assert(r && r.count === 1000, `${name}: unexpected result`);
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
    orm: 'prisma',
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

await prisma.$connect();

// Query construction (no I/O): Prisma's client does not expose a query-builder
// `toSQL()`; the closest public construction API is Prisma.sql, so these
// benchmarks measure the time to build the equivalent raw Sql objects.

results.push(await bench('to_sql_select_by_pk', () =>
  Prisma.sql`SELECT * FROM users WHERE id = ${500} LIMIT 1`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_filter_order', () =>
  Prisma.sql`SELECT * FROM users WHERE age > ${18} AND email LIKE ${'%@example.com%'} ORDER BY age, email LIMIT ${1000} OFFSET ${0}`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_in_list', () =>
  Prisma.sql`SELECT * FROM users WHERE id IN (${Prisma.join(ids50)}) ORDER BY id LIMIT 50`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_complex_filter', () =>
  Prisma.sql`SELECT * FROM users WHERE age > ${18} AND email LIKE ${'%example.com%'} AND id BETWEEN ${100} AND ${900} ORDER BY age, email LIMIT 100`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_paginated', () =>
  Prisma.sql`SELECT * FROM users WHERE age > ${18} AND email LIKE ${'%example.com%'} ORDER BY age, email LIMIT 20 OFFSET 500`
, 100000, { rows: 0, queries: 0 }));

let rebindId = 123;

results.push(await bench('to_sql_prepared_select_by_pk', () =>
  Prisma.sql`SELECT * FROM users WHERE id = $1 LIMIT 1`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('prepared_rebind_select_by_pk', () =>
  Prisma.sql`SELECT * FROM users WHERE id = ${rebindId++} LIMIT 1`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_conditional_filter', () => {
  const ageFilter = 1;
  const orderFilter = 1;
  const limitFilter = 1;

  const conditions = [];
  if (ageFilter) conditions.push(Prisma.sql`age > ${18}`);
  if (ageFilter) conditions.push(Prisma.sql`email LIKE ${'%@example.com%'}`);
  let where = Prisma.sql``;
  if (conditions.length > 0) {
    where = Prisma.sql`WHERE ${conditions[0]}`;
    for (let i = 1; i < conditions.length; i++) {
      where = Prisma.sql`${where} AND ${conditions[i]}`;
    }
  }

  const order = orderFilter ? Prisma.sql`ORDER BY age, email` : Prisma.sql``;
  const limit = limitFilter ? Prisma.sql`LIMIT ${100}` : Prisma.sql``;
  return Prisma.sql`SELECT * FROM users ${where} ${order} ${limit}`;
}, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_with_cte', () =>
  Prisma.sql`WITH active AS (SELECT * FROM users WHERE age > ${18}) SELECT * FROM active WHERE id > ${0}`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_with_recursive_cte', () =>
  Prisma.sql`WITH RECURSIVE nums(n) AS (SELECT 1 AS n UNION ALL SELECT n+1 FROM nums WHERE n < 5) SELECT * FROM nums`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_set_union', () =>
  Prisma.sql`SELECT * FROM users WHERE age > ${18} UNION ALL SELECT * FROM users WHERE age <= ${18}`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_with_join', () =>
  Prisma.sql`SELECT * FROM posts INNER JOIN users ON posts.author_id = users.id`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_exists_subquery', () =>
  Prisma.sql`SELECT * FROM users WHERE EXISTS (SELECT 1 FROM posts WHERE posts.author_id = users.id)`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_select_in_subquery', () =>
  Prisma.sql`SELECT * FROM users WHERE id IN (SELECT author_id FROM posts WHERE author_id > ${0})`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_nested_insert', () =>
  Prisma.sql`WITH new_user AS (INSERT INTO users (id, email, age, name, created_at) VALUES (9999, 'nested@example.com', 30, 'Nested', 0) RETURNING id) INSERT INTO posts (id, author_id, category_id, title, published_at, views) VALUES (10001, 9999, 1, 'nested post', 0, 0)`
, 100000, { rows: 0, queries: 0 }));

results.push(await bench('to_sql_nested_update', () =>
  Prisma.sql`WITH updated_user AS (UPDATE users SET name = 'updated' WHERE id = 1 RETURNING id) UPDATE posts SET author_id = 9999 WHERE id IN (10001, 10002)`
, 100000, { rows: 0, queries: 0 }));

// End-to-end: select by PK
results.push(await bench('select_by_pk', () =>
  prisma.user.findUnique({ where: { id: 500 } })
, 1000, { rows: 1, queries: 1 }));

// End-to-end: find many 1000 rows
results.push(await bench('find_many_1000', () =>
  prisma.user.findMany()
, 50, { queries: 1 }));

// End-to-end: filtered + ordered
results.push(await bench('find_filtered_ordered', () =>
  prisma.user.findMany({
    where: { age: { gt: 18 } },
    orderBy: [{ age: 'asc' }, { email: 'asc' }],
  })
, 50, { queries: 1 }));

// End-to-end: filtered + ordered + paginated
results.push(await bench('find_filtered_paginated', () =>
  prisma.user.findMany({
    where: { age: { gt: 18 } },
    orderBy: [{ age: 'asc' }, { email: 'asc' }],
    skip: 500,
    take: 20,
  })
, 50, { queries: 1 }));

// End-to-end: IN list with 50 ids
results.push(await bench('find_in_list', () =>
  prisma.user.findMany({
    where: { id: { in: ids50 } },
    orderBy: { id: 'asc' },
  })
, 100, { queries: 1 }));

// End-to-end: complex filter with multiple parameters
results.push(await bench('find_complex_filter', () =>
  prisma.user.findMany({
    where: {
      AND: [
        { age: { gt: 18 } },
        { email: { contains: 'example.com' } },
        { id: { gte: 100, lte: 900 } },
      ],
    },
    orderBy: [{ age: 'asc' }, { email: 'asc' }],
    take: 100,
  })
, 50, { queries: 1 }));

// End-to-end: count with filter
results.push(await bench('count_filtered', () =>
  prisma.user.count({ where: { age: { gt: 18 } } })
, 100, { queries: 1 }));

// End-to-end: exists with filter
results.push(await bench('exists_filtered', () =>
  prisma.user.findFirst({ where: { age: { gt: 18 } }, select: { id: true } }).then(r => r != null)
, 100, { rows: 1, queries: 1 }));

// End-to-end: include posts for all users
results.push(await bench('include_posts', () =>
  prisma.user.findMany({ include: { posts: true } })
, 10, { queries: 2 }));

// End-to-end: include author for all posts
results.push(await bench('include_author', () =>
  prisma.post.findMany({ include: { author: true } })
, 10, { queries: 2 }));

// End-to-end: include posts and their comments
results.push(await bench('include_posts_and_comments', () =>
  prisma.user.findMany({ include: { posts: { include: { comments: true } } } })
, 10, { queries: 3 }));

// End-to-end: posts with tags (many-to-many through post_tags)
results.push(await bench('include_posts_with_tags', () =>
  prisma.post.findMany({ include: { postTags: { include: { tag: true } } } })
, 10, { queries: 3 }));

// End-to-end: find popular posts (views > 1000) with author
results.push(await bench('find_popular_posts', () =>
  prisma.post.findMany({
    where: { views: { gt: 1000 } },
    orderBy: { views: 'desc' },
    take: 100,
    include: { author: true },
  })
, 50, { queries: 2 }));

// End-to-end: prepared select by PK (Prisma has no explicit prepared statement API;
// this measures the same client operation path for consistency).
results.push(await bench('prepared_select_by_pk', () =>
  prisma.user.findUnique({ where: { id: 500 } })
, 1000, { rows: 1, queries: 1 }));

// End-to-end: streaming find many 1000 rows
results.push(await bench('stream_find_many_1000', () =>
  prisma.user.findMany()
, 50, { rows: 1000, queries: 1 }));

// Bulk insert 1000 rows into bench_bulk
results.push(await bench('bulk_insert_1000', () =>
  prisma.benchBulk.createMany({ data: makeBulkRows() })
, 10, {
  setup: () => prisma.benchBulk.deleteMany(),
  rows: 1000,
  queries: 2,
}));

await prisma.$disconnect();

writeFileSync('prisma-results.json', JSON.stringify(results, null, 2));
console.log('Wrote prisma-results.json');
