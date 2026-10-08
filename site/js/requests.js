// The page from the stampede deck (requests.html), unchanged in what it draws,
// mounted into an <svg> here instead of a pane. deck hands a page its palette
// as --deck-* variables and its points as `deck:point` events; this takes the
// same names through mount() and setPoint().
(function () {
  'use strict'
  const NS = 'http://www.w3.org/2000/svg'
  const FROM = -2, TO = 6
  const skus = ['sku-1042', 'sku-2210', 'sku-0871', 'sku-5530', 'sku-1042', 'sku-7781',
    'sku-0034', 'sku-2210', 'sku-9902', 'sku-4417', 'sku-1042']
  const lanes = skus.map((sku, i) => ({ sku, start: FROM + 0.25 + i * 0.66 }))
  const REFILL = 4.4
  const at = { calm: -0.3, expire: 0.9, pile: 3.6, refill: 6, fix: 6 }

  function plan(i, fixed) {
    const s = lanes[i].start
    const hit = [[s, s + 0.06, 'hit']]
    if (fixed) return i === 4 ? [[s, s + 0.05, 'db']] : hit
    if (s < 0 || s >= REFILL) return hit
    const free = 0.6 + (i - 3) * 1.0
    return [[s, Math.max(s, free), 'wait'], [Math.max(s, free), Math.max(s, free) + 0.45, 'db']]
  }

  function mount(svg) {
    const uid = 'stripes' + Math.random().toString(36).slice(2, 8)
    svg.innerHTML = '<defs><pattern id="' + uid + '" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">' +
      '<rect width="8" height="8" style="fill: var(--deck-del); opacity: .22"></rect>' +
      '<rect width="3" height="8" style="fill: var(--deck-del); opacity: .85"></rect></pattern></defs>'
    const make = (tag, cls, text) => {
      const e = document.createElementNS(NS, tag)
      if (cls) e.setAttribute('class', cls)
      if (text) e.textContent = text
      return svg.appendChild(e)
    }
    const secs = [-2, 0, 2, 4, 6]
    const grid = secs.map(() => make('line', 'rq-grid'))
    const ticks = secs.map(s => make('text', 'rq-t', s < 0 ? '09:59:58' : '10:00:0' + s))
    const names = lanes.map(l => make('text', 'rq-t rq-lane', 'GET ' + l.sku))
    const bars = lanes.map(() => [0, 1].map(() => make('rect')))
    const ttl = make('line', 'rq-ttl'), ttlT = make('text', 'rq-t rq-ttlT', 'TTL expires')
    const count = make('text', 'rq-t rq-count')
    const keys = [['hit', 'redis'], ['wait', 'waiting for a connection'], ['db', 'postgres']]
      .map(([k, label]) => [make('rect', 'rq-' + k), make('text', 'rq-t rq-key', label)])
    keys[1][0].style.fill = 'url(#' + uid + ')'
    const head = make('line'); head.setAttribute('stroke', 'var(--deck-accent)')

    let W = 0, H = 0, shown = FROM, from = FROM, to = FROM, t0 = null, frame = 0, timer = 0, point = null

    function place(t) {
      if (!W || !H) return
      const fixed = point === 'fix'
      const L = 150, R = W - 20, T = 78, lane = Math.min(36, (H - T - 44) / lanes.length)
      const bar = Math.round(lane * 0.56), x = s => L + (R - L) * (s - FROM) / (TO - FROM)
      const bottom = T + lane * lanes.length
      grid.forEach((g, i) => { g.setAttribute('x1', x(secs[i])); g.setAttribute('x2', x(secs[i])); g.setAttribute('y1', T - 8); g.setAttribute('y2', bottom) })
      const every = (x(secs[1]) - x(secs[0])) < 110 ? 2 : 1
      ticks.forEach((e, i) => {
        const anchor = i === 0 ? 'start' : i === secs.length - 1 ? 'end' : 'middle'
        e.setAttribute('text-anchor', anchor); e.setAttribute('x', x(secs[i])); e.setAttribute('y', bottom + 20)
        e.style.opacity = i % every === 0 ? 1 : 0
      })
      let waiting = 0, inDb = 0
      lanes.forEach((l, i) => {
        const y = T + lane * i
        names[i].setAttribute('x', 12); names[i].setAttribute('y', y + bar / 2 + 4)
        names[i].style.opacity = t >= l.start ? 1 : 0.35
        const segs = plan(i, fixed)
        bars[i].forEach((r, j) => {
          const sg = segs[j]
          if (!sg || t < sg[0]) { r.setAttribute('width', 0); return }
          const end = Math.min(t, sg[1])
          r.setAttribute('x', x(sg[0])); r.setAttribute('y', y)
          r.setAttribute('width', Math.max(4, x(end) - x(sg[0]))); r.setAttribute('height', bar)
          r.setAttribute('rx', 3); r.setAttribute('class', 'rq-' + sg[2])
          r.style.fill = sg[2] === 'wait' ? 'url(#' + uid + ')' : ''
          if (t < sg[1] && sg[2] === 'wait') waiting++
          if (t < sg[1] && sg[2] === 'db') inDb++
        })
      })
      const shownTtl = !fixed && t >= 0
      ttl.setAttribute('x1', x(0)); ttl.setAttribute('x2', x(0)); ttl.setAttribute('y1', T - 14); ttl.setAttribute('y2', bottom)
      ttl.style.opacity = shownTtl ? 1 : 0
      ttlT.setAttribute('x', x(0) + 6); ttlT.setAttribute('y', T - 14); ttlT.style.opacity = shownTtl ? 1 : 0
      count.setAttribute('x', R); count.setAttribute('y', 20)
      count.textContent = fixed ? 'pool 1 / 40 · nobody waiting'
        : t < 0 ? 'pool 0 / 40' : waiting ? 'pool 40 / 40 · ' + (waiting * 180) + ' waiting' : 'pool ' + (inDb ? 40 : 0) + ' / 40'
      count.setAttribute('class', waiting ? 'rq-t rq-count bad' : 'rq-t rq-count')
      let kx = L
      keys.forEach(([r, label]) => {
        r.setAttribute('x', kx); r.setAttribute('y', 36); r.setAttribute('width', 16); r.setAttribute('height', 10); r.setAttribute('rx', 2)
        label.setAttribute('x', kx + 22); label.setAttribute('y', 45)
        kx += 30 + label.textContent.length * 7
      })
      head.setAttribute('x1', x(t)); head.setAttribute('x2', x(t)); head.setAttribute('y1', T - 10); head.setAttribute('y2', bottom + 4)
    }

    function run(now) {
      if (t0 === null) t0 = now
      const k = Math.min(1, (now - t0) / 1300), e = k * k * (3 - 2 * k)
      shown = from + (to - from) * e
      place(shown)
      frame = k < 1 ? requestAnimationFrame(run) : 0
    }

    function setPoint(name) {
      if (!(name in at)) return
      const was = point
      point = name
      if ((point === 'fix') !== (was === 'fix')) shown = FROM
      from = shown; to = at[point]; t0 = null
      cancelAnimationFrame(frame); clearTimeout(timer)
      if (matchMedia('(prefers-reduced-motion: reduce)').matches) { shown = to; place(to); return }
      frame = requestAnimationFrame(run)
      timer = setTimeout(() => { cancelAnimationFrame(frame); frame = 0; shown = to; place(to) }, 1500)
    }

    new ResizeObserver(() => { W = svg.clientWidth; H = svg.clientHeight; place(shown) }).observe(svg)
    return { setPoint }
  }

  window.deckRequests = { mount }
})()
