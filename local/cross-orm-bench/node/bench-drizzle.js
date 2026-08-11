import { drizzle } from 'drizzle-orm/better-sqlite3';
import Database from 'better-sqlite3';
import { eq, gt, asc } from 'drizzle-orm';
import { performance } from 'node:perf_hooks';
import { writeFileSync } from 'node:fs';
import * as schema from './schema.js';

const client = new Database('./bench.sqlite3');
const db = drizzle(client, { schema });

const { users, posts, benchBulk } = schema;

async function bench(name, fn, iters, setup) {
  // Warm up
  for (let i = 0; i < 3; i++) {
    if (setup) setup();
    const r = await fn();
    if (i === 2) {
      if (name.startsWith('find_many_1000')) console.assert(r.length === 1000, `${name}: expected 1000 got ${r.length}`);
      if (name === 'include_posts') console.assert(r.length === 1000 && r[0].posts?.length === 10, `${name}: unexpected result`);
    }
  }

  const start = performance.now();
  for (let i = 0; i < iters; i++) {
    if (setup) setup();
    await fn();
  }
  const total = performance.now() - start;
  const avg = total / iters;
  console.log(`${name}: ${avg.toFixed(3)} ms/op (total ${total.toFixed(1)} ms, ${iters} iters)`);
  return { orm: 'drizzle', operation: name, iters, total_ms: total, avg_ms: avg };
}

function makeBulkRows() {
  return Array.from({ length: 1000 }, (_, i) => ({
    id: i + 1,
    name: `bulk-${i}`,
    n: i * 3,
  }));
}

const results = [];

// Query construction (no I/O): Drizzle exposes .toSQL()
results.push(await bench('to_sql_select_by_pk', async () => {
  return db.select().from(users).where(eq(users.id, 500)).toSQL();
}, 100000));

results.push(await bench('to_sql_select_filter_order', async () => {
  return db.select().from(users).where(gt(users.age, 18)).orderBy(asc(users.email)).toSQL();
}, 100000));

// End-to-end
results.push(await bench('select_by_pk', async () => {
  return db.select().from(users).where(eq(users.id, 500)).get();
}, 1000));

results.push(await bench('find_many_1000', async () => {
  return db.select().from(users).all();
}, 50));

results.push(await bench('find_filtered_ordered', async () => {
  return db.select().from(users).where(gt(users.age, 18)).orderBy(asc(users.email)).all();
}, 50));

results.push(await bench('include_posts', async () => {
  const q = db.query.users.findMany({ with: { posts: true } });
  return q.execute();
}, 10));

results.push(await bench('bulk_insert_1000', async () => {
  return db.insert(benchBulk).values(makeBulkRows()).run();
}, 10, () => {
  client.prepare('DELETE FROM bench_bulk').run();
}));

client.close();

writeFileSync('drizzle-results.json', JSON.stringify(results, null, 2));
console.log('Wrote drizzle-results.json');
