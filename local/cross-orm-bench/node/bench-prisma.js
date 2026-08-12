import { PrismaClient } from '@prisma/client';
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
