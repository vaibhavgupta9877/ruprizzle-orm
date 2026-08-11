import { PrismaClient } from '@prisma/client';
import { performance } from 'node:perf_hooks';
import { writeFileSync } from 'node:fs';

const prisma = new PrismaClient();

async function bench(name, fn, iters, setup) {
  // Warm up
  for (let i = 0; i < 3; i++) {
    if (setup) await setup();
    const r = await fn();
    if (Array.isArray(r) && i === 2 && (name.includes('find_many') || name.includes('include'))) {
      if (name.includes('find_many')) console.assert(r.length === 1000, `${name}: expected 1000 got ${r.length}`);
      if (name.includes('include')) console.assert(r.length === 1000 && r[0].posts?.length === 10, `${name}: unexpected result`);
    }
  }

  const start = performance.now();
  for (let i = 0; i < iters; i++) {
    if (setup) await setup();
    await fn();
  }
  const total = performance.now() - start;
  const avg = total / iters;
  console.log(`${name}: ${avg.toFixed(3)} ms/op (total ${total.toFixed(1)} ms, ${iters} iters)`);
  return { orm: 'prisma', operation: name, iters, total_ms: total, avg_ms: avg };
}

function makeBulkRows() {
  return Array.from({ length: 1000 }, (_, i) => ({
    id: i + 1,
    name: `bulk-${i}`,
    n: i * 3,
  }));
}

const results = [];

(async () => {
  await prisma.$connect();

  results.push(await bench('select_by_pk', () => prisma.user.findUnique({ where: { id: 500 } }), 1000));

  results.push(await bench('find_many_1000', () => prisma.user.findMany(), 50));

  results.push(await bench('find_filtered_ordered', () =>
    prisma.user.findMany({
      where: { age: { gt: 18 } },
      orderBy: { email: 'asc' },
    }), 50));

  results.push(await bench('include_posts', () =>
    prisma.user.findMany({
      include: { posts: true },
    }), 10));

  results.push(await bench('bulk_insert_1000', () =>
    prisma.benchBulk.createMany({ data: makeBulkRows() }), 10,
    () => prisma.benchBulk.deleteMany()));

  await prisma.$disconnect();

  writeFileSync('prisma-results.json', JSON.stringify(results, null, 2));
  console.log('Wrote prisma-results.json');
})();
