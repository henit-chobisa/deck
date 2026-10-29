// /play/: a recorded deck, played in ten steps. The window is drawn the way deck draws it
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
  const pl = $('pl'), stage = $('pl-stage'), dk = $('dk')
  if (!D || !pl || !stage || !dk) return
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
      if (document.activeElement === area) { area.blur(); pl.focus({ preventScroll: true }) }
    }
  }
  function keep(text) {
    const at = where()
    const P = G.panes[st.heldPane]
    const held = st.held && P ? { file: P.ref.file, ref: P.ref.name, a: Math.min(st.held.a, st.held.b), b: Math.max(st.held.a, st.held.b) } : { file: '', ref: '', a: -1, b: -1 }
    st.review.push(Object.assign({ g: st.g, at, text }, held))
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
      '<div class="tl-r hot" data-k="copy"><span class="lb">a copied refresh token<b>refresh → 200, to day 30</b></span><div class="tr"><i class="f a" style="--w:30%"></i><i class="f b" style="--l:30%;--w:70%"></i><i class="m" style="left:52%"></i><i class="m" style="left:76%"></i><i class="m" style="left:100%"></i></div></div>' +
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

  // ------------------------------------------------------------------ story
  const N = 10
  const railBtns = [...document.querySelectorAll('#pl-rail button')]
  const scenes = { term: $('sc-term'), win: $('sc-win'), end: $('sc-end') }
  const sceneOf = (i) => (i <= 2 ? 'term' : i <= 8 ? 'win' : 'end')
  const bar = $('pl-bar'), barC = $('bar-c'), openBtn = $('bar-open'), tx = $('tm-tx')
  const auto = $('pl-auto'), nextBtn = $('pl-next'), prevBtn = $('pl-prev')
  const xs = $('xs'), pr = $('pr'), xsGo = $('xs-go'), xsSt = $('xs-st')
  const phone = () => matchMedia('(max-width: 980px)').matches
  let cur = -1, pending = null, playing = false, typed = false, decided = false, answeredYet = false, busyNow = false
  const visited = new Set()

  function track(i, how) {
    try { if (window.posthog && window.posthog.capture) window.posthog.capture('play step', { step: i + 1, name: railBtns[i].textContent, how }) } catch (_) {}
  }

  // ---- the bar, as pill.rs: the title, "Getting deck ready… n/total" while groups arrive,
  // then how many there are; Open and the cross, and no keys.
  function barTo(n) {
    if (n === null) { bar.classList.remove('up'); return }
    bar.classList.add('up')
    bar.classList.toggle('writing', n !== 'seal')
    barC.textContent = n === 'seal' ? D.groups.length + ' groups' : 'Getting deck ready… ' + n + '/' + D.groups.length
  }

  // ---- typing: every text node of a command is emptied and filled again a few characters at
  // a time, so the colours stay; an output line appears when its command is done.
  const cmds = [...document.querySelectorAll('#tm-deck pre')]
  function nodesOf(pre) { const w = document.createTreeWalker(pre, NodeFilter.SHOW_TEXT); const out = []; while (w.nextNode()) out.push(w.currentNode); return out }
  cmds.forEach((pre) => { pre._nodes = nodesOf(pre); pre._full = pre._nodes.map((t) => t.data) })
  function fillAll() {
    cmds.forEach((pre) => { pre._nodes.forEach((t, k) => { t.data = pre._full[k] }); pre.classList.add('shown'); pre.classList.remove('typing') })
    barTo('seal'); typed = true; tx.scrollTop = tx.scrollHeight
  }
  function type(done) {
    cmds.forEach((pre) => { pre._nodes.forEach((t) => { t.data = '' }); pre.classList.remove('shown') })
    barTo(null)
    let c = 0, k = 0, at = 0
    const tick = () => {
      if (c >= cmds.length) { typed = true; pending = null; done(); return }
      const pre = cmds[c]
      pre.classList.add('shown')
      if (pre.hasAttribute('data-out')) { pre._nodes.forEach((t, j) => { t.data = pre._full[j] }); c++; k = 0; at = 0; tx.scrollTop = tx.scrollHeight; later(tick, 260); return }
      pre.classList.add('typing')
      const full = pre._full[k]
      at = Math.min(full.length, at + 3)
      pre._nodes[k].data = full.slice(0, at)
      if (at >= full.length) { k++; at = 0 }
      tx.scrollTop = tx.scrollHeight
      if (k >= pre._nodes.length) {
        pre.classList.remove('typing')
        if (pre.dataset.bar) barTo(pre.dataset.bar === 'seal' ? 'seal' : +pre.dataset.bar)
        c++; k = 0; later(tick, 420)
      } else later(tick, 24)
    }
    pending = () => { hush(); fillAll(); pending = null; done() }
    later(tick, 500)
  }

  // ---- the window's walk: each point a beat apart, as deck walk reads it
  function walk(done) {
    const starts = pieceStarts()
    if (!starts.length || reduce) { if (starts.length) pickSentence(starts[0][1], starts[0][0]); done(); return }
    starts.forEach(([piece, sentence], i) => later(() => pickSentence(sentence, piece), 600 + i * 2300))
    later(done, 600 + starts.length * 2300)
  }

  // ---- step 9: the reader's decision, a comment on the line, the export and the pull request
  const DECISION = 'Keep 1h TTL, add revocation on logout instead'
  function holdLine(n) {
    const ix = G.panes.findIndex((P) => P.code && P.ref.file === 'src/auth/session.ts')
    if (ix < 0) return
    const i = G.panes[ix].rows.findIndex((r) => r.n === n && r.kind !== 'fresh')
    st.held = { a: i, b: i }; st.heldPane = ix; showHeld()
    G.panes[ix].code.scrollTop = Math.max(0, G.panes[ix].rowEls[i].offsetTop - 2 * ROW)
  }
  function exported(then) {
    xsGo.classList.add('press'); xsSt.textContent = 'Laying out the pages…'
    later(() => { xsGo.classList.remove('press'); xsSt.textContent = 'Done.'; later(() => { xs.hidden = true; pr.hidden = false; decided = true; pending = null; if (then) then() }, reduce ? 0 : 700) }, reduce ? 0 : 900)
  }
  function decide(done, instant) {
    if (decided) { pr.hidden = false; done(); return }
    xs.hidden = true; pr.hidden = true; xsSt.textContent = ''
    holdLine(5); composeOpen(true, false)
    const finishAll = () => {
      hush()
      if (!st.review.some((c) => c.text === DECISION)) { holdLine(5); area.value = DECISION; keep(DECISION) }
      xs.hidden = true; pr.hidden = false; xsSt.textContent = 'Done.'; decided = true; pending = null; done()
    }
    pending = finishAll
    if (reduce || instant) { finishAll(); return }
    let at = 0
    const tick = () => {
      at++; area.value = DECISION.slice(0, at)
      if (at < DECISION.length) { later(tick, 34); return }
      later(() => { keep(DECISION); later(() => { xs.hidden = false; later(() => exported(done), 900) }, 700) }, 500)
    }
    later(tick, 700)
  }

  // ---- one step
  const DWELL = [4200, 1800, 2600, 2400, 2800, 2800, 3400, 5500, 3200]
  function ready() {
    if (!playing || cur >= N - 1) { if (cur >= N - 1) setAuto(false); return }
    if (cur === 2) { later(() => { openBtn.classList.add('press'); later(() => { openBtn.classList.remove('press'); go(3, 'auto') }, 380) }, DWELL[2]); return }
    if (cur === 7 && !answeredYet) { later(askTyped, 1400); return }
    later(() => go(cur + 1, 'auto'), DWELL[cur])
  }
  function askTyped() {
    const Q = 'Does logout still kill the session?'
    holdLine(12); composeOpen(true, false)
    let at = 0
    pending = () => { hush(); area.value = Q; pending = null; finish(true); if (st.replyNow) st.replyNow() }
    if (reduce) { pending(); return }
    const tick = () => { at++; area.value = Q.slice(0, at); if (at < Q.length) later(tick, 40); else later(() => { pending = null; finish(true) }, 400) }
    later(tick, 300)
  }

  function render(i, how) {
    const forward = how !== 'rail' && i === cur + 1
    if (st.replyNow) st.replyNow()
    hush(); pending = null
    const from = cur
    cur = i
    stage.dataset.step = String(i + 1)
    for (const [k, s] of Object.entries(scenes)) { const on = sceneOf(i) === k; s.classList.toggle('on', on); s.inert = !on; s.setAttribute('aria-hidden', String(!on)) }
    railBtns.forEach((b, k) => { b.toggleAttribute('aria-current', k === i); if (k === i) b.setAttribute('aria-current', 'step'); b.classList.toggle('done', k < i) })
    $('pl-no').textContent = i + 1 + ' / ' + N
    $('pl-say').innerHTML = railBtns[i].dataset.cap
    prevBtn.disabled = i === 0; nextBtn.disabled = i === N - 1
    if (document.activeElement && document.activeElement.closest('[inert]')) pl.focus({ preventScroll: true })
    const first = !visited.has(i); visited.add(i)
    scenes.term.classList.toggle('s1', i === 0); scenes.term.classList.toggle('s3', i === 2)
    bar.classList.remove('gone'); const gone = $('bar-gone'); if (gone) gone.hidden = true
    scenes.win.classList.toggle('opening', from === 2 && i === 3 && !reduce)
    if (i === 0) { barTo(null); ready() }
    else if (i === 1) { if (forward && first && !typed && !reduce) type(ready); else { fillAll(); ready() } }
    else if (i === 2) { fillAll(); ready() }
    else if (i <= 8) {
      if (!st.opened) st.opened = Date.now()
      open(Math.min(i - 3, 3))
      xs.hidden = true; pr.hidden = i !== 8 || !decided
      asks.classList.toggle('invite', i === 7 && !answeredYet)
      if (i === 7) { if (answeredYet) { const S = G.panes.find((x) => x.ref.name === 'stolen'); if (S) S.page.to('end'); ready() } else ready() }
      else if (i === 8) decide(ready, !forward)
      else walk(ready)
    } else ready()
    // One frame a screen: when the step does not fit where the page is, bring the demo up under the nav.
    if (how !== 'load') {
      const navh = document.querySelector('.nav').getBoundingClientRect().bottom
      const r = stage.getBoundingClientRect(), top = pl.getBoundingClientRect().top
      if (top < navh - 12 || (phone() ? r.top > innerHeight * 0.5 : r.bottom > innerHeight + 1)) pl.scrollIntoView({ block: 'start', behavior: reduce ? 'auto' : 'smooth' })
    }
    if (how !== 'load') track(i, how)
  }
  function go(i, how) { if (i < 0 || i >= N) return; render(i, how) }
  function next(how) {
    if (busyNow) { if (st.replyNow) st.replyNow(); return }
    if (pending) { pending(); return }
    if (cur === 2) { openBtn.focus({ preventScroll: true }); openBtn.classList.remove('pulse'); void openBtn.offsetWidth; openBtn.classList.add('pulse'); say2('Open is yours to press.'); return }
    if (cur === 7 && !answeredYet) { askTyped(); return }
    go(cur + 1, how)
  }
  function prev(how) { if (cur > 0) go(cur - 1, how) }
  function say2(text) { $('pl-say').textContent = text; later(() => { if (cur >= 0) $('pl-say').innerHTML = railBtns[cur].dataset.cap }, 2600) }

  // ---- play it for me
  function setAuto(on) {
    playing = on
    auto.setAttribute('aria-pressed', String(on))
    auto.querySelector('.pl-at').textContent = on ? 'Pause' : cur > 0 && cur < N - 1 ? 'Resume' : 'Play it for me'
  }
  auto.addEventListener('click', () => {
    if (playing) { setAuto(false); hush(); return }
    setAuto(true)
    if (cur >= N - 1) { reset(); go(0, 'auto') } else if (!pending) ready()
  })
  // Any hand pauses it, except the one pressing Play it for me itself (a click, Enter or Space on it).
  const pause = (e) => { if (playing && !(auto.contains(e.target) && (e.type !== 'keydown' || e.key === 'Enter' || e.key === ' '))) { setAuto(false); hush() } }
  for (const t of ['pointerdown', 'keydown', 'wheel', 'touchstart']) document.addEventListener(t, pause, { capture: true, passive: true })

  function reset() {
    st.review = []; st.asked.clear(); st.brought = false; st.seen.clear(); st.opened = 0; st.held = null
    chat.innerHTML = '<p class="dk-empty">The floor is yours</p>'
    typed = decided = answeredYet = false; visited.clear(); cur = -1
    pr.hidden = true; xs.hidden = true
  }
  const story = {
    touch() { if (playing) { setAuto(false); hush() } },
    busy(on) { busyNow = on },
    answered() { answeredYet = true; asks.classList.remove('invite'); if (cur === 7) ready() },
  }

  // ---- hands: the buttons, the rail, the bar, the window's own controls
  nextBtn.addEventListener('click', () => next('next'))
  prevBtn.addEventListener('click', () => prev('back'))
  railBtns.forEach((b, k) => b.addEventListener('click', () => go(k, 'rail')))
  openBtn.addEventListener('click', () => { if (cur === 2 || cur === 1) go(3, 'open') })
  $('bar-x').addEventListener('click', () => {
    bar.classList.add('gone')
    let note = $('bar-gone')
    if (!note) {
      note = el('p', 'bar-gone'); note.id = 'bar-gone'
      note.append('Put away for good: the cross takes the deck off the bar. ')
      const back = el('button', 'bar-back', 'Bring it back'); back.type = 'button'
      back.addEventListener('click', () => { bar.classList.remove('gone'); note.hidden = true; openBtn.focus({ preventScroll: true }) })
      note.append(back); scenes.term.append(note)
    }
    note.hidden = false
  })
  $('pl-replay').addEventListener('click', () => { reset(); go(0, 'replay'); pl.focus({ preventScroll: true }) })
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
  function key(k) {
    if (k === 'n') next('key')
    else if (k === 'p') prev('key')
    else if (k === 'c') composeOpen(true)
    else if (k === 's') submit()
    else if (k === '?') { dock.hidden = !dock.hidden; q('[data-key="?"]').setAttribute('aria-expanded', String(!dock.hidden)) }
    else if (k === 'Escape') { if (!dock.hidden) { dock.hidden = true; q('[data-key="?"]').setAttribute('aria-expanded', 'false') } else clear() }
    else if (k === 'export') { xs.hidden = false; xsSt.textContent = ''; xsGo.focus({ preventScroll: true }) }
    else if (MESSAGES[k]) say(MESSAGES[k])
  }
  q('.dk-strip').addEventListener('click', (e) => { const b = e.target.closest('button[data-key]'); if (b) key(b.dataset.key) })
  xsGo.addEventListener('click', () => { if (pending && cur === 8) pending(); else exported(() => { if (cur !== 8) { pr.hidden = true; say('Exported. Nothing is downloaded here: it is part of the recording.') } }) })
  xs.querySelector('.xs-c').addEventListener('click', () => { xs.hidden = true })
  compose.querySelector('.later').addEventListener('click', () => finish(false))
  compose.querySelector('.now').addEventListener('click', () => finish(true))
  area.addEventListener('focus', () => { if (!st.composing) composeOpen(true) })
  area.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { e.preventDefault(); composeOpen(false); pl.focus({ preventScroll: true }) }
    else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); finish(!e.shiftKey) }
  })

  // ---- keys: the story's (n, p and the arrows) and, in the window, deck's own
  const inView = () => { const r = pl.getBoundingClientRect(); return r.bottom > 0 && r.top < innerHeight }
  document.addEventListener('keydown', (e) => {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return
    const t = e.target
    if (/^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName) || t.isContentEditable) return
    if (!inView() || (t !== document.body && !pl.contains(t) && t !== document.documentElement)) return
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key
    if (k === 'n' || k === 'ArrowRight') { e.preventDefault(); next('key') }
    else if (k === 'p' || k === 'ArrowLeft') { e.preventDefault(); prev('key') }
    else if (cur >= 3 && cur <= 8 && (k === 'c' || k === 's' || k === '?' || k === 'Escape' || MESSAGES[k])) { e.preventDefault(); key(k) }
  })

  // ---- phones: one frame a screen; a tap outside a control moves on, a swipe moves either way
  let swipe = null, swiped = 0
  stage.addEventListener('touchstart', (e) => { const t = e.touches[0]; swipe = t && e.touches.length === 1 ? [t.clientX, t.clientY] : null }, { passive: true })
  stage.addEventListener('touchend', (e) => {
    const t = e.changedTouches[0]
    if (!swipe || !t || !phone() || e.target.closest('.dk-code, .dk-chat, .ak-row, textarea')) return
    const dx = t.clientX - swipe[0], dy = t.clientY - swipe[1]
    if (Math.abs(dx) > 60 && Math.abs(dy) < 40) { swiped = Date.now(); if (dx < 0) next('swipe'); else prev('swipe') }
  }, { passive: true })
  stage.addEventListener('click', (e) => {
    if (!phone() || Date.now() - swiped < 500) return
    if (e.target.closest('button, a, textarea, input, label, [data-ln], .dk-w, .dk-prev, .dk-chat, .dk-compose, .dk-dg, .dk-page, .xs, .pr, .ak-row')) return
    if (String(getSelection() || '')) return
    next('tap')
  })

  render(0, 'load')
  document.documentElement.classList.add('pl-ready')
})()
