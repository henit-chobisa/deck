// /play/: a recorded deck, played full screen on a drawn desktop. The window is drawn the way deck draws it
// (crates/app: view.rs, pane.rs, prose.rs, and the HTML export's measurements in
// crates/render): the band with its number, titles and narration, the review and the
// conversation down the side, panes of whole files with their range lit, a proposed change
// as rows going and arriving, values on the lines, the strip, and the asks under the window.
// The narration is cut at its points; a click takes the sentence it lands in and lights its
// point. Lights rise in 150 ms when a hand raised them and go out in 420, in one easing.
;(function () {
  'use strict'
  const D = window.PLAY_DECK
  const $ = (id) => document.getElementById(id)
  const desk = $('desk'), dk = $('dk'), start = $('pi-start')
  if (!D || !desk || !dk || !start) return
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]))
  const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e }
  const timers = new Set()
  const later = (fn, ms) => { const t = setTimeout(() => { timers.delete(t); fn() }, ms); timers.add(t); return t }
  const hush = () => { timers.forEach(clearTimeout); timers.clear() }

  // ------------------------------------------------------------------ prose
  // A point is lines, names, or both: "9-11 refresh", "+5 ttl", "before flow".
  function readPoint(text) {
    const point = { lines: null, names: [] }
    for (const part of (text || '').trim().split(/\s+/).filter(Boolean)) {
      const m = /^(\+)?(\d+)(?:-(\d+))?$/.exec(part)
      if (m && !point.lines) point.lines = { after: !!m[1], from: +m[2], to: +(m[3] || m[2]) }
      else point.names.push(part)
    }
    return point
  }

  // Paragraphs of words. Every word knows its sentence and the piece of the narration it falls
  // in; a piece starts at each point. A word carries the space after it, so a lit sentence is
  // one band rather than a row of islands.
  function readSay(say, refNames) {
    const pieces = [null], paras = []
    let piece = 0, sentence = 0
    for (const raw of say.split(/\n\s*\n/)) {
      const segs = []
      const re = /\[point(?:\s+([^\]]*))?\]\s*|\[pause\]\s*|\[([A-Za-z][\w-]*)\]|\*\*([^*]+)\*\*|`([^`]+)`|([^[*`]+|[[*`])/g
      let m
      while ((m = re.exec(raw.trim()))) {
        if (m[0].startsWith('[point')) { pieces.push(readPoint(m[1])); piece = pieces.length - 1; continue }
        if (m[0].startsWith('[pause]')) continue
        if (m[2] !== undefined) { segs.push(refNames.includes(m[2]) ? { chip: m[2], piece } : { text: m[0], piece }); continue }
        if (m[3] !== undefined) segs.push({ text: m[3], b: true, piece })
        else if (m[4] !== undefined) segs.push({ text: m[4], code: true, piece })
        else segs.push({ text: m[5], piece })
      }
      const words = []
      for (const s of segs) {
        if (s.chip) { words.push({ chip: s.chip, text: s.chip, piece: s.piece }); continue }
        for (const p of s.text.match(/\S+\s*|\s+/g) || []) {
          if (!p.trim()) { if (words.length) words[words.length - 1].text += p; continue }
          words.push({ text: p, b: s.b, code: s.code, piece: s.piece })
        }
      }
      for (const w of words) { w.sentence = sentence; if (!w.chip && /[.!?]["”’)]?\s*$/.test(w.text)) sentence++ }
      const last = words[words.length - 1]
      if (last && (last.chip || !/[.!?]["”’)]?\s*$/.test(last.text))) sentence++
      paras.push(words)
    }
    return { paras, pieces }
  }

  // ------------------------------------------------------------------- code
  const KW = new Set(['import', 'from', 'type', 'const', 'let', 'await', 'async', 'return', 'if', 'export', 'function', 'throw', 'new', 'string'])
  function paint(line) {
    const re = /(\/\/.*$)|(`[^`]*`|"[^"]*"|'[^']*')|(\b\d[\d_.]*\b)|([A-Za-z_$][\w$]*)(?=\s*\()|([A-Za-z_$][\w$]*)|(\s+|.)/g
    let out = '', m
    while ((m = re.exec(line))) {
      if (m[1]) out += '<span class="c">' + esc(m[1]) + '</span>'
      else if (m[2]) out += '<span class="s">' + esc(m[2]) + '</span>'
      else if (m[3]) out += '<span class="nu">' + esc(m[3]) + '</span>'
      else if (m[4]) out += KW.has(m[4]) ? '<span class="k">' + m[4] + '</span>' : /^[A-Z]/.test(m[4]) ? '<span class="ty">' + m[4] + '</span>' : '<span class="f">' + m[4] + '</span>'
      else if (m[5]) out += KW.has(m[5]) ? '<span class="k">' + m[5] + '</span>' : /^[A-Z][a-z]/.test(m[5]) ? '<span class="ty">' + m[5] + '</span>' : esc(m[5])
      else out += esc(m[6])
    }
    return out
  }

  // Where a value hangs on its line: right after the first time its name is there as a whole
  // word, or at the line's end (deck_render::after_name).
  function afterName(text, name) {
    const word = (ch) => !!ch && /[\w$]/.test(ch)
    let from = 0
    for (;;) {
      const at = text.indexOf(name, from)
      if (at < 0) return text.length
      const end = at + name.length
      if ((!word(name[0]) || !word(text[at - 1])) && (!word(name[name.length - 1]) || !word(text[end]))) return end
      from = at + Math.max(1, name.length)
    }
  }

  // The rows a code ref draws: the whole file, its range lit; or, for a proposed change, every
  // old line going and every new line arriving, both numbered, as the window draws --after.
  function rowsOf(ref) {
    const src = D.files[ref.file], [a, b] = ref.range, rows = []
    const plain = (i, ctx) => ({ n: i + 1, text: src[i], kind: ctx ? 'ctx' : '', lit: false })
    if (ref.after) {
      for (let i = 0; i < a - 1; i++) rows.push(plain(i, true))
      for (let i = a - 1; i < b; i++) rows.push({ n: i + 1, text: src[i], kind: 'gone' })
      ref.after.forEach((text, j) => rows.push({ n: a + j, text, kind: 'fresh' }))
      const shift = ref.after.length - (b - a + 1)
      for (let i = b; i < src.length; i++) rows.push({ n: i + 1 + shift, text: src[i], kind: 'ctx' })
    } else {
      for (let i = 0; i < src.length; i++) rows.push(i >= a - 1 && i < b ? { n: i + 1, text: src[i], kind: '', lit: true } : plain(i, true))
    }
    return rows
  }

  // Which rows a point's lines name. `+` reads the arriving side of a change; without it, the
  // going side when there is one.
  function rowsPointed(rows, lines) {
    if (!lines) return []
    const hasGone = rows.some((r) => r.kind === 'gone')
    const side = lines.after ? 'fresh' : hasGone ? 'gone' : null
    const out = []
    rows.forEach((r, i) => {
      const onSide = side ? r.kind === side : r.kind !== 'gone'
      if (onSide && r.n >= lines.from && r.n <= lines.to) out.push(i)
    })
    return out
  }

  function rowHtml(r, i, values) {
    let text = r.text || ''
    let body = ''
    const inline = (values || []).filter((v) => v.line === r.n && !v.keys && r.kind !== 'gone')
      .map((v) => [afterName(text, v.at), v]).sort((x, y) => x[0] - y[0])
    let at = 0
    for (const [place, v] of inline) {
      body += paint(text.slice(at, place)) + '<span class="val' + (v.lit ? ' lit' : '') + '">' + esc(v.is) + '</span>'
      at = place
    }
    body += paint(text.slice(at))
    const sign = r.kind === 'gone' ? '−' : r.kind === 'fresh' ? '+' : ''
    let html = '<div class="dk-row' + (r.kind ? ' ' + r.kind : '') + (r.lit ? ' lit' : '') + '" data-ln="' + i + '"><span class="bar">' + sign + '</span><span class="num">' + r.n + '</span><span class="txt">' + (body || ' ') + '</span></div>'
    for (const v of (values || []).filter((v) => v.line === r.n && v.keys)) {
      const lead = text.length - text.trimStart().length
      html += '<button type="button" class="dk-row dk-prev" aria-expanded="true"><span class="bar"></span><span class="num"></span><span class="txt">' + ' '.repeat(lead) + '<span class="pv">↳ ' + esc(v.at) + ' ▸ ' + esc(v.is) + '</span></span></button>'
      html += '<div class="dk-keysv">' + v.keys.map((k) => '<div class="dk-row kv' + (k.why ? ' why' : '') + '"><span class="bar"></span><span class="num"></span><span class="txt">' + ' '.repeat(lead + 4) + '<span class="kk">' + esc(k.key) + '</span> <span class="val">' + esc(k.is) + '</span>' + (k.why ? ' <span class="kw">' + esc(k.why) + '</span>' : '') + '</span></div>').join('') + '</div>'
    }
    return html
  }

  // ----------------------------------------------------------------- window
  const KEYS = [['n', 'next'], ['p', 'previous'], ['c', 'comment']]
  const DOCK = [['n', 'next group'], ['p', 'previous group'], ['c', 'comment on the lines you hold'], ['esc', 'clear the lights'], ['s', 'send the review'], ['t', 'turn the panes'], ['h', 'hide, back to the bar'], ['q', 'close without answering']]
  dk.innerHTML =
    '<div class="dk-band"><div class="dk-head"><div class="dk-num" aria-hidden="true"></div><div class="dk-titles"><div class="dk-deck">' + esc(D.title) + '</div>' +
      '<div class="dk-tr"><h2 class="dk-title"></h2><span class="dk-conf"><span class="bars" aria-hidden="true"><i></i><i></i><i></i></span><span class="lv"></span></span></div></div></div>' +
      '<div class="dk-prose"></div></div>' +
    '<aside class="dk-side"><div class="dk-rev"><div class="dk-rh"><span>YOUR REVIEW</span><span class="dk-seen"></span></div><ol class="dk-toc"></ol></div>' +
      '<div class="dk-rail"><div class="dk-railh"><span class="dk-fold" aria-hidden="true">›</span><b>CHAT</b><span class="dk-send"></span></div>' +
        '<div class="dk-chat"><p class="dk-empty">The floor is yours</p></div>' +
        '<div class="dk-compose idle"><div class="on"></div><textarea aria-label="Your comment" rows="1" placeholder="Write a comment · c"></textarea>' +
          '<div class="row"><span>esc discard</span><button type="button" class="later">Add to review<span class="hint">⇧⌘↩</span></button><button type="button" class="now">Ask now<span class="hint">⌘↩</span></button></div></div></div></aside>' +
    '<div class="dk-panes"></div>' +
    '<div class="dk-strip"><span class="grp"></span><span class="msg" aria-live="polite">your agent can make incorrect claims</span><span class="fill"></span>' +
      KEYS.map(([k, l]) => '<button type="button" class="key" data-key="' + k + '"><kbd>' + k + '</kbd>' + l + '</button>').join('') +
      '<span class="sep" aria-hidden="true"></span><button type="button" class="key" data-key="?" aria-expanded="false"><kbd>?</kbd>keys</button>' +
      '<button type="button" class="dk-exp" data-key="export"><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 8V1.5M3.5 4L6 1.5 8.5 4M1.5 7.5v3h9v-3"/></svg>export</button><span class="ver">deck 0.1.7-rc.2</span></div>' +
    '<div class="dk-dock" hidden>' + DOCK.map(([k, l]) => '<span><kbd>' + k + '</kbd>' + l + '</span>').join('') + '</div>'

  const q = (s) => dk.querySelector(s)
  const prose = q('.dk-prose'), panes = q('.dk-panes'), chat = q('.dk-chat'), toc = q('.dk-toc')
  const compose = q('.dk-compose'), area = compose.querySelector('textarea'), msg = q('.dk-strip .msg'), dock = q('.dk-dock')
  const asks = $('dk-asks')
  const ROW = 19
  const LV = { high: 'High', medium: 'Medium', low: 'Low' }
  const st = { g: -1, said: null, point: null, held: null, heldPane: -1, review: [], composing: false, opened: 0, seen: new Set(), asked: new Set(), brought: false, msgTimer: 0 }
  let G = null

  function say(text) {
    msg.textContent = text; msg.classList.add('now')
    clearTimeout(st.msgTimer)
    st.msgTimer = setTimeout(() => { msg.textContent = 'your agent can make incorrect claims'; msg.classList.remove('now') }, 4200)
  }

  function pathOf(ref) { return ref.file + ':' + (ref.range[0] === ref.range[1] ? ref.range[0] : ref.range[0] + '-' + ref.range[1]) }

  function paneOf(ref) {
    const pane = el('section', 'dk-pane ' + ref.kind + (ref.brought ? ' brought' : ''))
    pane.setAttribute('aria-label', (ref.name || '') + ': ' + (ref.note || ''))
    const head = el('div', 'dk-ph')
    head.append(el('b', null, ref.name || ''))
    if (ref.kind === 'code') head.append(el('span', 'path', pathOf(ref)))
    const chev = el('span', 'chev', '‹'); chev.setAttribute('aria-hidden', 'true'); head.append(chev)
    pane.append(head, el('div', 'dk-note', ref.note || ''))
    const P = { ref, el: pane }
    if (ref.kind === 'code') {
      const code = el('div', 'dk-code'), box = el('div', 'dk-rows')
      P.rows = rowsOf(ref)
      box.innerHTML = P.rows.map((r, i) => rowHtml(r, i, ref.values)).join('')
      code.append(box); pane.append(code)
      P.code = code; P.rowEls = [...box.querySelectorAll('[data-ln]')]
      box.querySelectorAll('.dk-prev').forEach((b) => b.addEventListener('click', () => {
        const open = b.getAttribute('aria-expanded') !== 'true'
        b.setAttribute('aria-expanded', String(open)); b.nextElementSibling.hidden = !open
      }))
    } else if (ref.kind === 'flow') {
      P.flow = flowOf(pane, head)
    } else {
      const again = el('button', 'dk-again', 'again ↻'); again.type = 'button'
      const holder = el('div', 'dk-page')
      pane.append(again, holder)
      P.page = pageOf(ref.name, holder)
      again.addEventListener('click', () => { P.page.again(); say('From the top.') })
    }
    pane.append(el('div', 'dk-frame'))
    return P
  }

  function restingTop(P) {
    const first = P.rows.findIndex((r) => r.lit || r.kind === 'gone' || r.kind === 'fresh')
    if (first < 0) return 0
    return Math.max(0, P.rowEls[first].offsetTop - 2 * ROW)
  }

  function open(g) {
    const same = g === st.g
    st.g = g; st.said = null; st.point = null; st.held = null; st.seen.add(g)
    const group = D.groups[g]
    const refs = group.refs.slice()
    if (g === 3 && st.brought) refs.push(...D.answers['Does logout still kill the session?'].brings)
    const names = refs.map((r) => r.name)
    const read = readSay(group.say, names)
    G = { read, refs, panes: [] }
    q('.dk-num').textContent = String(g + 1)
    q('.dk-title').textContent = group.title
    const conf = q('.dk-conf'); conf.className = 'dk-conf ' + group.confidence; conf.querySelector('.lv').textContent = LV[group.confidence]
    prose.textContent = ''
    read.paras.forEach((words) => {
      const p = el('p', 'dk-para')
      words.forEach((w, i) => {
        const s = el('span', 'dk-w')
        s.dataset.sentence = w.sentence; s.dataset.piece = w.piece
        if (w.chip) {
          s.append(el('span', 'dk-name', w.chip)); s.dataset.chip = w.chip
          const next = words[i + 1]
          s.append(document.createTextNode(w.text.slice(w.chip.length) || (next && !/^[,.;:!?)]/.test(next.text) ? ' ' : '')))
        } else if (w.b || w.code) {
          const trail = w.text.match(/\s*$/)[0]
          s.append(el(w.b ? 'strong' : 'code', 'dk-chip' + (w.b ? ' b' : ''), w.text.slice(0, w.text.length - trail.length)))
          if (trail) s.append(document.createTextNode(trail))
        } else s.textContent = w.text
        p.append(s)
      })
      prose.append(p)
    })
    panes.textContent = ''
    panes.dataset.n = refs.length
    refs.forEach((ref) => { const P = paneOf(ref); panes.append(P.el); G.panes.push(P) })
    q('.dk-strip .grp').textContent = 'group ' + (g + 1) + ' of ' + D.groups.length
    toc.innerHTML = D.groups.map((x, i) => {
      const said = st.review.filter((c) => c.g === i).length
      return '<li class="' + (i === g ? 'on' : '') + (st.seen.has(i) ? ' seen' : '') + '"><button type="button" data-g="' + i + '"' + (i === g ? ' aria-current="true"' : '') + '><span class="ck" aria-hidden="true">' + (st.seen.has(i) && i !== g ? '✓' : '') + '</span><b>' + (i + 1) + '</b><span class="tt">' + esc(x.title) + '</span>' + (said ? '<span class="cm">✎ ' + said + '</span>' : '') + '</button></li>'
    }).join('')
    q('.dk-seen').textContent = st.seen.size + ' of ' + D.groups.length
    asks.innerHTML = '<button type="button" class="ak-n" aria-label="Earlier questions" data-dir="-1">‹</button><div class="ak-row">' +
      group.ask.map((a) => '<button type="button" class="ak" data-ask="' + esc(a) + '">' + esc(a) + '</button>').join('') +
      '</div><button type="button" class="ak-n" aria-label="More questions" data-dir="1">›</button>'
    dk.classList.remove('pointing')
    requestAnimationFrame(() => G.panes.forEach((P) => { if (P.code) P.code.scrollTop = restingTop(P) }))
    marks()
    if (!same) dk.classList.add('fresh-g')
  }

  // ------------------------------------------------------------------ lights
  function light(point, hand) {
    st.point = point
    const named = point ? point.names : []
    const codeAt = G.panes.findIndex((P) => P.code && named.includes(P.ref.name))
    const target = codeAt >= 0 ? codeAt : G.panes.findIndex((P) => P.code)
    let pointing = false
    G.panes.forEach((P, ix) => {
      let talk = named.includes(P.ref.name)
      if (P.code) {
        const hit = new Set(point && ix === target ? rowsPointed(P.rows, point.lines) : [])
        P.rowEls.forEach((r, i) => r.classList.toggle('pt', hit.has(i)))
        if (hit.size) {
          talk = pointing = true
          const first = Math.min(...hit), last = Math.max(...hit)
          const top = P.rowEls[first].offsetTop, bottom = P.rowEls[last].offsetTop + ROW
          if (top < P.code.scrollTop || bottom > P.code.scrollTop + P.code.clientHeight) P.code.scrollTo({ top: Math.max(0, top - 2 * ROW), behavior: reduce || !hand ? 'auto' : 'smooth' })
        }
      } else if (P.flow) { const k = named.find((n) => ROUTES[n]); if (k || !point) P.flow.route(k || null); if (k) talk = true }
      else if (P.page) { const k = named.find((n) => PAGES[P.ref.name].steps.includes(n)); if (k || !point) P.page.to(k || null); if (k) talk = true }
      P.el.classList.toggle('talk', talk)
    })
    dk.classList.toggle('pointing', pointing)
  }
  function pickSentence(sentence, piece, hand) {
    st.said = sentence
    prose.querySelectorAll('.dk-w').forEach((w) => w.classList.toggle('lit', +w.dataset.sentence === sentence))
    light(G.read.pieces[piece] || null, hand)
  }
  // Read the narration's points in order, a beat each, as walking it does.
  function pieceStarts() {
    const seen = new Map()
    prose.querySelectorAll('.dk-w').forEach((w) => { const k = +w.dataset.piece; if (k > 0 && !seen.has(k)) seen.set(k, +w.dataset.sentence) })
    return [...seen.entries()]
  }
  function clear() {
    st.said = null; st.held = null
    prose.querySelectorAll('.dk-w.lit').forEach((w) => w.classList.remove('lit'))
    showHeld(); light(null)
  }

  // ---- holding lines: press, drag, release; shift extends; a tap on a phone
  let dragging = null, touchFrom = null, touchedAt = 0
  function showHeld() {
    G.panes.forEach((P, ix) => P.rowEls && P.rowEls.forEach((r, i) => {
      r.classList.toggle('held', !!(st.held && st.heldPane === ix && i >= Math.min(st.held.a, st.held.b) && i <= Math.max(st.held.a, st.held.b)))
    }))
  }
  function where() {
    if (st.held && G.panes[st.heldPane]) {
      const P = G.panes[st.heldPane]
      const a = P.rows[Math.min(st.held.a, st.held.b)].n, b = P.rows[Math.max(st.held.a, st.held.b)].n
      return P.ref.file.split('/').pop() + ':' + (a === b ? a : a + '-' + b)
    }
    return 'group ' + (st.g + 1)
  }
  function rowAt(e) { const r = e.target.closest('[data-ln]'); const pane = e.target.closest('.dk-pane'); return r && pane ? [G.panes.findIndex((P) => P.el === pane), +r.dataset.ln] : null }
  panes.addEventListener('mousedown', (e) => {
    const hit = rowAt(e)
    if (!hit || e.button !== 0 || Date.now() - touchedAt < 800) return
    const [ix, i] = hit
    st.held = e.shiftKey && st.held && st.heldPane === ix ? { a: st.held.a, b: i } : { a: i, b: i }
    st.heldPane = ix; dragging = ix; showHeld()
  })
  panes.addEventListener('mouseover', (e) => {
    if (dragging === null) return
    const hit = rowAt(e)
    if (hit && hit[0] === dragging && st.held.b !== hit[1]) { st.held.b = hit[1]; showHeld() }
  })
  addEventListener('mouseup', () => { if (dragging !== null) { dragging = null; say('Holding ' + where() + '. Press c to comment.') } })
  panes.addEventListener('touchstart', (e) => { const t = e.touches[0]; touchFrom = t ? [t.clientX, t.clientY] : null }, { passive: true })
  panes.addEventListener('touchend', (e) => {
    const hit = rowAt(e), t = e.changedTouches[0]
    if (!hit || !touchFrom || !t || Math.hypot(t.clientX - touchFrom[0], t.clientY - touchFrom[1]) > 10) return
    touchedAt = Date.now()
    st.held = { a: hit[1], b: hit[1] }; st.heldPane = hit[0]
    showHeld(); say('Holding ' + where() + '. Tap Write a comment.')
  }, { passive: true })

  prose.addEventListener('mousedown', (e) => { if (e.button === 0) e.preventDefault() })
  prose.addEventListener('click', (e) => { const w = e.target.closest('.dk-w'); if (w) { story.touch(); pickSentence(+w.dataset.sentence, +w.dataset.piece, true) } })

  // ---- the conversation
  const stamp = () => { const s = Math.max(0, Math.round((Date.now() - st.opened) / 1000)); return Math.floor(s / 60) + ':' + String(s % 60).padStart(2, '0') }
  function post(kind, who, text, tag) {
    const empty = chat.querySelector('.dk-empty'); if (empty) empty.remove()
    const m = el('div', 'dk-msg ' + kind)
    m.dataset.step = String(cur) // what happened at a step goes when the reader steps back before it
    const w = el('div', 'who'); w.append(el('span', null, who), el('time', null, stamp()))
    m.append(el('i'), w)
    if (text !== null) m.append(el('p', null, text))
    if (tag) m.append(el('span', 'rec', tag))
    chat.append(m); chat.scrollTop = chat.scrollHeight
    return m
  }
  // An answer arrives after a beat; anything that moves the story on brings it in at once.
  function reply(text, at, then) {
    const waiting = post('agent', 'agent · ' + at, null); waiting.append(el('div', 'dk-pulse'))
    let t = 0
    const done = () => { if (st.replyNow !== done) return; st.replyNow = null; clearTimeout(t); timers.delete(t); waiting.remove(); post('agent', 'agent · ' + at, text, 'recorded answer'); if (then) then() }
    st.replyNow = done
    t = later(done, reduce ? 300 : 1300)
  }
  function tocPaint() {
    toc.querySelectorAll('button[data-g]').forEach((b) => {
      const i = +b.dataset.g, said = st.review.filter((c) => c.g === i).length
      let cm = b.querySelector('.cm')
      if (said && !cm) { cm = el('span', 'cm'); b.append(cm) }
      if (cm) cm.textContent = said ? '✎ ' + said : ''
    })
  }
  function marks() {
    G.panes.forEach((P, ix) => P.rowEls && P.rowEls.forEach((r, i) => r.classList.toggle('noted', st.review.some((c) => c.g === st.g && c.file === P.ref.file && c.ref === P.ref.name && i >= c.a && i <= c.b))))
    const n = st.review.length
    q('.dk-send').textContent = n ? '· ' + n + ' to send' : ''
  }
  function composeOpen(on, focus = true) {
    st.composing = on
    compose.classList.toggle('idle', !on)
    if (on) {
      compose.querySelector('.on').textContent = st.held ? 'comment on ' + where() : 'comment on this group'
      area.placeholder = 'Say what you would change…'
      if (focus) setTimeout(() => area.focus({ preventScroll: true }), 30)
    } else {
      area.value = ''; area.placeholder = 'Write a comment · c'
      if (document.activeElement === area) { area.blur(); desk.focus({ preventScroll: true }) }
    }
  }
  function keep(text) {
    const at = where()
    const P = G.panes[st.heldPane]
    const held = st.held && P ? { file: P.ref.file, ref: P.ref.name, a: Math.min(st.held.a, st.held.b), b: Math.max(st.held.a, st.held.b) } : { file: '', ref: '', a: -1, b: -1 }
    st.review.push(Object.assign({ g: st.g, at, text, step: cur }, held))
    post('you', 'you · ' + at + ' · in review', text)
    composeOpen(false); marks(); tocPaint()
    say('Kept for the review. Press s to send it.')
  }
  function finish(now) {
    const text = area.value.trim()
    if (!text) { say('Write something first, or press esc.'); return }
    if (!now) { keep(text); return }
    const at = where()
    composeOpen(false)
    if (st.g === 3 && /log\s*out/i.test(text)) { ask('Does logout still kill the session?', text, at); return }
    post('you', 'you · ' + at, text)
    reply(D.unknown, at)
  }
  function ask(question, typed, at) {
    const A = D.answers[question]
    if (!A) return
    if (st.composing) composeOpen(false)
    st.asked.add(question)
    post('you', 'you · ' + (at || where()), typed || question)
    const big = !!A.brings
    if (big) story.busy(true)
    reply(A.a, A.at, () => {
      if (!big) return
      st.brought = true
      open(3)
      const P = G.panes.find((x) => x.ref.name === 'logout')
      if (P) { const hit = P.rows.map((r, i) => (r.n >= 2 && r.n <= 4 ? i : -1)).filter((i) => i >= 0); P.rowEls.forEach((r, i) => r.classList.toggle('pt', hit.includes(i))); P.el.classList.add('talk') }
      const S = G.panes.find((x) => x.ref.name === 'stolen')
      if (S) S.page.play()
      story.busy(false)
      story.answered()
    })
  }
  function submit() {
    if (!st.review.length) { say('Nothing to send yet. Hold a line, press c, and add it to the review.'); return }
    say('In deck, s sends your ' + st.review.length + (st.review.length === 1 ? ' comment' : ' comments') + ' to the agent. Here they stay on the page.')
  }

  // ------------------------------------------------------------ the picture
  // deck bring --diagram: boxes and arrows, with routes the reader can play. A route lights
  // its steps a beat apart, and the code beside it lights the same step.
  const NODES = {
    login: [160, 24, 110, 'login'], token: [160, 86, 176, 'access token · 15 min'], refresh: [160, 148, 128, 'refresh(token)'],
    check: [160, 218, 0, 'session alive?'], out: [72, 294, 120, 'signed out'], again: [248, 294, 136, 'new access token'],
  }
  const EDGES = {
    e1: ['M160 38V70'], e2: ['M160 100V132', '15 min later', 168, 120], e3: ['M160 162V194'],
    e4: ['M126 232Q86 250 74 278', 'no', 88, 250], e5: ['M194 232Q234 250 246 278', 'yes', 224, 250], e6: ['M306 280Q326 160 250 96'],
  }
  const LINES = { refresh: [7, 8], check: [9, 9], out: [10, 10], again: [12, 14] }
  const ROUTES = { before: ['login', 'e1', 'token', 'e2', 'refresh', 'e3', 'check', 'e4', 'out'], now: ['login', 'e1', 'token', 'e2', 'refresh', 'e3', 'check', 'e5', 'again', 'e6', 'token'] }
  function flowOf(pane, head) {
    const routes = el('span', 'dk-routes')
    for (const [k, label] of [['before', 'idle an hour, before'], ['now', 'idle an hour, now']]) {
      const b = el('button', 'rt', label + ' ▸'); b.type = 'button'; b.dataset.route = k
      b.addEventListener('click', () => { story.touch(); play(k, true) })
      routes.append(b)
    }
    head.insertBefore(routes, head.querySelector('.chev'))
    const box = el('div', 'dk-dg')
    let svg = '<svg viewBox="0 0 330 316" role="img" aria-label="login, access token, refresh, session alive: no, signed out; yes, new access token"><defs><marker id="ah" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L8 4L0 8z" fill="context-stroke"/></marker></defs>'
    for (const [k, [d, label, lx, ly]] of Object.entries(EDGES)) {
      svg += '<path class="eg" data-k="' + k + '" d="' + d + '" pathLength="1" marker-end="url(#ah)"/><path class="eg-on" data-k="' + k + '" d="' + d + '" pathLength="1"/>'
      if (label) svg += '<text class="el" x="' + lx + '" y="' + ly + '">' + label + '</text>'
    }
    for (const [k, [x, y, w, label]] of Object.entries(NODES)) {
      const shape = w ? '<rect x="' + (x - w / 2) + '" y="' + (y - 14) + '" width="' + w + '" height="28" rx="7"/>' : '<path d="M' + (x - 86) + ' ' + y + 'L' + (x - 64) + ' ' + (y - 20) + 'H' + (x + 64) + 'L' + (x + 86) + ' ' + y + 'L' + (x + 64) + ' ' + (y + 20) + 'H' + (x - 64) + 'Z"/>'
      svg += '<g class="nd" data-k="' + k + '">' + shape + '<text x="' + x + '" y="' + (y + 4) + '">' + label + '</text></g>'
    }
    box.innerHTML = svg + '</svg>'
    pane.append(box)
    let run = 0
    function reset() { run++; box.querySelectorAll('.on').forEach((n) => n.classList.remove('on')); routes.querySelectorAll('.rt').forEach((b) => b.classList.remove('on')) }
    function code(k) {
      const P = G.panes.find((x) => x.code), lines = LINES[k]
      if (!P || !lines) return
      const hit = P.rows.map((r, i) => (r.n >= lines[0] && r.n <= lines[1] ? i : -1)).filter((i) => i >= 0)
      P.rowEls.forEach((r, i) => r.classList.toggle('pt', hit.includes(i)))
      dk.classList.add('pointing')
    }
    function play(k, hand) {
      reset()
      if (!k) return
      const mine = run
      routes.querySelector('[data-route="' + k + '"]').classList.add('on')
      const steps = ROUTES[k]
      steps.forEach((s, i) => {
        const go = () => { if (run !== mine) return; box.querySelectorAll('[data-k="' + s + '"]').forEach((n) => n.classList.add('on')); code(s) }
        if (reduce) go(); else later(go, i * 260 + (hand ? 0 : 120))
      })
    }
    return { route: (k) => play(k, false), reset }
  }

  // ------------------------------------------------------------------ pages
  // deck bring --page: markup the agent wrote, for the idea that only makes sense moving.
  const PAGES = {
    life: { steps: ['before', 'now', 'logout'], html:
      '<div class="tl"><div class="tl-ax"><span style="left:0">login</span><span style="left:20%">1 h</span><span style="left:45%">day 3</span><span style="left:100%">day 30</span></div>' +
      '<div class="tl-r" data-k="before"><span class="lb">before · <b>SESSION_TTL 1 h</b></span><div class="tr"><i class="f a" style="--w:20%"></i><b class="x" style="left:20%">idle an hour · signed out</b></div></div>' +
      '<div class="tl-r" data-k="now"><span class="lb">now · <b>SESSION_TTL 30 days</b></span><div class="tr"><i class="f a" style="--w:45%"></i><i class="f b" style="--l:45%;--w:55%"></i><span class="click" style="left:45%"></span><b class="x lo" style="left:45%">logout · signed out</b></div></div>' +
      '<p class="tl-n">time, not to scale</p></div>' },
    stolen: { steps: ['logout', 'copy', 'end'], html:
      '<div class="tl"><div class="tl-ax"><span style="left:0">login</span><span style="left:30%">logout</span><span style="left:100%">day 30</span></div>' +
      '<div class="tl-r" data-k="logout"><span class="lb">the cookie</span><div class="tr"><i class="f a" style="--w:30%"></i><span class="click" style="left:30%"></span><b class="x" style="left:30%">cleared</b></div></div>' +
      '<div class="tl-r hot" data-k="copy"><span class="lb">copied token<b>200 until day 30</b></span><div class="tr"><i class="f a" style="--w:30%"></i><i class="f b" style="--l:30%;--w:70%"></i><i class="m" style="left:52%"></i><i class="m" style="left:76%"></i><i class="m" style="left:100%"></i></div></div>' +
      '<p class="tl-n">nothing revokes it · values from logout.spec.ts</p></div>' },
  }
  function pageOf(name, holder) {
    const P = PAGES[name]
    holder.innerHTML = P.html
    const tl = holder.firstChild
    let run = 0
    function to(k) {
      const upto = k ? P.steps.indexOf(k) : -1
      P.steps.forEach((s, i) => tl.classList.toggle('s-' + s, i <= upto))
    }
    function play() {
      const mine = ++run
      to(null)
      if (reduce) { to(P.steps[P.steps.length - 1]); return }
      P.steps.forEach((s, i) => later(() => { if (run === mine) to(s) }, 200 + i * 1100))
    }
    return { to: (k) => { run++; to(k) }, again: play, play }
  }

  // ------------------------------------------------------------------ the desktop
  // Ten steps on a desktop: the terminal, deck's commands, the bar, the deck window, the answer,
  // the decision, the end. A pointer acts for the reader when they let it (Play it for me, or
  // Next); everything it does, the reader can do by hand.
  const N = 10
  const E = 'cubic-bezier(.45,0,.55,1)'
  const railBtns = [...desk.querySelectorAll('#pl-rail button')]
  const tw = $('tw'), dw = $('dw'), bar = $('pl-bar'), barC = $('bar-c'), openBtn = $('bar-open'), tx = $('tm-tx')
  const cursor = $('cursor'), endc = $('endc'), file = $('dt-file'), mbApp = $('mb-app')
  const auto = $('pl-auto'), nextBtn = $('pl-next'), prevBtn = $('pl-prev')
  const xs = $('xs'), xsGo = $('xs-go'), xsSt = $('xs-st')
  const deckDot = desk.querySelector('.di[data-app="deck"]')
  let cur = -1, pending = null, playing = false, decided = false, answeredYet = false, busyNow = false, seq = 0, shown = false
  const typed = { p: false, d: false }
  const wait = (ms) => new Promise((r) => later(r, reduce ? 0 : ms))

  function track(i, how) {
    try { if (window.posthog && window.posthog.capture) window.posthog.capture('play step', { step: i + 1, name: railBtns[i].textContent, how }) } catch (_) {}
  }

  // ---- the bar, as pill.rs: the title, "Getting deck ready… n/total" while groups arrive, then the
  // count; Open and the cross, and no keys
  function barTo(n, said) {
    if (n === null) { bar.classList.remove('up'); return }
    bar.classList.add('up')
    bar.classList.toggle('writing', n !== 'seal')
    barC.textContent = n === 'seal' ? D.groups.length + ' groups' + (said ? ' · ' + said + (said === 1 ? ' comment' : ' comments') : '') : 'Getting deck ready… ' + n + '/' + D.groups.length
  }

  // ---- the terminal: lines typed a few characters at a time, their colours kept; outputs appear whole
  const lines = [...tx.querySelectorAll('pre')]
  lines.forEach((pre) => { const w = document.createTreeWalker(pre, NodeFilter.SHOW_TEXT); pre._n = []; while (w.nextNode()) pre._n.push(w.currentNode); pre._f = pre._n.map((t) => t.data) })
  const part = (k) => lines.filter((p) => (p.dataset.ty || p.dataset.out) === k)
  const show = (pre, on) => { pre._n.forEach((t, j) => { t.data = on ? pre._f[j] : '' }); pre.classList.toggle('shown', on); pre.classList.remove('typing') }
  const spinner = $('tw-spin'), typing = $('tw-type'), hint = $('tw-ph')
  const spin = (on) => { spinner.hidden = !on }
  const input = (t) => { typing.textContent = t; hint.hidden = !!t }
  const PROMPT = part('p')[0]._f.join('').replace(/^>\s*/, '')
  function fill(keys) {
    spin(false); input('')
    lines.forEach((pre) => show(pre, keys.includes(pre.dataset.ty || pre.dataset.out)))
    if (keys.includes('d')) { barTo('seal'); typed.d = true }
    if (keys.includes('a')) typed.p = true
    tx.scrollTop = tx.scrollHeight
  }
  async function typeLine(pre, my, cps) {
    pre.classList.add('shown', 'typing')
    for (let k = 0; k < pre._n.length; k++) {
      const full = pre._f[k]
      for (let at = 0; at < full.length;) {
        if (my !== seq) return false
        at = Math.min(full.length, at + cps); pre._n[k].data = full.slice(0, at); tx.scrollTop = tx.scrollHeight
        await wait(cps > 2 ? 24 : 38)
      }
    }
    pre.classList.remove('typing')
    return my === seq
  }

  // ---- the pointer: it travels on a gentle curve, eased, and presses what it lands on
  let px = 0, py = 0, raf = 0
  function place(x, y) { px = x; py = y; cursor.style.transform = `translate(${x}px,${y}px)` }
  function aim(target) { const r = target.getBoundingClientRect(); return [r.left + Math.min(r.width * 0.55, 60), r.top + r.height * 0.55] }
  function glide(target, my) {
    return new Promise((done) => {
      if (my !== seq) return done(false)
      const [x1, y1] = aim(target)
      cursor.classList.add('on')
      if (reduce) { place(x1, y1); return done(true) }
      const x0 = px, y0 = py, d = Math.hypot(x1 - x0, y1 - y0)
      const bend = Math.min(120, d * 0.18), cx = (x0 + x1) / 2 - ((y1 - y0) / (d || 1)) * bend, cy = (y0 + y1) / 2 + ((x1 - x0) / (d || 1)) * bend
      const ms = Math.min(1100, 420 + d * 0.45), t0 = performance.now()
      cancelAnimationFrame(raf)
      const ease = (t) => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2)
      const step = (now) => {
        if (my !== seq) return done(false)
        const t = Math.min(1, (now - t0) / ms), e = ease(t), u = 1 - e
        place(u * u * x0 + 2 * u * e * cx + e * e * x1, u * u * y0 + 2 * u * e * cy + e * e * y1)
        if (t < 1) raf = requestAnimationFrame(step); else done(true)
      }
      raf = requestAnimationFrame(step)
    })
  }
  async function press(target, my, act) {
    if (!(await glide(target, my))) return false
    await wait(160)
    if (my !== seq) return false
    cursor.classList.add('down'); target.classList.add('pressed')
    await wait(140)
    cursor.classList.remove('down'); target.classList.remove('pressed')
    if (my !== seq) return false
    if (act) act(); else target.click()
    return true
  }
  // a key pressed for the reader, shown as a keycap by the pointer
  function keycap(k) { const c = el('kbd', 'keycap', k); c.style.transform = `translate(${px + 18}px,${py + 22}px)`; desk.append(c); later(() => c.remove(), reduce ? 600 : 900) }

  // ---- the deck window opens out of the bar, and closes back into it
  function fromBar() {
    const b = bar.getBoundingClientRect(), w = dw.getBoundingClientRect()
    return `translate(${(b.left + b.width / 2 - (w.left + w.width / 2)).toFixed(0)}px,${(b.top + b.height / 2 - (w.top + w.height / 2)).toFixed(0)}px) scale(${Math.max(0.12, b.width / w.width).toFixed(3)})`
  }
  function grow(out) {
    if (reduce || !dw.animate) return
    const k = [{ transform: fromBar(), opacity: 0 }, { transform: 'none', opacity: 1 }]
    dw.animate(out ? k.reverse() : k, { duration: 760, easing: E, fill: out ? 'forwards' : 'none' })
  }

  function holdLine(n) {
    const ix = G.panes.findIndex((P) => P.code && P.ref.file === 'src/auth/session.ts')
    if (ix < 0) return null
    const i = G.panes[ix].rows.findIndex((r) => r.n === n && r.kind !== 'fresh')
    G.panes[ix].code.scrollTop = Math.max(0, G.panes[ix].rowEls[i].offsetTop - 2 * ROW)
    return { ix, i, row: G.panes[ix].rowEls[i] }
  }
  const holdIt = (h) => { st.held = { a: h.i, b: h.i }; st.heldPane = h.ix; showHeld() }

  // ---- the walk: each point a beat apart, as deck walk reads it
  async function walk(my) {
    const starts = pieceStarts()
    if (!starts.length) return
    if (reduce) { pickSentence(starts[0][1], starts[0][0]); return }
    for (const [piece, sentence] of starts) { await wait(600); if (my !== seq) return; pickSentence(sentence, piece); await wait(1700) }
  }

  // ---- step 8: ask, by holding a line, c, and the question typed
  const QUESTION = 'Does logout still kill the session?'
  async function askIt(my) {
    if (answeredYet) return
    const h = holdLine(12)
    pending = () => { pending = null; seq++; composeOpen(false); if (h) holdIt(h); if (!answeredYet && !busyNow) ask(QUESTION, QUESTION, 'session.ts:12'); if (st.replyNow) st.replyNow() }
    if (reduce) { pending(); return }
    if (h && !(await press(h.row, my, () => holdIt(h)))) return
    keycap('c'); composeOpen(true, false)
    for (let at = 1; at <= QUESTION.length; at++) { if (my !== seq) return; area.value = QUESTION.slice(0, at); await wait(42) }
    await wait(300)
    if (my !== seq) return
    pending = null
    await press(compose.querySelector('.now'), my, () => finish(true))
  }

  // ---- step 9: decide, in a comment on the line; export it
  const DECISION = 'Keep 1h TTL, add revocation on logout instead'
  function done9() {
    xs.hidden = true; xsSt.textContent = ''; decided = true; pending = null
    file.hidden = false
  }
  async function decide(my, instant) {
    if (decided) { done9(); return }
    xs.hidden = true; file.hidden = true
    const finishAll = () => {
      seq++
      if (!st.review.some((c) => c.text === DECISION)) { const h = holdLine(5); if (h) holdIt(h); area.value = DECISION; keep(DECISION) }
      done9()
    }
    pending = finishAll
    if (reduce || instant) { finishAll(); return }
    const h = holdLine(5)
    if (h && !(await press(h.row, my, () => holdIt(h)))) return
    keycap('c'); composeOpen(true, false)
    for (let at = 1; at <= DECISION.length; at++) { if (my !== seq) return; area.value = DECISION.slice(0, at); await wait(34) }
    await wait(300)
    if (!(await press(compose.querySelector('.later'), my, () => keep(DECISION)))) return
    await wait(500)
    if (!(await press(dk.querySelector('.dk-exp'), my, () => { xs.hidden = false }))) return
    await wait(500)
    if (!(await press(xsGo, my, () => { xsSt.textContent = 'Laying out the pages…' }))) return
    await wait(900)
    if (my !== seq) return
    xsSt.textContent = 'Done.'
    await wait(600)
    if (my !== seq) return
    done9()
  }

  // ---- one step
  const DWELL = [2600, 1800, 2400, 2000, 2400, 2600, 3000, 5200, 3000]
  async function run(i, forward, my) {
    if (i === 0) {
      if (forward && !typed.p && !reduce) {
        // typed into the input box, sent, then the agent works: its tool calls and their results, then its answer
        fill([])
        pending = () => { pending = null; seq++; fill(['p', 'a']) }
        await wait(500)
        for (let at = 1; at <= PROMPT.length; at++) { if (my !== seq) return; input(PROMPT.slice(0, at)); await wait(38) }
        await wait(380)
        if (my !== seq) return
        input(''); show(part('p')[0], true); spin(true)
        for (const pre of part('a')) {
          await wait(pre.id === 'tm-sum' ? 1000 : 700)
          if (my !== seq) return
          if (pre.id === 'tm-sum') spin(false)
          show(pre, true); tx.scrollTop = tx.scrollHeight
        }
        typed.p = true; pending = null
      } else fill(['p', 'a'])
      await glide($('tm-sum'), my)
    } else if (i === 1) {
      if (forward && !typed.d && !reduce) {
        fill(['p', 'a'])
        pending = () => { pending = null; seq++; fill(['p', 'a', 'd']) }
        spin(true)
        for (const pre of part('d')) {
          if (pre.dataset.out) { await wait(300); if (my !== seq) return; show(pre, true); tx.scrollTop = tx.scrollHeight; continue }
          await wait(420)
          if (!(await typeLine(pre, my, 3))) return
          if (pre.dataset.bar) barTo(pre.dataset.bar === 'seal' ? 'seal' : +pre.dataset.bar)
        }
        typed.d = true; pending = null; spin(false)
      } else fill(['p', 'a', 'd'])
    } else if (i === 2) {
      fill(['p', 'a', 'd'])
      if (playing) { await wait(DWELL[2]); if (await press(openBtn, my, () => {})) go(3, 'auto'); return }
    } else if (i <= 6) await walk(my)
    else if (i === 7) {
      if (answeredYet) { const S = G.panes.find((x) => x.ref.name === 'stolen'); if (S) S.page.to('end') }
      else if (playing) { await wait(1200); await askIt(my); return }
    } else if (i === 8) await decide(my, !forward)
    if (my === seq) ready()
  }
  function ready() {
    if (!playing || cur >= N - 1) { if (cur >= N - 1) setAuto(false); return }
    const my = seq
    later(() => { if (my === seq) go(cur + 1, 'auto') }, DWELL[cur])
  }

  function render(i, how) {
    const forward = how !== 'rail' && i === cur + 1
    if (st.replyNow) st.replyNow()
    hush(); pending = null; const my = ++seq
    // only what has happened by step i: later steps' questions, answers, panes and comments go
    chat.querySelectorAll('.dk-msg').forEach((m) => { if (+m.dataset.step > i) m.remove() })
    if (!chat.querySelector('.dk-msg, .dk-empty')) chat.innerHTML = '<p class="dk-empty">The floor is yours</p>'
    st.review = st.review.filter((c) => c.step <= i)
    if (i < 7) { answeredYet = false; st.brought = false; st.asked.clear() }
    if (i < 8) { decided = false; file.hidden = true }
    if (st.composing) composeOpen(false)
    const from = cur
    cur = i
    desk.dataset.step = String(i + 1)
    railBtns.forEach((b, k) => { if (k === i) b.setAttribute('aria-current', 'step'); else b.removeAttribute('aria-current'); b.classList.toggle('done', k < i) })
    $('pl-no').textContent = i + 1 + ' / ' + N
    $('pl-say').innerHTML = railBtns[i].dataset.cap
    prevBtn.disabled = i === 0; nextBtn.disabled = i === N - 1
    const inWin = i >= 3 && i <= 8
    mbApp.textContent = i <= 2 ? 'Terminal' : inWin ? 'deck' : 'Desktop'
    deckDot.classList.toggle('on', inWin)
    tw.classList.toggle('on', i <= 2); tw.classList.toggle('back', i === 2)
    endc.hidden = i !== 9; xs.hidden = true
    bar.classList.remove('gone'); const gone = $('bar-gone'); if (gone) gone.hidden = true
    if (i === 0) barTo(null)
    if (i >= 2 && !inWin) barTo('seal', i === 9 ? st.review.length : 0)
    if (inWin) {
      bar.classList.remove('up')
      if (!st.opened) st.opened = Date.now()
      open(Math.min(i - 3, 3))
      asks.classList.toggle('invite', i === 7 && !answeredYet)
      if (!shown) { shown = true; dw.classList.add('on'); if (from === 2 && forward) grow(false) }
    } else if (shown) {
      shown = false
      if (i === 9 && from === 8 && !reduce && dw.animate) { grow(true); later(() => { dw.classList.remove('on'); dw.getAnimations().forEach((a) => a.cancel()) }, 760) } else dw.classList.remove('on')
    }
    if (i >= 3 && i <= 6) cursor.classList.remove('on')
    if (document.activeElement && document.activeElement.closest('[inert], [hidden]')) desk.focus({ preventScroll: true })
    if (how !== 'load') track(i, how)
    run(i, forward, my)
  }
  function go(i, how) { if (i >= 0 && i < N) render(i, how) }
  function next(how) {
    if (busyNow) { if (st.replyNow) st.replyNow(); return }
    if (pending) { pending(); return }
    if (cur === 2) { openBtn.focus({ preventScroll: true }); openBtn.classList.remove('pulse'); void openBtn.offsetWidth; openBtn.classList.add('pulse'); return }
    if (cur === 7 && !answeredYet) { askIt(seq); return }
    go(cur + 1, how)
  }
  function prev(how) { if (cur > 0) go(cur - 1, how) }

  // ---- play it for me: any hand pauses it, except the one pressing it
  function setAuto(on) {
    playing = on
    auto.setAttribute('aria-pressed', String(on))
    auto.querySelector('.pl-at').textContent = on ? 'Pause' : cur > 0 && cur < N - 1 ? 'Resume' : 'Play it for me'
  }
  auto.addEventListener('click', () => {
    if (playing) { setAuto(false); hush(); seq++; return }
    setAuto(true)
    if (cur >= N - 1) { reset(); go(0, 'auto') } else go(cur, 'auto')
  })
  const pause = (e) => { if (playing && !(auto.contains(e.target) && (e.type !== 'keydown' || e.key === 'Enter' || e.key === ' '))) { setAuto(false); hush(); seq++ } }
  for (const t of ['pointerdown', 'keydown', 'wheel']) desk.addEventListener(t, pause, { capture: true, passive: true })
  const story = {
    touch() { if (playing) { setAuto(false); hush(); seq++ } },
    busy(on) { busyNow = on },
    answered() { answeredYet = true; asks.classList.remove('invite'); if (cur === 7 && playing) ready() },
  }

  // ---- hands: the controls, the rail, the bar, the window's own buttons
  nextBtn.addEventListener('click', () => next('next'))
  prevBtn.addEventListener('click', () => prev('back'))
  railBtns.forEach((b, k) => b.addEventListener('click', () => go(k, 'rail')))
  openBtn.addEventListener('click', () => { if (cur === 1 || cur === 2 || cur === 9) go(3, 'open') })
  $('bar-x').addEventListener('click', () => {
    bar.classList.add('gone')
    let note = $('bar-gone')
    if (!note) {
      note = el('p', 'bar-gone'); note.id = 'bar-gone'
      note.append('Put away for good: the cross takes the deck off the bar. ')
      const back = el('button', 'bar-back', 'Bring it back'); back.type = 'button'
      back.addEventListener('click', () => { bar.classList.remove('gone'); note.hidden = true; openBtn.focus({ preventScroll: true }) })
      note.append(back); desk.append(note)
    }
    note.hidden = false
  })
  $('pl-replay').addEventListener('click', () => { reset(); go(0, 'replay'); desk.focus({ preventScroll: true }) })
  toc.addEventListener('click', (e) => { const b = e.target.closest('button[data-g]'); if (b) go(3 + +b.dataset.g, 'review') })
  asks.addEventListener('click', (e) => {
    const n = e.target.closest('.ak-n'); if (n) { asks.querySelector('.ak-row').scrollBy({ left: +n.dataset.dir * 240, behavior: reduce ? 'auto' : 'smooth' }); return }
    const b = e.target.closest('.ak'); if (!b || busyNow) return
    story.touch(); ask(b.dataset.ask)
  })
  const MESSAGES = {
    t: 't turns the panes a quarter: one above the other.', h: 'h puts the deck back on the bar, comments and all.',
    q: 'q closes without answering. Your agent hears that you closed it.', w: 'w reads the deck aloud, once deck walk has a voice.', z: 'z dims the rest of your screen behind the deck.',
  }
  const dockKey = (on) => { dock.hidden = !on; q('[data-key="?"]').setAttribute('aria-expanded', String(on)) }
  function key(k) {
    if (k === 'c') composeOpen(true)
    else if (k === 's') submit()
    else if (k === '?') dockKey(dock.hidden)
    else if (k === 'export') { xs.hidden = false; xsSt.textContent = ''; xsGo.focus({ preventScroll: true }) }
    else if (k === 'n') next('key')
    else if (k === 'p') prev('key')
    else if (MESSAGES[k]) say(MESSAGES[k])
  }
  q('.dk-strip').addEventListener('click', (e) => { const b = e.target.closest('button[data-key]'); if (b) key(b.dataset.key) })
  xsGo.addEventListener('click', () => {
    if (pending && cur === 8) { pending(); return }
    xsSt.textContent = 'Laying out the pages…'
    later(() => { xsSt.textContent = 'Done.'; later(() => { xs.hidden = true; file.hidden = false; say('Exported. Nothing is downloaded here: it is part of the recording.') }, reduce ? 0 : 600) }, reduce ? 0 : 900)
  })
  $('xs-c').addEventListener('click', () => { xs.hidden = true })
  compose.querySelector('.later').addEventListener('click', () => finish(false))
  compose.querySelector('.now').addEventListener('click', () => finish(true))
  area.addEventListener('focus', () => { if (!st.composing) composeOpen(true) })
  area.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); composeOpen(false) }
    else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); finish(!e.shiftKey) }
  })

  // ---- keys, while the desktop is up: n p and the arrows step; in the window, deck's own; Esc leaves
  document.addEventListener('keydown', (e) => {
    if (desk.hidden || e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return
    const t = e.target
    if (/^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)) return
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key
    if (k === 'Escape') { e.preventDefault(); if (!dock.hidden) dockKey(false); else if (!xs.hidden) xs.hidden = true; else leave(); return }
    if (k === 'n' || k === 'ArrowRight') { e.preventDefault(); next('key') }
    else if (k === 'p' || k === 'ArrowLeft') { e.preventDefault(); prev('key') }
    else if (cur >= 3 && cur <= 8 && (k === 'c' || k === 's' || k === '?' || MESSAGES[k])) { e.preventDefault(); key(k) }
  }, true)

  // ---- going in and out: full screen when the browser allows it, the whole window when it does not
  const page = [...document.querySelectorAll('body > :not(#desk):not(script)')]
  const canDesk = () => innerWidth > 1100 && matchMedia('(hover: hover) and (pointer: fine)').matches
  function reset() {
    seq++; hush(); setAuto(false)
    st.review = []; st.asked.clear(); st.brought = false; st.seen.clear(); st.opened = 0; st.held = null
    chat.innerHTML = '<p class="dk-empty">The floor is yours</p>'
    typed.p = typed.d = false; decided = answeredYet = shown = false; cur = -1
    file.hidden = true; xs.hidden = true; endc.hidden = true
    dw.classList.remove('on'); tw.classList.remove('on', 'back'); barTo(null)
    cursor.classList.remove('on')
  }
  function enter() {
    if (!canDesk()) return
    reset()
    desk.hidden = false
    document.documentElement.classList.add('desk-on')
    page.forEach((e) => { e.inert = true })
    try { const r = desk.requestFullscreen && desk.requestFullscreen({ navigationUI: 'hide' }); if (r && r.catch) r.catch(() => {}) } catch (_) {}
    place(innerWidth * 0.66, innerHeight * 0.72)
    desk.focus({ preventScroll: true })
    tick()
    render(0, 'load')
    track(0, 'start')
  }
  function close() {
    if (desk.hidden) return
    reset()
    desk.hidden = true
    document.documentElement.classList.remove('desk-on')
    page.forEach((e) => { e.inert = false })
    // the browser hands the focus to the page as fullscreen ends, after this: give it back to Start then too
    const back = () => start.focus({ preventScroll: true })
    back(); requestAnimationFrame(back); setTimeout(back, 120)
  }
  function leave() { if (document.fullscreenElement) document.exitFullscreen().catch(() => {}); close() }
  document.addEventListener('fullscreenchange', () => { if (!document.fullscreenElement) { if (desk.hidden) start.focus({ preventScroll: true }); else close() } })
  start.addEventListener('click', enter)
  $('dx').addEventListener('click', leave)

  // ---- the desktop's own life: the clock, and the step rail that shows while a hand is about
  const clock = $('mb-clock')
  function tick() { const d = new Date(); clock.textContent = d.toLocaleDateString('en-US', { weekday: 'short' }) + ' ' + d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' }).replace(/\s?[AP]M$/, '') }
  setInterval(() => { if (!desk.hidden) tick() }, 20000)
  let chromeT = 0
  desk.addEventListener('pointermove', () => { desk.classList.add('chrome'); clearTimeout(chromeT); chromeT = setTimeout(() => desk.classList.remove('chrome'), 2600) }, { passive: true })
  document.documentElement.classList.add('pl-ready')
})()
