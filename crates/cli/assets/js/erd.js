/**
 * Ruprizzle Studio schema graph.
 *
 * A dependency-free SVG entity-relationship diagram in the style of Supabase's
 * schema visualizer and Prisma ERD: each model is a table node listing its
 * columns with PK / FK / UQ markers, and each relation is one edge drawn from the
 * foreign-key column to the column it references, with crow's-foot cardinality at
 * both ends.
 *
 * Layout is a layered (Sugiyama-style) pass per connected component: cycles are
 * broken, nodes are layered by longest path so referenced tables sit left of the
 * tables pointing at them, layers are ordered by barycenter sweeps to cut
 * crossings, and vertical positions are relaxed towards their neighbours. Tall
 * layers wrap into extra columns. Dragged positions persist in localStorage.
 *
 * Every string from the schema reaches the page through `textContent` or an
 * attribute setter, never through HTML parsing.
 */
(function () {
  'use strict';

  const SVG_NS = 'http://www.w3.org/2000/svg';

  const C = {
    canvas: '#09090b',
    node: '#121215',
    header: '#18181b',
    border: '#2a2a31',
    borderActive: '#6366f1',
    text: '#f4f4f5',
    muted: '#a1a1aa',
    faint: '#71717a',
    edge: '#52525b',
    edgeActive: '#818cf8',
    pk: '#f59e0b',
    fk: '#818cf8',
    uq: '#2dd4bf',
    enumC: '#c084fc',
    rowHover: '#1d1d23',
  };
  const SANS = "Inter, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif";
  const MONO = "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace";

  const HEAD_H = 40;
  const ROW_H = 24;
  const PAD_B = 8;
  const MIN_W = 230;
  const MAX_W = 420;
  const GAP_X = 130;
  const GAP_Y = 36;
  const STUB = 20;
  const MIN_ZOOM = 0.1;
  const MAX_ZOOM = 2.5;

  const reduceMotion =
    window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // --------------------------------------------------------------- helpers --

  function svg(tag, attrs, parent) {
    const node = document.createElementNS(SVG_NS, tag);
    if (attrs) for (const k in attrs) node.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(node);
    return node;
  }

  function html(tag, className, parent, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    if (parent) parent.appendChild(node);
    return node;
  }

  function storage() {
    try {
      return window.localStorage;
    } catch (_) {
      return null;
    }
  }

  function loadJSON(key, fallback) {
    try {
      const s = storage();
      const raw = s && s.getItem(key);
      return raw ? JSON.parse(raw) : fallback;
    } catch (_) {
      return fallback;
    }
  }

  function saveJSON(key, value) {
    try {
      const s = storage();
      if (s) s.setItem(key, JSON.stringify(value));
    } catch (_) {
      /* storage full or blocked: positions just do not persist */
    }
  }

  function hashString(s) {
    let h = 2166136261;
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 16777619);
    }
    return (h >>> 0).toString(36);
  }

  function hue(name) {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) % 360;
    return h;
  }

  const measureCtx = document.createElement('canvas').getContext('2d');
  function textWidth(text, font) {
    measureCtx.font = font;
    return measureCtx.measureText(text).width;
  }

  function modelUrl(name) {
    return '/studio/models/' + encodeURIComponent(name);
  }

  function kindLabel(kind) {
    return { one_to_one: 'one-to-one', many_to_one: 'many-to-one', many_to_many: 'many-to-many' }[kind] || kind;
  }

  // ---------------------------------------------------------------- layout --

  /**
   * Positions `nodes` (objects with `id`, `w`, `h`) given directed `links`
   * (`[from, to]`, drawn left to right). Writes `x` / `y` onto each node.
   */
  function layoutGraph(nodes, links) {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const adj = new Map(nodes.map((n) => [n.id, new Set()]));
    const out = new Map(nodes.map((n) => [n.id, new Set()]));
    for (const [a, b] of links) {
      if (a === b || !byId.has(a) || !byId.has(b)) continue;
      adj.get(a).add(b);
      adj.get(b).add(a);
      out.get(a).add(b);
    }

    // Connected components, largest first; singletons are gathered separately.
    const seen = new Set();
    const components = [];
    const singles = [];
    for (const n of nodes) {
      if (seen.has(n.id)) continue;
      const comp = [];
      const stack = [n.id];
      seen.add(n.id);
      while (stack.length) {
        const v = stack.pop();
        comp.push(v);
        for (const w of adj.get(v)) {
          if (!seen.has(w)) {
            seen.add(w);
            stack.push(w);
          }
        }
      }
      if (comp.length === 1) singles.push(byId.get(comp[0]));
      else components.push(comp);
    }
    components.sort((a, b) => b.length - a.length);

    let offsetY = 0;
    let maxRight = 0;
    for (const comp of components) {
      const box = layoutComponent(comp, byId, out, adj);
      for (const id of comp) byId.get(id).y += offsetY;
      offsetY += box.h + GAP_Y * 3;
      maxRight = Math.max(maxRight, box.w);
    }

    // Unrelated tables go in a grid underneath.
    if (singles.length) {
      const rowWidth = Math.max(maxRight, 1100);
      let x = 0;
      let y = offsetY;
      let rowH = 0;
      for (const n of singles) {
        if (x > 0 && x + n.w > rowWidth) {
          x = 0;
          y += rowH + GAP_Y;
          rowH = 0;
        }
        n.x = x;
        n.y = y;
        x += n.w + GAP_Y * 1.5;
        rowH = Math.max(rowH, n.h);
      }
    }
  }

  function layoutComponent(ids, byId, out, adj) {
    const idSet = new Set(ids);

    // 1. Break cycles: DFS, dropping back edges.
    const succ = new Map(ids.map((id) => [id, []]));
    const state = new Map();
    const visit = (root) => {
      const stack = [[root, Array.from(out.get(root)).filter((w) => idSet.has(w)), 0]];
      state.set(root, 1);
      while (stack.length) {
        const top = stack[stack.length - 1];
        const [v, kids] = top;
        if (top[2] >= kids.length) {
          state.set(v, 2);
          stack.pop();
          continue;
        }
        const w = kids[top[2]++];
        const s = state.get(w);
        if (s === 1) continue; // back edge
        succ.get(v).push(w);
        if (!s) {
          state.set(w, 1);
          stack.push([w, Array.from(out.get(w)).filter((x) => idSet.has(x)), 0]);
        }
      }
    };
    // Start from nodes nothing points at, so natural roots stay roots.
    const indeg = new Map(ids.map((id) => [id, 0]));
    for (const id of ids) for (const w of out.get(id)) if (idSet.has(w)) indeg.set(w, indeg.get(w) + 1);
    const order = ids.slice().sort((a, b) => indeg.get(a) - indeg.get(b));
    for (const id of order) if (!state.get(id)) visit(id);

    const pred = new Map(ids.map((id) => [id, []]));
    for (const [v, ws] of succ) for (const w of ws) pred.get(w).push(v);

    // 2. Longest-path layering over the acyclic graph.
    const layer = new Map();
    const topo = [];
    const deg = new Map(ids.map((id) => [id, pred.get(id).length]));
    const queue = ids.filter((id) => deg.get(id) === 0);
    while (queue.length) {
      const v = queue.shift();
      topo.push(v);
      for (const w of succ.get(v)) {
        deg.set(w, deg.get(w) - 1);
        if (deg.get(w) === 0) queue.push(w);
      }
    }
    for (const v of topo) {
      let l = 0;
      for (const p of pred.get(v)) l = Math.max(l, layer.get(p) + 1);
      layer.set(v, l);
    }
    // Pull sources right, next to what they point at, to shorten edges.
    for (let i = topo.length - 1; i >= 0; i--) {
      const v = topo[i];
      if (pred.get(v).length === 0 && succ.get(v).length) {
        let m = Infinity;
        for (const w of succ.get(v)) m = Math.min(m, layer.get(w));
        layer.set(v, m - 1);
      }
    }
    let minLayer = Infinity;
    for (const l of layer.values()) minLayer = Math.min(minLayer, l);
    const layers = [];
    for (const v of topo) {
      const l = layer.get(v) - minLayer;
      (layers[l] = layers[l] || []).push(v);
    }

    // 3. Order within layers: barycenter sweeps over all neighbours.
    const pos = new Map();
    const index = () => {
      for (const L of layers) L.forEach((v, i) => pos.set(v, (i + 0.5) / L.length));
    };
    index();
    for (let iter = 0; iter < 16; iter++) {
      const seq = iter % 2 === 0 ? layers : layers.slice().reverse();
      for (const L of seq) {
        const bc = new Map();
        for (const v of L) {
          let sum = 0;
          let n = 0;
          for (const w of adj.get(v)) {
            if (!idSet.has(w) || layer.get(w) === layer.get(v)) continue;
            sum += pos.get(w);
            n++;
          }
          bc.set(v, n ? sum / n : pos.get(v));
        }
        L.sort((a, b) => bc.get(a) - bc.get(b));
        L.forEach((v, i) => pos.set(v, (i + 0.5) / L.length));
      }
    }

    // 4. Wrap tall layers into extra columns.
    let totalH = 0;
    let tallest = 0;
    for (const id of ids) {
      totalH += byId.get(id).h + GAP_Y;
      tallest = Math.max(tallest, byId.get(id).h);
    }
    const maxColH = Math.max(tallest, 900, totalH / Math.max(2, Math.ceil(Math.sqrt(ids.length))));
    const columns = [];
    for (const L of layers) {
      if (!L) continue;
      let col = [];
      let h = 0;
      for (const v of L) {
        const nh = byId.get(v).h + GAP_Y;
        if (col.length && h + nh > maxColH) {
          columns.push(col);
          col = [];
          h = 0;
        }
        col.push(v);
        h += nh;
      }
      if (col.length) columns.push(col);
    }

    // 5. Coordinates: x by column, y relaxed towards neighbours.
    let x = 0;
    for (const col of columns) {
      let w = 0;
      for (const v of col) w = Math.max(w, byId.get(v).w);
      for (const v of col) byId.get(v).x = x + (w - byId.get(v).w) / 2;
      x += w + GAP_X;
    }
    let maxH = 0;
    const colHeights = columns.map((col) => {
      let h = -GAP_Y;
      for (const v of col) h += byId.get(v).h + GAP_Y;
      maxH = Math.max(maxH, h);
      return h;
    });
    columns.forEach((col, ci) => {
      let y = (maxH - colHeights[ci]) / 2;
      for (const v of col) {
        byId.get(v).y = y;
        y += byId.get(v).h + GAP_Y;
      }
    });
    for (let iter = 0; iter < 24; iter++) {
      for (const col of columns) {
        const want = col.map((v) => {
          const n = byId.get(v);
          let sum = 0;
          let k = 0;
          for (const w of adj.get(v)) {
            if (!idSet.has(w)) continue;
            const m = byId.get(w);
            sum += m.y + m.h / 2;
            k++;
          }
          return k ? sum / k - n.h / 2 : n.y;
        });
        const f = [];
        for (let i = 0; i < col.length; i++) {
          const prev = i ? f[i - 1] + byId.get(col[i - 1]).h + GAP_Y : -Infinity;
          f.push(Math.max(want[i], prev));
        }
        const b = [];
        for (let i = col.length - 1; i >= 0; i--) {
          const next = i < col.length - 1 ? b[i + 1] - byId.get(col[i]).h - GAP_Y : Infinity;
          b[i] = Math.min(want[i], next);
        }
        let prevBottom = -Infinity;
        col.forEach((v, i) => {
          const n = byId.get(v);
          n.y = Math.max((f[i] + b[i]) / 2, prevBottom + GAP_Y);
          prevBottom = n.y + n.h;
        });
      }
    }

    // Normalise to the origin.
    let minY = Infinity;
    let maxY = -Infinity;
    let maxX = 0;
    for (const id of ids) {
      const n = byId.get(id);
      minY = Math.min(minY, n.y);
      maxY = Math.max(maxY, n.y + n.h);
      maxX = Math.max(maxX, n.x + n.w);
    }
    for (const id of ids) byId.get(id).y -= minY;
    return { w: maxX, h: maxY - minY };
  }

  // ------------------------------------------------------------------ view --

  class ErdView {
    constructor(root) {
      this.root = root;
      this.dataUrl = root.dataset.erdSource;
      this.stage = root.querySelector('[data-erd-stage]');
      this.status = root.querySelector('[data-erd-status]');
      this.opts = loadJSON('ruprizzle.erd.options', { keysOnly: false, navFields: false, enums: false });
      this.view = { x: 0, y: 0, k: 1 };
      this.nodes = new Map();
      this.edges = [];
      this.selected = null;
      this.searchHits = null;
      this.hover = null;
    }

    async init() {
      this.setStatus('Loading schema…');
      let data;
      try {
        const res = await fetch(this.dataUrl, { headers: { Accept: 'application/json' } });
        if (!res.ok) throw new Error('HTTP ' + res.status);
        data = await res.json();
      } catch (err) {
        this.setStatus('Could not load the schema graph: ' + err.message, true);
        return;
      }
      this.data = data;
      const signature = (data.models || [])
        .map((m) => m.name + ':' + m.fields.map((f) => f.name).join(','))
        .join('|');
      this.posKey = 'ruprizzle.erd.pos.' + hashString(signature);
      this.saved = loadJSON(this.posKey, {});

      if (!(data.models || []).length) {
        this.setStatus('The schema declares no models, so there is nothing to draw.', true);
        return;
      }
      this.setStatus('');
      this.buildDom();
      this.bindToolbar();
      this.bindCanvas();
      this.rebuild(false);

      if (!this.focusFromHash(false)) this.fit(false);
      window.addEventListener('hashchange', () => this.focusFromHash(true));
    }

    /** Selects and centres the model named in the URL hash (`/studio/erd#User`). */
    focusFromHash(animate) {
      let focus = '';
      try {
        focus = decodeURIComponent((location.hash || '').slice(1));
      } catch (_) {
        return false;
      }
      if (!focus || !this.nodes.has(focus)) return false;
      this.select(focus);
      this.centerOn(focus, Math.max(this.view.k, 0.9), animate);
      return true;
    }

    setStatus(message, isError) {
      if (!this.status) return;
      this.status.textContent = message;
      this.status.hidden = !message;
      this.status.classList.toggle('erd-status-error', !!isError);
    }

    buildDom() {
      this.svg = svg('svg', { class: 'erd-svg', role: 'img', 'aria-label': 'Schema entity-relationship diagram' }, this.stage);
      this.viewport = svg('g', {}, this.svg);
      this.edgeLayer = svg('g', { class: 'erd-edges' }, this.viewport);
      this.nodeLayer = svg('g', { class: 'erd-nodes' }, this.viewport);

      this.tooltip = html('div', 'erd-tooltip', this.stage);
      this.tooltip.hidden = true;

      this.details = html('aside', 'erd-details', this.stage);
      this.details.hidden = true;

      const mini = html('div', 'erd-minimap', this.stage);
      this.mini = svg('svg', { width: '100%', height: '100%' }, mini);
      this.miniNodes = svg('g', {}, this.mini);
      this.miniView = svg('rect', { class: 'erd-minimap-view' }, this.mini);
      this.miniBox = mini;
    }

    // ----------------------------------------------------------- building --

    visibleFields(model) {
      return model.fields.filter((f) => {
        if (f.is_relation) return this.opts.navFields;
        if (this.opts.keysOnly) return f.is_id || f.is_fk || f.is_unique;
        return true;
      });
    }

    rebuild(keepView) {
      const data = this.data;
      this.nodes.clear();
      this.edges = [];
      this.edgeLayer.replaceChildren();
      this.nodeLayer.replaceChildren();

      const headFont = '600 13px ' + SANS;
      const rowFont = '12px ' + MONO;

      for (const m of data.models) {
        const fields = this.visibleFields(m);
        let w = textWidth(m.name, headFont) + 90;
        if (m.table && m.table !== m.name) w = Math.max(w, textWidth(m.name, headFont) + textWidth(m.table, '11px ' + MONO) + 100);
        for (const f of fields) w = Math.max(w, 44 + textWidth(f.name, rowFont) + 24 + textWidth(f.type_name, rowFont) + 14);
        w = Math.min(MAX_W, Math.max(MIN_W, Math.ceil(w)));
        this.nodes.set(m.name, {
          id: m.name,
          kind: 'model',
          model: m,
          fields,
          w,
          h: HEAD_H + Math.max(1, fields.length) * ROW_H + PAD_B,
          x: 0,
          y: 0,
        });
      }
      if (this.opts.enums) {
        for (const e of data.enums || []) {
          const id = 'enum:' + e.name;
          let w = textWidth(e.name, headFont) + 90;
          for (const v of e.variants) w = Math.max(w, textWidth(v, rowFont) + 32);
          w = Math.min(MAX_W, Math.max(170, Math.ceil(w)));
          this.nodes.set(id, {
            id,
            kind: 'enum',
            enumDef: e,
            fields: e.variants.map((v) => ({ name: v, type_name: '' })),
            w,
            h: HEAD_H + Math.max(1, e.variants.length) * ROW_H + PAD_B,
            x: 0,
            y: 0,
          });
        }
      }

      for (const r of data.relations || []) {
        if (!this.nodes.has(r.from) || !this.nodes.has(r.to)) continue;
        this.edges.push({ kind: 'relation', rel: r, from: r.from, to: r.to, fromField: r.from_fields[0], toField: r.to_fields[0] });
      }
      if (this.opts.enums) {
        for (const m of data.models) {
          for (const f of m.fields) {
            if (f.enum_name && this.nodes.has('enum:' + f.enum_name)) {
              this.edges.push({ kind: 'enum', from: m.name, to: 'enum:' + f.enum_name, fromField: f.name, toField: null });
            }
          }
        }
      }

      // Referenced tables (and enums) go left of the tables that use them.
      const links = this.edges.map((e) => [e.to, e.from]);
      layoutGraph(Array.from(this.nodes.values()), links);
      for (const n of this.nodes.values()) {
        const p = this.saved[n.id];
        if (p) {
          n.x = p[0];
          n.y = p[1];
        }
      }

      for (const n of this.nodes.values()) this.drawNode(n);
      for (const e of this.edges) this.drawEdge(e);
      this.applyHighlight();
      this.drawMinimap();
      if (!keepView) this.fit(false);
      this.syncToolbar();
    }

    drawNode(n) {
      const g = svg('g', { class: 'erd-node', transform: `translate(${n.x},${n.y})`, 'data-id': n.id }, this.nodeLayer);
      n.g = g;
      const isEnum = n.kind === 'enum';
      const accent = isEnum ? C.enumC : `hsl(${hue(n.id)}, 70%, 64%)`;

      svg('rect', { class: 'erd-node-shadow', x: 0, y: 4, width: n.w, height: n.h, rx: 10, fill: '#000', opacity: 0.35 }, g);
      n.body = svg('rect', { class: 'erd-node-body', width: n.w, height: n.h, rx: 10, fill: C.node, stroke: C.border, 'stroke-width': 1 }, g);
      svg('path', { d: `M0,10 a10,10 0 0 1 10,-10 h${n.w - 20} a10,10 0 0 1 10,10 v${HEAD_H - 10} h-${n.w} z`, fill: C.header }, g);
      svg('rect', { x: 10, y: 0, width: n.w - 20, height: 2.5, rx: 1.25, fill: accent }, g);
      svg('line', { x1: 0, x2: n.w, y1: HEAD_H, y2: HEAD_H, stroke: C.border }, g);

      const header = svg('rect', { class: 'erd-node-header', width: n.w, height: HEAD_H, fill: 'transparent', 'data-drag': n.id }, g);
      header.style.cursor = 'grab';

      const title = svg('text', { x: 14, y: 25, fill: C.text, 'font-family': SANS, 'font-size': 13, 'font-weight': 600, 'pointer-events': 'none' }, g);
      title.textContent = isEnum ? n.enumDef.name : n.id;
      if (isEnum) {
        const tag = svg('text', { x: n.w - 12, y: 25, fill: C.enumC, 'text-anchor': 'end', 'font-family': MONO, 'font-size': 10, 'pointer-events': 'none' }, g);
        tag.textContent = 'enum';
      } else {
        const m = n.model;
        if (m.table && m.table !== m.name) {
          const t = svg('text', { x: 14 + textWidth(m.name, '600 13px ' + SANS) + 8, y: 25, fill: C.faint, 'font-family': MONO, 'font-size': 11, 'pointer-events': 'none' }, g);
          t.textContent = m.table;
        }
        const link = svg('a', { href: modelUrl(m.name), class: 'erd-open' }, g);
        const lt = svg('title', {}, link);
        lt.textContent = 'Open ' + m.name + ' table';
        svg('rect', { x: n.w - 34, y: 8, width: 24, height: 24, rx: 5, fill: 'transparent', class: 'erd-open-bg' }, link);
        svg('path', { d: `M${n.w - 26},${24} l8,-8 m-5,0 h5 v5`, stroke: C.muted, 'stroke-width': 1.5, fill: 'none', 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, link);
      }

      n.rowY = new Map();
      if (!n.fields.length) {
        const empty = svg('text', { x: 14, y: HEAD_H + 16, fill: C.faint, 'font-family': SANS, 'font-size': 11, 'font-style': 'italic' }, g);
        empty.textContent = this.opts.keysOnly ? 'no key columns' : 'no columns';
      }
      n.fields.forEach((f, i) => {
        const y = HEAD_H + 4 + i * ROW_H;
        n.rowY.set(f.name, y + ROW_H / 2);
        const row = svg('g', { class: 'erd-row', 'data-field': f.name }, g);
        svg('rect', { x: 1, y, width: n.w - 2, height: ROW_H, fill: 'transparent', class: 'erd-row-bg' }, row);
        if (isEnum) {
          const t = svg('text', { x: 14, y: y + 16, fill: C.muted, 'font-family': MONO, 'font-size': 12 }, row);
          t.textContent = f.name;
          return;
        }
        const badge = f.is_id ? ['PK', C.pk] : f.is_fk ? ['FK', C.fk] : f.is_unique ? ['UQ', C.uq] : null;
        if (badge) {
          svg('rect', { x: 10, y: y + 5, width: 24, height: 14, rx: 3, fill: badge[1], opacity: 0.16 }, row);
          const b = svg('text', { x: 22, y: y + 15.5, fill: badge[1], 'text-anchor': 'middle', 'font-family': SANS, 'font-size': 9, 'font-weight': 700 }, row);
          b.textContent = badge[0];
        }
        const name = svg('text', {
          x: 42,
          y: y + 16,
          fill: f.is_relation ? C.fk : C.text,
          'font-family': MONO,
          'font-size': 12,
          'font-style': f.is_relation ? 'italic' : 'normal',
        }, row);
        name.textContent = f.name;
        const type = svg('text', {
          x: n.w - 12,
          y: y + 16,
          fill: f.enum_name ? C.enumC : f.is_relation ? C.fk : C.faint,
          'text-anchor': 'end',
          'font-family': MONO,
          'font-size': 11,
        }, row);
        type.textContent = f.type_name;
        const tip = svg('title', {}, row);
        tip.textContent = f.column && f.column !== f.name ? `${f.name} → column ${f.column}` : f.name;
      });
    }

    anchor(node, field) {
      const y = field && node.rowY.has(field) ? node.rowY.get(field) : HEAD_H / 2;
      return node.y + y;
    }

    edgePath(e) {
      const a = this.nodes.get(e.from);
      const b = this.nodes.get(e.to);
      const ya = this.anchor(a, e.fromField);
      const yb = this.anchor(b, e.toField);
      let xa;
      let xb;
      let sa;
      let sb;
      if (a === b) {
        xa = xb = a.x + a.w;
        sa = sb = 1;
      } else if (a.x + a.w + 20 < b.x) {
        xa = a.x + a.w; sa = 1; xb = b.x; sb = -1;
      } else if (b.x + b.w + 20 < a.x) {
        xa = a.x; sa = -1; xb = b.x + b.w; sb = 1;
      } else {
        // Stacked in one column: leave and re-enter on the same side.
        const right = a.x + a.w / 2 <= b.x + b.w / 2;
        xa = right ? a.x + a.w : a.x; sa = right ? 1 : -1;
        xb = right ? b.x + b.w : b.x; sb = sa;
      }
      const pa = xa + sa * STUB;
      const pb = xb + sb * STUB;
      let dx = Math.max(40, Math.abs(pb - pa) * 0.45);
      if (sa === sb) dx = Math.max(dx, 50 + Math.abs(yb - ya) * 0.15);
      let d;
      if (a === b && Math.abs(ya - yb) < 1) {
        const loop = 36;
        d = `M${xa},${ya} H${pa} C${pa + loop},${ya} ${pa + loop},${ya - 30} ${pa},${ya - 30} H${xa}`;
        return { d, xa, ya, sa, xb, yb: ya - 30, sb };
      }
      d = `M${xa},${ya} H${pa} C${pa + sa * dx},${ya} ${pb + sb * dx},${yb} ${pb},${yb} H${xb}`;
      return { d, xa, ya, sa, xb, yb, sb };
    }

    /** Crow's-foot marker at (x, y) on a border, pointing out along `s`. */
    marker(g, x, y, s, type, color) {
      const p = (d) => svg('path', { d, stroke: color, 'stroke-width': 1.5, fill: 'none', class: 'erd-marker' }, g);
      const circle = (cx) => svg('circle', { cx, cy: y, r: 3.5, fill: C.canvas, stroke: color, 'stroke-width': 1.5, class: 'erd-marker' }, g);
      const bar = (bx) => p(`M${bx},${y - 6} V${y + 6}`);
      if (type === 'one') {
        bar(x + s * 7);
        bar(x + s * 12);
      } else if (type === 'zero-one') {
        bar(x + s * 7);
        circle(x + s * 15);
      } else if (type === 'many') {
        p(`M${x},${y - 7} L${x + s * 11},${y} L${x},${y + 7}`);
        circle(x + s * 15);
      }
    }

    drawEdge(e) {
      const g = svg('g', { class: 'erd-edge' + (e.kind === 'enum' ? ' erd-edge-enum' : '') }, this.edgeLayer);
      e.g = g;
      this.renderEdge(e);
    }

    renderEdge(e) {
      const g = e.g;
      g.replaceChildren();
      const geo = this.edgePath(e);
      const isEnum = e.kind === 'enum';
      const color = isEnum ? C.enumC : C.edge;
      e.hit = svg('path', { d: geo.d, stroke: 'transparent', 'stroke-width': 14, fill: 'none', class: 'erd-edge-hit' }, g);
      e.line = svg('path', {
        d: geo.d,
        stroke: color,
        'stroke-width': 1.5,
        fill: 'none',
        class: 'erd-edge-line',
        opacity: isEnum ? 0.55 : 1,
      }, g);
      if (isEnum || (e.rel && e.rel.kind === 'many_to_many')) e.line.setAttribute('stroke-dasharray', '5 4');
      if (isEnum) return;
      const r = e.rel;
      const fromType = r.kind === 'one_to_one' ? 'zero-one' : 'many';
      const toType = r.kind === 'many_to_many' ? 'many' : r.optional ? 'zero-one' : 'one';
      this.marker(g, geo.xa, geo.ya, geo.sa, fromType, color);
      this.marker(g, geo.xb, geo.yb, geo.sb, toType, color);
    }

    edgesOf(id) {
      return this.edges.filter((e) => e.from === id || e.to === id);
    }

    moveNode(n, x, y) {
      n.x = x;
      n.y = y;
      n.g.setAttribute('transform', `translate(${x},${y})`);
      for (const e of this.edgesOf(n.id)) this.renderEdge(e);
    }

    // --------------------------------------------------------- highlighting --

    focusSet() {
      if (this.searchHits) return { nodes: this.searchHits, edges: null };
      const id = this.hover || this.selected;
      if (!id) return null;
      const nodes = new Set([id]);
      const edges = new Set();
      for (const e of this.edgesOf(id)) {
        edges.add(e);
        nodes.add(e.from);
        nodes.add(e.to);
      }
      return { nodes, edges };
    }

    applyHighlight(edgeFocus) {
      const f = this.focusSet();
      for (const n of this.nodes.values()) {
        const on = !f || f.nodes.has(n.id);
        n.g.classList.toggle('erd-dim', !on);
        const active = n.id === this.selected || (this.searchHits && this.searchHits.has(n.id));
        n.body.setAttribute('stroke', active ? C.borderActive : C.border);
        n.body.setAttribute('stroke-width', active ? 1.5 : 1);
      }
      for (const e of this.edges) {
        let on;
        if (edgeFocus) on = e === edgeFocus;
        else if (!f) on = true;
        else if (f.edges) on = f.edges.has(e);
        else on = f.nodes.has(e.from) && f.nodes.has(e.to);
        const hot = edgeFocus ? e === edgeFocus : f && f.edges && f.edges.has(e);
        e.g.classList.toggle('erd-dim', !on);
        const color = e.kind === 'enum' ? C.enumC : hot ? C.edgeActive : C.edge;
        e.g.querySelectorAll('.erd-edge-line, .erd-marker').forEach((el) => {
          el.setAttribute('stroke', color);
          if (el.classList.contains('erd-edge-line')) el.setAttribute('stroke-width', hot ? 2.25 : 1.5);
        });
      }
      if (edgeFocus) {
        for (const n of this.nodes.values()) {
          n.g.classList.toggle('erd-dim', n.id !== edgeFocus.from && n.id !== edgeFocus.to);
        }
      }
    }

    select(id) {
      this.selected = id;
      this.applyHighlight();
      this.renderDetails();
    }

    renderDetails() {
      const d = this.details;
      d.replaceChildren();
      const n = this.selected && this.nodes.get(this.selected);
      if (!n) {
        d.hidden = true;
        return;
      }
      d.hidden = false;
      const head = html('div', 'erd-details-head', d);
      const titleWrap = html('div', '', head);
      html('div', 'erd-details-kicker', titleWrap, n.kind === 'enum' ? 'Enum' : 'Model');
      html('h3', 'erd-details-title', titleWrap, n.kind === 'enum' ? n.enumDef.name : n.id);
      const close = html('button', 'btn btn-secondary btn-sm', head, '✕');
      close.type = 'button';
      close.setAttribute('aria-label', 'Close details');
      close.addEventListener('click', () => this.select(null));

      if (n.kind === 'enum') {
        html('div', 'erd-details-meta', d, n.enumDef.variants.length + ' variants');
        const list = html('div', 'erd-details-chips', d);
        for (const v of n.enumDef.variants) html('span', 'erd-chip', list, v);
        return;
      }
      const m = n.model;
      const cols = m.fields.filter((f) => !f.is_relation).length;
      html('div', 'erd-details-meta', d, `table ${m.table} · ${cols} column${cols === 1 ? '' : 's'}`);
      if (m.docs) html('p', 'erd-details-docs', d, m.docs);

      const rels = this.data.relations.filter((r) => r.from === m.name || r.to === m.name);
      html('div', 'erd-details-section', d, rels.length ? 'Relations' : 'No relations');
      for (const r of rels) {
        const outgoing = r.from === m.name;
        const other = outgoing ? r.to : r.from;
        const item = html('button', 'erd-rel', d);
        item.type = 'button';
        const top = html('div', 'erd-rel-top', item);
        html('span', 'erd-rel-dir', top, outgoing ? 'references' : 'referenced by');
        html('span', 'erd-rel-model', top, other);
        const keys = r.from_fields.map((f, i) => `${r.from}.${f} → ${r.to}.${r.to_fields[i] || '?'}`).join(', ');
        html('div', 'erd-rel-keys', item, r.through ? `via ${r.through}` : keys);
        html('div', 'erd-rel-meta', item, `${kindLabel(r.kind)}${r.optional ? ' · optional' : ''} · ON DELETE ${r.on_delete}`);
        item.addEventListener('click', () => {
          this.select(other);
          this.centerOn(other, this.view.k, true);
        });
      }
      const open = html('a', 'btn btn-primary btn-sm erd-details-open', d, 'Open table →');
      open.href = modelUrl(m.name);
    }

    showTooltip(e, clientX, clientY) {
      const r = e.rel;
      const t = this.tooltip;
      t.replaceChildren();
      if (e.kind === 'enum') {
        html('div', 'erd-tip-title', t, `${e.from}.${e.fromField}`);
        html('div', 'erd-tip-meta', t, 'typed by enum ' + e.to.slice(5));
      } else {
        html('div', 'erd-tip-title', t, r.from_fields.map((f, i) => `${r.from}.${f} → ${r.to}.${r.to_fields[i] || '?'}`).join('\n') || `${r.from} ↔ ${r.to}`);
        html('div', 'erd-tip-meta', t, `${kindLabel(r.kind)}${r.optional ? ', optional' : ''}${r.through ? ', via ' + r.through : ''}`);
        html('div', 'erd-tip-meta', t, `ON DELETE ${r.on_delete} · ON UPDATE ${r.on_update}`);
        if (r.relation_name) html('div', 'erd-tip-name', t, r.relation_name);
      }
      const box = this.stage.getBoundingClientRect();
      t.hidden = false;
      t.style.left = Math.min(clientX - box.left + 14, box.width - t.offsetWidth - 8) + 'px';
      t.style.top = Math.min(clientY - box.top + 14, box.height - t.offsetHeight - 8) + 'px';
    }

    // ------------------------------------------------------------- viewport --

    applyView() {
      const v = this.view;
      this.viewport.setAttribute('transform', `translate(${v.x},${v.y}) scale(${v.k})`);
      const zoomLabel = this.root.querySelector('[data-erd-zoom-label]');
      if (zoomLabel) zoomLabel.textContent = Math.round(v.k * 100) + '%';
      this.drawMinimapView();
    }

    animateTo(target, animate) {
      if (!animate || reduceMotion) {
        this.view = target;
        this.applyView();
        return;
      }
      const from = { ...this.view };
      const start = performance.now();
      const dur = 260;
      cancelAnimationFrame(this.anim);
      const step = (now) => {
        const t = Math.min(1, (now - start) / dur);
        const e = 1 - Math.pow(1 - t, 3);
        this.view = {
          x: from.x + (target.x - from.x) * e,
          y: from.y + (target.y - from.y) * e,
          k: from.k + (target.k - from.k) * e,
        };
        this.applyView();
        if (t < 1) this.anim = requestAnimationFrame(step);
      };
      this.anim = requestAnimationFrame(step);
    }

    bounds(ids) {
      let x0 = Infinity;
      let y0 = Infinity;
      let x1 = -Infinity;
      let y1 = -Infinity;
      for (const n of this.nodes.values()) {
        if (ids && !ids.has(n.id)) continue;
        x0 = Math.min(x0, n.x);
        y0 = Math.min(y0, n.y);
        x1 = Math.max(x1, n.x + n.w);
        y1 = Math.max(y1, n.y + n.h);
      }
      if (x0 === Infinity) return { x: 0, y: 0, w: 1, h: 1 };
      return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    }

    fit(animate, ids) {
      const b = this.bounds(ids);
      // Leave room for the details panel when it is open.
      const panel = this.details && !this.details.hidden && this.stage.clientWidth > 900 ? this.details.offsetWidth + 16 : 0;
      const W = (this.stage.clientWidth || 800) - panel;
      const H = this.stage.clientHeight || 600;
      const pad = 60;
      const k = Math.max(MIN_ZOOM, Math.min(1.1, (W - pad * 2) / b.w, (H - pad * 2) / b.h));
      this.animateTo({ k, x: (W - b.w * k) / 2 - b.x * k, y: (H - b.h * k) / 2 - b.y * k }, animate);
    }

    centerOn(id, k, animate) {
      const n = this.nodes.get(id);
      if (!n) return;
      const W = this.stage.clientWidth || 800;
      const H = this.stage.clientHeight || 600;
      const kk = k || this.view.k;
      this.animateTo({ k: kk, x: W / 2 - (n.x + n.w / 2) * kk, y: H / 2 - (n.y + n.h / 2) * kk }, animate);
    }

    zoomAt(factor, cx, cy) {
      const v = this.view;
      const k = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, v.k * factor));
      const f = k / v.k;
      this.view = { k, x: cx - (cx - v.x) * f, y: cy - (cy - v.y) * f };
      this.applyView();
    }

    toWorld(clientX, clientY) {
      const r = this.svg.getBoundingClientRect();
      return { x: (clientX - r.left - this.view.x) / this.view.k, y: (clientY - r.top - this.view.y) / this.view.k };
    }

    // -------------------------------------------------------------- minimap --

    drawMinimap() {
      this.miniNodes.replaceChildren();
      const b = this.bounds();
      const pad = 40;
      this.miniWorld = { x: b.x - pad, y: b.y - pad, w: b.w + pad * 2, h: b.h + pad * 2 };
      this.mini.setAttribute('viewBox', `${this.miniWorld.x} ${this.miniWorld.y} ${this.miniWorld.w} ${this.miniWorld.h}`);
      this.mini.setAttribute('preserveAspectRatio', 'xMidYMid meet');
      for (const n of this.nodes.values()) {
        svg('rect', { x: n.x, y: n.y, width: n.w, height: n.h, rx: 8, fill: n.kind === 'enum' ? '#3b2a4d' : '#27272a' }, this.miniNodes);
      }
      this.drawMinimapView();
    }

    drawMinimapView() {
      if (!this.miniView || !this.stage) return;
      const v = this.view;
      const W = this.stage.clientWidth;
      const H = this.stage.clientHeight;
      this.miniView.setAttribute('x', -v.x / v.k);
      this.miniView.setAttribute('y', -v.y / v.k);
      this.miniView.setAttribute('width', W / v.k);
      this.miniView.setAttribute('height', H / v.k);
      const stroke = this.miniWorld ? Math.max(this.miniWorld.w, this.miniWorld.h) / 150 : 4;
      this.miniView.setAttribute('stroke-width', stroke);
    }

    // ------------------------------------------------------------ events --

    bindCanvas() {
      const stage = this.stage;
      let drag = null;

      this.svg.addEventListener('pointerdown', (ev) => {
        if (ev.button !== 0) return;
        if (ev.target.closest('a')) return;
        const handle = ev.target.closest('[data-drag]');
        const nodeEl = ev.target.closest('.erd-node');
        const start = { cx: ev.clientX, cy: ev.clientY, moved: false };
        if (nodeEl) {
          const n = this.nodes.get(nodeEl.dataset.id);
          const w = this.toWorld(ev.clientX, ev.clientY);
          drag = { type: handle ? 'node' : 'nodeBody', n, ox: w.x - n.x, oy: w.y - n.y, ...start };
          if (handle) handle.style.cursor = 'grabbing';
        } else {
          drag = { type: 'pan', vx: this.view.x, vy: this.view.y, ...start };
          stage.classList.add('erd-panning');
        }
        this.svg.setPointerCapture(ev.pointerId);
      });

      this.svg.addEventListener('pointermove', (ev) => {
        if (!drag) {
          this.onHover(ev);
          return;
        }
        const dx = ev.clientX - drag.cx;
        const dy = ev.clientY - drag.cy;
        if (!drag.moved && Math.hypot(dx, dy) < 4) return;
        drag.moved = true;
        this.tooltip.hidden = true;
        if (drag.type === 'pan') {
          this.view = { ...this.view, x: drag.vx + dx, y: drag.vy + dy };
          this.applyView();
        } else {
          // Dragging from a row still moves the whole table.
          const w = this.toWorld(ev.clientX, ev.clientY);
          this.moveNode(drag.n, Math.round(w.x - drag.ox), Math.round(w.y - drag.oy));
        }
      });

      const end = (ev) => {
        if (!drag) return;
        stage.classList.remove('erd-panning');
        const d = drag;
        drag = null;
        const handle = this.svg.querySelector('[data-drag][style*="grabbing"]');
        if (handle) handle.style.cursor = 'grab';
        if (d.type !== 'pan' && d.moved) {
          this.saved[d.n.id] = [d.n.x, d.n.y];
          saveJSON(this.posKey, this.saved);
          this.drawMinimap();
        } else if (!d.moved) {
          if (d.type === 'pan') this.select(null);
          else this.select(d.n.id);
        }
        if (ev && ev.pointerId !== undefined && this.svg.hasPointerCapture(ev.pointerId)) {
          this.svg.releasePointerCapture(ev.pointerId);
        }
      };
      this.svg.addEventListener('pointerup', end);
      this.svg.addEventListener('pointercancel', end);

      this.svg.addEventListener('dblclick', (ev) => {
        const nodeEl = ev.target.closest('.erd-node');
        if (nodeEl && nodeEl.dataset.id.indexOf('enum:') !== 0) {
          window.location.href = modelUrl(nodeEl.dataset.id);
        } else if (!nodeEl) {
          this.fit(true);
        }
      });

      this.svg.addEventListener('pointerleave', () => {
        this.tooltip.hidden = true;
        if (this.hover || this.edgeHover) {
          this.hover = null;
          this.edgeHover = null;
          this.applyHighlight();
        }
      });

      this.svg.addEventListener(
        'wheel',
        (ev) => {
          ev.preventDefault();
          const r = this.svg.getBoundingClientRect();
          // Trackpad pinches arrive as ctrl+wheel with small deltas.
          const scale = ev.ctrlKey ? 0.01 : 0.0015;
          const delta = ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaY;
          this.zoomAt(Math.exp(-delta * scale), ev.clientX - r.left, ev.clientY - r.top);
        },
        { passive: false }
      );

      // Minimap: click or drag to move the viewport.
      let miniDrag = false;
      const miniMove = (ev) => {
        const r = this.miniBox.getBoundingClientRect();
        const mw = this.miniWorld;
        const s = Math.min(r.width / mw.w, r.height / mw.h);
        const ox = (r.width - mw.w * s) / 2;
        const oy = (r.height - mw.h * s) / 2;
        const wx = mw.x + (ev.clientX - r.left - ox) / s;
        const wy = mw.y + (ev.clientY - r.top - oy) / s;
        const W = this.stage.clientWidth;
        const H = this.stage.clientHeight;
        this.view = { ...this.view, x: W / 2 - wx * this.view.k, y: H / 2 - wy * this.view.k };
        this.applyView();
      };
      this.miniBox.addEventListener('pointerdown', (ev) => {
        miniDrag = true;
        this.miniBox.setPointerCapture(ev.pointerId);
        miniMove(ev);
      });
      this.miniBox.addEventListener('pointermove', (ev) => miniDrag && miniMove(ev));
      this.miniBox.addEventListener('pointerup', () => (miniDrag = false));

      window.addEventListener('resize', () => this.drawMinimapView());
    }

    onHover(ev) {
      const edgeEl = ev.target.closest('.erd-edge');
      if (edgeEl) {
        const e = this.edges.find((x) => x.g === edgeEl);
        if (e) {
          if (this.edgeHover !== e) {
            this.edgeHover = e;
            this.hover = null;
            this.applyHighlight(e);
          }
          this.showTooltip(e, ev.clientX, ev.clientY);
          return;
        }
      }
      this.tooltip.hidden = true;
      const nodeEl = ev.target.closest('.erd-node');
      const id = nodeEl ? nodeEl.dataset.id : null;
      if (id !== this.hover || this.edgeHover) {
        this.hover = id;
        this.edgeHover = null;
        if (!this.searchHits) this.applyHighlight();
      }
    }

    // -------------------------------------------------------------- toolbar --

    syncToolbar() {
      this.root.querySelectorAll('[data-erd-toggle]').forEach((btn) => {
        const on = !!this.opts[btn.dataset.erdToggle];
        btn.setAttribute('aria-pressed', on ? 'true' : 'false');
      });
      const enumBtn = this.root.querySelector('[data-erd-toggle="enums"]');
      if (enumBtn && !(this.data.enums || []).length) enumBtn.hidden = true;
      const stats = this.root.querySelector('[data-erd-stats]');
      if (stats) {
        const m = this.data.models.length;
        const r = this.data.relations.length;
        stats.textContent = `${m} model${m === 1 ? '' : 's'} · ${r} relation${r === 1 ? '' : 's'}`;
      }
    }

    bindToolbar() {
      const root = this.root;
      const on = (sel, fn) => root.querySelectorAll(sel).forEach((el) => el.addEventListener('click', fn));
      const center = () => ({ x: this.stage.clientWidth / 2, y: this.stage.clientHeight / 2 });

      on('[data-erd-action="zoom-in"]', () => this.zoomAt(1.25, center().x, center().y));
      on('[data-erd-action="zoom-out"]', () => this.zoomAt(0.8, center().x, center().y));
      on('[data-erd-action="fit"]', () => this.fit(true));
      on('[data-erd-action="relayout"]', () => {
        this.saved = {};
        saveJSON(this.posKey, this.saved);
        this.rebuild(true);
        this.fit(true);
      });
      on('[data-erd-action="export"]', () => this.exportSvg());
      root.querySelectorAll('[data-erd-toggle]').forEach((btn) =>
        btn.addEventListener('click', () => {
          const key = btn.dataset.erdToggle;
          this.opts[key] = !this.opts[key];
          saveJSON('ruprizzle.erd.options', this.opts);
          this.rebuild(true);
          if (this.selected && !this.nodes.has(this.selected)) this.select(null);
          else this.renderDetails();
        })
      );

      const search = root.querySelector('[data-erd-search]');
      if (search) {
        search.addEventListener('input', () => {
          const term = search.value.trim().toLowerCase();
          if (!term) {
            this.searchHits = null;
          } else {
            this.searchHits = new Set();
            for (const n of this.nodes.values()) {
              const name = (n.kind === 'enum' ? n.enumDef.name : n.id).toLowerCase();
              const table = n.model ? n.model.table.toLowerCase() : '';
              const fieldHit = (n.model ? n.model.fields : n.fields).some((f) => f.name.toLowerCase().includes(term));
              if (name.includes(term) || table.includes(term) || fieldHit) this.searchHits.add(n.id);
            }
          }
          this.applyHighlight();
        });
        search.addEventListener('keydown', (ev) => {
          if (ev.key === 'Enter' && this.searchHits && this.searchHits.size) {
            ev.preventDefault();
            if (this.searchHits.size === 1) {
              const id = this.searchHits.values().next().value;
              this.select(id);
              this.centerOn(id, Math.max(this.view.k, 0.9), true);
            } else {
              this.fit(true, this.searchHits);
            }
          } else if (ev.key === 'Escape') {
            search.value = '';
            this.searchHits = null;
            this.applyHighlight();
            search.blur();
          }
        });
      }

      document.addEventListener('keydown', (ev) => {
        const t = document.activeElement;
        if (t && /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)) return;
        if (ev.ctrlKey || ev.metaKey || ev.altKey) return;
        if (ev.key === 'f') this.fit(true);
        else if (ev.key === '+' || ev.key === '=') this.zoomAt(1.25, center().x, center().y);
        else if (ev.key === '-') this.zoomAt(0.8, center().x, center().y);
        else if (ev.key === 'Escape') this.select(null);
      });
    }

    exportSvg() {
      const b = this.bounds();
      const pad = 32;
      const out = document.createElementNS(SVG_NS, 'svg');
      out.setAttribute('xmlns', SVG_NS);
      out.setAttribute('width', Math.ceil(b.w + pad * 2));
      out.setAttribute('height', Math.ceil(b.h + pad * 2));
      out.setAttribute('viewBox', `${b.x - pad} ${b.y - pad} ${b.w + pad * 2} ${b.h + pad * 2}`);
      svg('rect', { x: b.x - pad, y: b.y - pad, width: b.w + pad * 2, height: b.h + pad * 2, fill: C.canvas }, out);
      const edges = this.edgeLayer.cloneNode(true);
      const nodes = this.nodeLayer.cloneNode(true);
      for (const layer of [edges, nodes]) {
        layer.querySelectorAll('.erd-dim').forEach((el) => el.classList.remove('erd-dim'));
        layer.querySelectorAll('.erd-edge-hit, .erd-open, title').forEach((el) => el.remove());
        out.appendChild(layer);
      }
      const text = new XMLSerializer().serializeToString(out);
      const url = URL.createObjectURL(new Blob([text], { type: 'image/svg+xml' }));
      const a = document.createElement('a');
      a.href = url;
      a.download = 'schema-erd.svg';
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    }
  }

  function boot() {
    document.querySelectorAll('[data-erd-source]').forEach((root) => {
      if (root._erd) return;
      root._erd = new ErdView(root);
      root._erd.init();
    });
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot);
  else boot();
})();
