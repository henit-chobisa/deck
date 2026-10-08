// A working copy of the stampede deck from the recording: the same title, the
// same narration and points, the same code, map and page. Everything runs in
// the browser; the agent's answers are written in advance.
(function () {
  'use strict'

  const PATH = 'services/catalog/src/pricing/pricing.service.ts'
  const SRC = [
    'import { CACHE_MANAGER } from "@nestjs/cache-manager";',
    'import { Inject, Injectable, Logger } from "@nestjs/common";',
    'import { InjectRepository } from "@nestjs/typeorm";',
    'import type { Cache } from "cache-manager";',
    'import { Repository } from "typeorm";',
    '',
    'import { PriceEntity } from "./price.entity";',
    'import type { Price } from "./pricing.types";',
    '',
    'const ONE_HOUR = 60 * 60 * 1000;',
    '',
    '@Injectable()',
    'export class PricingService {',
    '  private readonly log = new Logger(PricingService.name);',
    '',
    '  constructor(',
    '    @Inject(CACHE_MANAGER) private readonly cache: Cache,',
    '    @InjectRepository(PriceEntity)',
    '    private readonly prices: Repository<PriceEntity>,',
    '  ) {}',
    '',
    '  /**',
    '   * The price on every product page and in every cart.',
    '   * About 2,000 calls a second, nearly all of them served from Redis.',
    '   */',
    '  async priceFor(sku: string, region: string): Promise<Price> {',
    '    const key = `price:${region}:${sku}`;',
    '',
    '    const cached = await this.cache.get<Price>(key);',
    '    if (cached) return cached;',
    '',
    '    // Miss: compute it from the catalog, with promotions and tax.',
    '    const price = await this.prices.query(',
    '      `SELECT * FROM effective_price($1, $2)`,',
    '      [sku, region],',
    '    );',
    '',
    '    await this.cache.set(key, price, ONE_HOUR);',
    '    return price;',
    '  }',
    '}',
  ]

  // Group 1 shows the file as it is; group 2 shows line 38 replaced.
  const ROWS = [
    SRC.map((text, i) => ({ n: i + 1, text, kind: '' })),
    [
      ...SRC.slice(0, 37).map((text, i) => ({ n: i + 1, text, kind: '' })),
      { n: 38, text: SRC[37], kind: 'del' },
      { n: 38, text: '    // Spread expiry over ±10 minutes so keys never expire together.', kind: 'add' },
      { n: 39, text: '    await this.cache.set(key, price, ONE_HOUR + jitter(10 * 60 * 1000));', kind: 'add' },
      ...SRC.slice(38).map((text, i) => ({ n: i + 40, text, kind: '' })),
    ],
  ]

  const nm = (name) => '<span class="dk-nm">' + name + '</span>'
  const TITLE = 'Every hour, on the hour, checkout p99 jumps to 4 seconds'
  const GROUPS = [
    {
      ref: [26, 40], refLabel: PATH + ':26-40', note: 'the price on every page',
      says: [
        { html: 'Most of the hour, ' + nm('pricing') + ' answers from Redis in two milliseconds.', page: 'calm', lines: [29, 30], block: 'redis', talk: 'code' },
        { html: 'At 10:00:00 every price key expires at once, so every request misses and runs the 40 ms query on Postgres.', page: 'expire', lines: [33, 36], block: 'pg', talk: 'code' },
        { html: 'Forty connections fill in a blink and the rest queue, so a price that took two milliseconds now takes four seconds.', page: 'pile', lines: [26, 26], block: 'api', talk: 'page' },
        { html: 'It clears when the cache refills, and comes back at 11:00, because every key was written with the same one-hour TTL.', page: 'refill', lines: [38, 38], block: 'redis', talk: 'code' },
        { html: 'Press <b>on the hour</b> in ' + nm('map') + ' to watch a request take that path.', talk: 'map' },
      ],
    },
    {
      ref: [38, 39], refLabel: PATH + ':38', note: 'ttl, spread out',
      says: [
        { html: 'Spread the expiry, and ' + nm('requests') + ' stays quiet.', page: 'fix', talk: 'page' },
        { html: 'In ' + nm('pricing') + ' each key lives an hour, give or take ten minutes, so they stop expiring together.', page: 'fix', lines: [38, 39], block: 'redis', talk: 'code' },
        { html: 'On the ' + nm('map') + ', Postgres sees one miss at a time instead of a wall.', page: 'fix', block: 'pg', talk: 'map' },
        { html: '<b>Ship the jitter, or add a single-flight lock on a miss as well?</b>', page: 'fix' },
      ],
    },
  ]

  const ANSWERS = [
    {
      q: 'Why does it come back at 11:00?',
      a: 'Because the refill writes every key again with the same TTL. Line 38 sets ONE_HOUR, and line 10 makes that exactly an hour, so the keys the 10:00 stampede wrote all expire together at 11:00, and it starts over.',
      go: { g: 0, s: 3 },
    },
    {
      q: 'How many are waiting at 10:00:01?',
      a: 'All forty connections are busy and every other request queues behind them. The counter at the top of requests shows the pool full and the queue growing, which is the four seconds you see at the p99.',
      go: { g: 0, s: 2 },
    },
    {
      q: 'Why not a single-flight lock?',
      a: 'A lock stops one key being computed a hundred times, but every key still expires at 10:00:00, so Postgres still gets thousands of different misses in the same second. Jitter spreads those out. The lock is worth adding on top if a few SKUs are very hot, which is the question the second group ends on.',
      go: { g: 1, s: 0 },
    },
  ]
  const UNKNOWN = 'This deck is running in your browser with three answers written in advance, so I cannot answer that one. In deck, your own agent answers whatever you ask, right here, and can bring a file or draw what it means.'

  // ---------------------------------------------------------------- syntax
  const KW = new Set(['import', 'from', 'type', 'const', 'await', 'async', 'return', 'if', 'export', 'class', 'private', 'readonly', 'new', 'this'])
  const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]))
  function paint(line) {
    const t = line.trimStart()
    if (t.startsWith('//') || t.startsWith('/**') || t.startsWith('*')) return '<span class="c">' + esc(line) + '</span>'
    const re = /(`[^`]*`|"[^"]*"|'[^']*')|(@\w+)|(\b\d[\d_]*\b)|([A-Za-z_$][\w$]*)(?=\s*[(<])|([A-Za-z_$][\w$]*)|(\s+|.)/g
    let out = '', m
    while ((m = re.exec(line))) {
      if (m[1]) out += '<span class="s">' + esc(m[1]) + '</span>'
      else if (m[2]) out += '<span class="d">' + esc(m[2]) + '</span>'
      else if (m[3]) out += '<span class="nu">' + esc(m[3]) + '</span>'
      else if (m[4]) out += KW.has(m[4]) ? '<span class="k">' + m[4] + '</span>' : /^[A-Z]/.test(m[4]) ? '<span class="ty">' + m[4] + '</span>' : '<span class="f">' + m[4] + '</span>'
      else if (m[5]) out += KW.has(m[5]) ? '<span class="k">' + m[5] + '</span>' : /^[A-Z][a-z]/.test(m[5]) ? '<span class="ty">' + m[5] + '</span>' : esc(m[5])
      else out += esc(m[6])
    }
    return out
  }

  // ------------------------------------------------------------------- map
  const SVGNS = 'http://www.w3.org/2000/svg'
  const cyl = (x, y, w, h) => 'M' + x + ' ' + (y + 8) + ' a' + w / 2 + ' 8 0 0 1 ' + w + ' 0 v' + (h - 16) + ' a' + w / 2 + ' 8 0 0 1 ' + (-w) + ' 0 Z M' + x + ' ' + (y + 8) + ' a' + w / 2 + ' 8 0 0 0 ' + w + ' 0'
  const MAP = '<svg viewBox="104 8 426 330" preserveAspectRatio="xMidYMid meet" aria-hidden="true">' +
    '<rect class="mp-cluster" x="112" y="112" width="410" height="236" rx="10"></rect><text class="mp-cluster-t" x="128" y="134">catalog</text>' +
    '<g class="mp-edge" data-edge="page-api"><path d="M232 84 V150"></path><text x="242" y="122">GET /price</text></g>' +
    '<g class="mp-edge" data-edge="api-redis"><path d="M232 210 V262"></path><text x="242" y="242">get</text></g>' +
    '<g class="mp-edge" data-edge="api-pg"><path d="M322 180 H422 V262"></path><text x="336" y="171">on a miss</text></g>' +
    '<g class="mp-node" data-node="page"><ellipse cx="232" cy="50" rx="92" ry="34"></ellipse><text x="232" y="47">product page</text><text class="sub" x="232" y="65">2,000 a second</text></g>' +
    '<g class="mp-node accent" data-node="api"><rect x="142" y="150" width="180" height="60" rx="6"></rect><text x="232" y="176">catalog-api</text><text class="sub" x="232" y="195">PricingService</text></g>' +
    '<g class="mp-node" data-node="redis"><path d="' + cyl(142, 262, 180, 66) + '"></path><text x="232" y="300">redis</text><text class="sub" x="232" y="317">price:* · ttl 1 h</text></g>' +
    '<g class="mp-node" data-node="pg"><path d="' + cyl(332, 262, 180, 66) + '"></path><text x="422" y="300">postgres</text><text class="sub" x="422" y="317">effective_price() · 40 ms</text></g>' +
    '<circle class="mp-spark" r="5" cx="-20" cy="-20"></circle></svg>'
  const FLOWS = {
    'most requests': { route: [[232, 84], [232, 150], [232, 210], [232, 262]], nodes: ['page', 'api', 'redis'], edges: ['page-api', 'api-redis'] },
    'on the hour': { route: [[232, 84], [232, 150], [322, 180], [422, 180], [422, 262]], nodes: ['page', 'api', 'pg'], edges: ['page-api', 'api-pg'] },
  }

  // --------------------------------------------------------------- the window
  function build(dk) {
    dk.innerHTML =
      '<div class="dk-band">' +
        '<div class="dk-say">' +
          '<div class="dk-title"></div>' +
          '<div class="dk-prose"></div>' +
          '<div class="dk-track">' +
            '<button type="button" class="dk-play" aria-label="Walk the group">▶</button>' +
            '<button type="button" class="dk-again-top" aria-label="Start the group again">↺</button>' +
            '<div class="dk-line"><div class="dk-fill"></div><div class="dk-knob"></div></div>' +
          '</div>' +
        '</div>' +
        '<div class="dk-keys"><p>KEYS</p>' +
          [['n', 'next'], ['p', 'previous'], ['w', 'walk'], ['c', 'comment'], ['t', 'turn'], ['⟲', 'clear'], ['z', 'lights'], ['h', 'hide'], ['s', 'submit'], ['q', 'close']]
            .map(([k, l]) => '<button type="button" data-key="' + (k === '⟲' ? 'clear' : k) + '"><kbd>' + k + '</kbd>' + l + '</button>').join('') +
        '</div>' +
      '</div>' +
      '<div class="dk-room">' +
        '<div class="dk-panes">' +
          '<section class="dk-pane code" data-pane="code"><div class="dk-head"><span class="nm">pricing</span><span class="path"></span></div><div class="dk-note"></div><div class="dk-code" data-lenis-prevent></div></section>' +
          '<section class="dk-pane map" data-pane="map"><div class="dk-head"><span class="nm">map</span></div><div class="dk-note">a price, on the product page</div><div class="dk-map">' + MAP + '<div class="dk-flows"><button type="button" data-flow="most requests">most requests</button><button type="button" data-flow="on the hour">on the hour</button></div></div></section>' +
          '<section class="dk-pane page" data-pane="page"><div class="dk-head"><span class="nm">requests</span></div><div class="dk-note">GET /price around 10:00:00</div><button type="button" class="dk-again">again ↺</button><div class="dk-page"><svg></svg></div></section>' +
        '</div>' +
        '<aside class="dk-rail">' +
          '<button type="button" class="dk-rail-head" aria-expanded="false"><span class="chev">›</span><span class="lab">CHAT</span><span class="sub"></span><span class="dot"></span></button>' +
          '<div class="dk-chat" data-lenis-prevent><p class="dk-empty">The floor is yours</p></div>' +
          '<div class="dk-compose idle"><div class="on"></div><textarea aria-label="Your comment" placeholder="Write a comment · c"></textarea>' +
            '<div class="row"><span>esc discard</span><button type="button" class="later">Add to review<span class="hint">⇧⌘↩</span></button><button type="button" class="now">Ask now<span class="hint">⌘↩</span></button></div></div>' +
        '</aside>' +
      '</div>' +
      '<div class="dk-foot"><span class="grp"></span><span class="msg" aria-live="polite"></span><span class="r"><span class="cnt">0 comments</span><span>d-1791214494-5575</span><span>deck 0.1.5</span></span></div>'
  }

  function start() {
    const dk = document.getElementById('dk')
    if (!dk) return
    build(dk)
    const $ = (s) => dk.querySelector(s)
    const prose = $('.dk-prose'), code = $('.dk-code'), fill = $('.dk-fill'), knob = $('.dk-knob'), line = $('.dk-line')
    const mapBox = $('.dk-map'), spark = $('.mp-spark'), rail = $('.dk-rail'), chat = $('.dk-chat')
    const compose = $('.dk-compose'), area = compose.querySelector('textarea'), foot = $('.dk-foot .msg')
    $('.dk-title').textContent = TITLE
    const page = window.deckRequests ? window.deckRequests.mount($('.dk-page svg')) : { setPoint() {} }

    const st = { g: 0, s: 0, sel: null, lights: true, comments: [], walking: 0, flowing: 0, footTimer: 0, composing: false }

    function say(msg) {
      foot.textContent = msg
      clearTimeout(st.footTimer)
      st.footTimer = setTimeout(() => { foot.textContent = '' }, 4200)
    }

    // ---- narration and track
    function renderProse() {
      const G = GROUPS[st.g]
      prose.innerHTML = G.says.map((x, i) => '<span class="dk-sn" role="button" tabindex="-1" data-s="' + i + '">' + x.html + '</span>').join(' ')
      line.querySelectorAll('.dk-tick').forEach((t) => t.remove())
      G.says.forEach((_, i) => {
        const b = document.createElement('button')
        b.type = 'button'; b.className = 'dk-tick'; b.dataset.s = i
        b.setAttribute('aria-label', 'Sentence ' + (i + 1))
        b.style.left = (i / G.says.length * 100) + '%'
        line.appendChild(b)
      })
      $('.dk-foot .grp').textContent = 'group ' + (st.g + 1) + '/' + GROUPS.length
      $('.dk-code').scrollTop = 0
    }

    function renderCode() {
      const G = GROUPS[st.g], S = G.says[st.s] || {}
      const rows = ROWS[st.g]
      const noted = new Set(st.comments.filter((c) => c.g === st.g && c.a).flatMap((c) => range(c.a, c.b)))
      code.innerHTML = rows.map((r, i) => {
        const inRef = r.n >= G.ref[0] && r.n <= G.ref[1] && (st.g === 0 || r.kind)
        const pt = st.lights && S.lines && r.kind !== 'del' && r.n >= S.lines[0] && r.n <= S.lines[1]
        const sel = st.sel && i >= Math.min(st.sel.a, st.sel.b) && i <= Math.max(st.sel.a, st.sel.b)
        const cls = ['dk-ln', r.kind, st.lights && inRef ? 'lit' : '', pt ? 'pt' : '', sel ? 'sel' : '', noted.has(i) ? 'noted' : ''].filter(Boolean).join(' ')
        const sign = r.kind === 'del' ? '-' : r.kind === 'add' ? '+' : ''
        return '<div class="' + cls + '" data-i="' + i + '"><span class="g"></span><span class="sign">' + sign + '</span><span class="n">' + r.n + '</span><span class="t">' + (paint(r.text) || ' ') + '</span></div>'
      }).join('')
      $('.dk-pane.code .path').textContent = G.refLabel
      $('.dk-pane.code .dk-note').textContent = G.note
    }

    const range = (a, b) => { const out = []; for (let i = Math.min(a, b); i <= Math.max(a, b); i++) out.push(i); return out }

    function apply(scroll) {
      const G = GROUPS[st.g], S = G.says[st.s] || {}
      prose.querySelectorAll('.dk-sn').forEach((e, i) => e.classList.toggle('now', i === st.s))
      const n = G.says.length
      const at = (st.s + 1) / n * 100
      fill.style.width = at + '%'
      knob.style.left = (st.s / n * 100) + '%'
      line.querySelectorAll('.dk-tick').forEach((t, i) => t.classList.toggle('past', i <= st.s))
      renderCode()
      if (S.page) page.setPoint(S.page)
      lightMap(st.lights && S.block ? [S.block] : [], [])
      dk.querySelectorAll('.dk-pane').forEach((p) => p.classList.toggle('talk', st.lights && p.dataset.pane === S.talk))
      if (scroll) {
        const first = code.querySelector('.dk-ln.pt') || code.querySelector('.dk-ln.lit')
        if (first) code.scrollTo({ top: Math.max(0, first.offsetTop - code.clientHeight / 3), behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' })
      }
    }

    function lightMap(nodes, edges) {
      mapBox.classList.toggle('focus', nodes.length > 0)
      mapBox.querySelectorAll('.mp-node').forEach((e) => e.classList.toggle('lit', nodes.includes(e.dataset.node)))
      mapBox.querySelectorAll('.mp-edge').forEach((e) => e.classList.toggle('lit', edges.includes(e.dataset.edge)))
    }

    function go(g, s) {
      const changed = g !== st.g
      st.g = Math.max(0, Math.min(GROUPS.length - 1, g))
      st.s = Math.max(0, Math.min(GROUPS[st.g].says.length - 1, s))
      if (changed) { st.sel = null; renderProse() }
      apply(true)
    }

    // ---- walking without a voice: the track moves by itself
    function walk(on) {
      clearInterval(st.walking); st.walking = 0
      $('.dk-play').textContent = on ? '❚❚' : '▶'
      $('.dk-play').setAttribute('aria-label', on ? 'Hold' : 'Walk the group')
      if (!on) return
      st.walking = setInterval(() => {
        const n = GROUPS[st.g].says.length
        if (st.s >= n - 1) { walk(false); return }
        go(st.g, st.s + 1)
      }, 3400)
    }

    // ---- flows in the map
    function flow(name) {
      const F = FLOWS[name]
      if (!F) return
      cancelAnimationFrame(st.flowing)
      mapBox.querySelectorAll('.dk-flows button').forEach((b) => b.classList.toggle('on', b.dataset.flow === name))
      const pts = F.route, segs = []
      let total = 0
      for (let i = 1; i < pts.length; i++) { const d = Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]); segs.push(d); total += d }
      const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches
      const dur = reduce ? 0 : 1700
      let t0 = null
      const step = (now) => {
        if (t0 === null) t0 = now
        const k = dur ? Math.min(1, (now - t0) / dur) : 1
        let d = k * total, i = 0
        while (i < segs.length - 1 && d > segs[i]) { d -= segs[i]; i++ }
        const r = segs[i] ? Math.min(1, d / segs[i]) : 1
        const x = pts[i][0] + (pts[i + 1][0] - pts[i][0]) * r, y = pts[i][1] + (pts[i + 1][1] - pts[i][1]) * r
        spark.setAttribute('cx', x); spark.setAttribute('cy', y)
        const reached = Math.min(F.nodes.length, 1 + Math.floor(k * (F.nodes.length - 1) + 0.0001))
        lightMap(F.nodes.slice(0, Math.max(1, reached)), F.edges.slice(0, Math.max(0, reached - 1)))
        if (k < 1) st.flowing = requestAnimationFrame(step)
        else {
          lightMap(F.nodes, F.edges)
          setTimeout(() => { spark.setAttribute('cx', -20); spark.setAttribute('cy', -20) }, 600)
        }
      }
      st.flowing = requestAnimationFrame(step)
      dk.querySelectorAll('.dk-pane').forEach((p) => p.classList.toggle('talk', p.dataset.pane === 'map'))
    }

    // ---- the rail: comments and the conversation
    function openRail(open) {
      rail.classList.toggle('open', open)
      $('.dk-rail-head').setAttribute('aria-expanded', String(open))
      if (open) rail.classList.remove('unread')
    }

    function where() {
      if (!st.sel) return 'group ' + (st.g + 1)
      const rows = ROWS[st.g]
      const a = rows[Math.min(st.sel.a, st.sel.b)].n, b = rows[Math.max(st.sel.a, st.sel.b)].n
      return 'pricing.service.ts:' + (a === b ? a : a + '-' + b)
    }

    function compose_(open) {
      st.composing = open
      compose.classList.toggle('idle', !open)
      if (open) {
        openRail(true)
        compose.querySelector('.on').textContent = 'comment on ' + where()
        area.placeholder = 'Say what you would change…'
        setTimeout(() => area.focus({ preventScroll: true }), 30)
      } else {
        area.value = ''
        area.placeholder = 'Write a comment · c'
        area.blur()
        dk.focus({ preventScroll: true })
      }
    }

    function post(kind, who, text, extra) {
      const empty = chat.querySelector('.dk-empty')
      if (empty) empty.remove()
      const m = document.createElement('div')
      m.className = 'dk-msg ' + kind
      const dot = document.createElement('i')
      const w = document.createElement('div'); w.className = 'who'; w.textContent = who
      m.append(dot, w)
      if (text !== null) { const p = document.createElement('p'); p.textContent = text; m.append(p) }
      if (extra) m.append(extra)
      chat.append(m)
      chat.scrollTop = chat.scrollHeight
      if (!rail.classList.contains('open')) rail.classList.add('unread')
      return m
    }

    function stamp() {
      const d = new Date()
      return d.getHours() + ':' + String(d.getMinutes()).padStart(2, '0')
    }

    function reply(text, then) {
      const pulse = document.createElement('div'); pulse.className = 'dk-pulse'
      const waiting = post('agent', 'agent · g' + (st.g + 1), null, pulse)
      setTimeout(() => {
        waiting.remove()
        post('agent', 'agent · g' + (st.g + 1) + ' · ' + stamp(), text)
        if (then) then()
      }, 1300)
    }

    function updateCount() {
      const n = st.comments.length
      $('.dk-foot .cnt').textContent = n + (n === 1 ? ' comment' : ' comments')
      $('.dk-rail-head .sub').textContent = n ? '· ' + n + ' to send' : ''
    }

    function finish(now) {
      const text = area.value.trim()
      if (!text) { say('Write something first, or press esc.'); return }
      const at = where()
      if (now) {
        post('you', 'you · ' + at + ' · ' + stamp(), text)
        compose_(false)
        reply(UNKNOWN)
      } else {
        st.comments.push({ g: st.g, a: st.sel ? Math.min(st.sel.a, st.sel.b) : null, b: st.sel ? Math.max(st.sel.a, st.sel.b) : null, at, text })
        post('you', 'you · ' + at + ' · in review', text)
        compose_(false)
        updateCount()
        renderCode()
        say('Kept for the review. Press s to submit.')
      }
    }

    function ask(i) {
      const A = ANSWERS[i]
      openRail(true)
      post('you', 'you · group ' + (st.g + 1) + ' · ' + stamp(), A.q)
      reply(A.a, () => { walk(false); go(A.go.g, A.go.s) })
    }

    function submit() {
      if (!st.comments.length) { say('Nothing to send yet. Click a line, press c, and add it to the review.'); return }
      const n = st.comments.length
      say('Review sent to your agent · ' + n + (n === 1 ? ' comment' : ' comments'))
      const list = document.getElementById('loop-comments'), cnt = document.getElementById('loop-count')
      if (list && cnt) {
        list.textContent = ''
        st.comments.forEach((c, i) => {
          if (i) list.append(document.createElement('br'))
          const at = document.createElement('span'); at.className = 'hl'; at.textContent = c.at
          list.append(at, document.createTextNode('  "' + c.text + '"'))
        })
        cnt.textContent = n + (n === 1 ? ' comment' : ' comments')
      }
    }

    // ---- input
    const MESSAGES = {
      w: 'w reads the deck aloud once deck walk has set up a voice.',
      t: 't turns the panes a quarter in the window.',
      h: 'h puts the deck back on the bar, comments and all.',
      q: 'q closes without answering. Your agent hears that you closed it.',
      clear: 'The agent clears the lights when it finishes a point.',
    }
    function key(k) {
      if (k === 'n') { walk(false); go(st.g + 1, 0) }
      else if (k === 'p') { walk(false); go(st.g - 1, 0) }
      else if (k === 'c') compose_(true)
      else if (k === 's') submit()
      else if (k === 'z') { st.lights = !st.lights; apply(false); say(st.lights ? 'Lights on.' : 'Lights off.') }
      else if (MESSAGES[k]) say(MESSAGES[k])
    }

    dk.addEventListener('keydown', (e) => {
      if (e.target === area) {
        if (e.key === 'Escape') { e.preventDefault(); compose_(false) }
        else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); finish(!e.shiftKey) }
        return
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return
      if (e.key === 'Escape') { st.sel = null; renderCode(); return }
      const k = e.key.toLowerCase()
      if ('npcszwthq'.includes(k) && k.length === 1) { e.preventDefault(); key(k) }
    })

    area.addEventListener('focus', () => { if (!st.composing) compose_(true) })
    compose.querySelector('.later').addEventListener('click', () => finish(false))
    compose.querySelector('.now').addEventListener('click', () => finish(true))
    $('.dk-rail-head').addEventListener('click', () => openRail(!rail.classList.contains('open')))
    $('.dk-keys').addEventListener('click', (e) => { const b = e.target.closest('button'); if (b) key(b.dataset.key) })
    $('.dk-play').addEventListener('click', () => walk(!st.walking))
    $('.dk-again-top').addEventListener('click', () => { walk(false); go(st.g, 0) })
    $('.dk-again').addEventListener('click', () => { const S = GROUPS[st.g].says[st.s]; if (S && S.page) { page.setPoint('calm'); setTimeout(() => page.setPoint(S.page), 60) } })
    prose.addEventListener('click', (e) => { const sn = e.target.closest('.dk-sn'); if (sn) { walk(false); go(st.g, +sn.dataset.s) } })
    line.addEventListener('click', (e) => { const t = e.target.closest('.dk-tick'); if (t) { walk(false); go(st.g, +t.dataset.s) } })
    mapBox.querySelector('.dk-flows').addEventListener('click', (e) => { const b = e.target.closest('button'); if (b) flow(b.dataset.flow) })

    // select lines: press, drag, release; shift extends
    let dragging = false, touchedAt = 0
    code.addEventListener('mousedown', (e) => {
      const ln = e.target.closest('.dk-ln')
      if (!ln || e.button !== 0 || Date.now() - touchedAt < 800) return
      const i = +ln.dataset.i
      st.sel = e.shiftKey && st.sel ? { a: st.sel.a, b: i } : { a: i, b: i }
      dragging = true
      renderCode()
    })
    code.addEventListener('mouseover', (e) => {
      if (!dragging) return
      const ln = e.target.closest('.dk-ln')
      if (ln && st.sel && st.sel.b !== +ln.dataset.i) { st.sel.b = +ln.dataset.i; renderCode() }
    })
    window.addEventListener('mouseup', () => { if (dragging) { dragging = false; say('Selected ' + where().replace('pricing.service.ts:', 'lines ') + '. Press c to comment.') } })
    code.addEventListener('touchend', (e) => {
      const ln = e.target.closest('.dk-ln')
      if (!ln) return
      touchedAt = Date.now()
      st.sel = { a: +ln.dataset.i, b: +ln.dataset.i }
      renderCode()
      say('Selected ' + where().replace('pricing.service.ts:', 'line ') + '. Open CHAT below and write your comment.')
    }, { passive: true })

    document.querySelectorAll('[data-ask]').forEach((b) => b.addEventListener('click', () => ask(+b.dataset.ask)))

    renderProse()
    apply(true)
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start)
  else start()
})()
