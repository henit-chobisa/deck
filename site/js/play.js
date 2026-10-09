// A real deck, running in the browser: the Black Friday plan, exactly as the
// agent wrote it (js/deck-data.js), read and lit the way deck reads and lights
// it. The narration is cut into pieces at its points; a click takes the
// sentence it lands in and lights the point of the piece it lands in. Lights
// come up in 150 ms when a hand raised them, hand over in 200, and go out in
// 420, as in pane.rs. The page is the deck's own page, in a frame, wearing
// deck's colours and hearing points through deck's own shim (page.rs).
(function () {
  'use strict'
  const DECK = window.DECK
  if (!DECK) return

  // ------------------------------------------------------------------ prose
  // A point is lines, names, or both: "19-21 today", "+23-24 offload", "spike".
  function readPoint(text) {
    const point = { lines: null, names: [] }
    for (const part of (text || '').trim().split(/\s+/).filter(Boolean)) {
      const m = /^(\+)?(\d+)(?:-(\d+))?$/.exec(part)
      if (m && !point.lines) point.lines = { after: !!m[1], from: +m[2], to: +(m[3] || m[2]) }
      else point.names.push(part)
    }
    return point
  }

  // Paragraphs of words. Every word knows its sentence and the piece of the
  // narration it falls in; a piece starts at each point.
  function readSay(say, refNames) {
    const pieces = [null]
    const paras = []
    let piece = 0, sentence = 0
    for (const raw of say.split(/\n\s*\n/)) {
      const segs = []
      const re = /\[point(?:\s+([^\]]*))?\]|\[pause\]|\[([A-Za-z][\w-]*)\]|\*\*([^*]+)\*\*|\*([^*\n]+)\*|`([^`]+)`|([^[*`]+|[[*`])/g
      let m
      const text = raw.trim()
      while ((m = re.exec(text))) {
        if (m[0].startsWith('[point')) { pieces.push(readPoint(m[1])); piece = pieces.length - 1; continue }
        if (m[0] === '[pause]') continue
        if (m[2] !== undefined) { if (refNames.includes(m[2])) segs.push({ chip: m[2], piece }); else segs.push({ text: m[0], piece }); continue }
        if (m[3] !== undefined) segs.push({ text: m[3], b: true, piece })
        else if (m[4] !== undefined) segs.push({ text: m[4], em: true, piece })
        else if (m[5] !== undefined) segs.push({ text: m[5], code: true, piece })
        else segs.push({ text: m[6], piece })
      }
      // Words, each carrying the space after it, so a lit sentence is one
      // continuous band rather than a row of islands.
      const words = []
      for (const s of segs) {
        if (s.chip) { words.push({ chip: s.chip, text: s.chip, piece: s.piece }); continue }
        const parts = s.text.match(/\S+\s*|\s+/g) || []
        for (const p of parts) {
          if (!p.trim()) { if (words.length) words[words.length - 1].text += p; continue }
          words.push({ text: p, b: s.b, em: s.em, code: s.code, piece: s.piece })
        }
      }
      for (const w of words) {
        w.sentence = sentence
        if (!w.chip && /[.!?]["”’)]?\s*$/.test(w.text)) sentence++
      }
      if (words.length && (words[words.length - 1].chip || !/[.!?]["”’)]?\s*$/.test(words[words.length - 1].text))) sentence++
      paras.push(words)
    }
    return { paras, pieces }
  }

  // ------------------------------------------------------------------- code
  const KW = new Set(['import', 'from', 'type', 'const', 'let', 'await', 'async', 'return', 'if', 'export', 'class', 'private', 'readonly', 'new', 'this', 'extends', 'super'])
  const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]))
  function paint(line) {
    const t = line.trimStart()
    if (t.startsWith('/**') || t.startsWith('*') || t.startsWith('//')) return '<span class="c">' + esc(line) + '</span>'
    const re = /(\/\/.*$)|(`[^`]*`|"[^"]*"|'[^']*')|(@\w+)|(\b\d[\d_.]*\b)|([A-Za-z_$][\w$]*)(?=\s*[(<])|([A-Za-z_$][\w$]*)|(\s+|.)/g
    let out = '', m
    while ((m = re.exec(line))) {
      if (m[1]) out += '<span class="c">' + esc(m[1]) + '</span>'
      else if (m[2]) out += '<span class="s">' + esc(m[2]) + '</span>'
      else if (m[3]) out += '<span class="d">' + esc(m[3]) + '</span>'
      else if (m[4]) out += '<span class="nu">' + esc(m[4]) + '</span>'
      else if (m[5]) out += KW.has(m[5]) ? '<span class="k">' + m[5] + '</span>' : /^[A-Z]/.test(m[5]) ? '<span class="ty">' + m[5] + '</span>' : '<span class="f">' + m[5] + '</span>'
      else if (m[6]) out += KW.has(m[6]) ? '<span class="k">' + m[6] + '</span>' : /^[A-Z][a-z]/.test(m[6]) ? '<span class="ty">' + m[6] + '</span>' : esc(m[6])
      else out += esc(m[7])
    }
    return out
  }

  // The rows a code ref draws: the whole file, with its range lit, or with the
  // range replaced — every old line going, then every new line arriving, which
  // is how deck draws a proposed change.
  function rowsOf(ref) {
    const [a, b] = ref.range
    const src = ref.src
    const rows = []
    const plain = (i) => ({ n: i + 1, text: src[i], kind: '', lit: false })
    for (let i = 0; i < a - 1; i++) rows.push(plain(i))
    if (ref.after) {
      for (let i = a - 1; i < b; i++) rows.push({ n: i + 1, text: src[i], kind: 'gone', lit: false })
      ref.after.forEach((text, j) => rows.push({ n: a + j, text, kind: 'fresh', lit: false }))
      const shift = ref.after.length - (b - a + 1)
      for (let i = b; i < src.length; i++) rows.push({ n: i + 1 + shift, text: src[i], kind: '', lit: false })
    } else if (ref.before !== undefined) {
      for (let i = a - 1; i < b; i++) rows.push({ n: i + 1, text: src[i], kind: 'fresh', lit: false })
      for (let i = b; i < src.length; i++) rows.push(plain(i))
    } else {
      for (let i = a - 1; i < b; i++) rows.push({ n: i + 1, text: src[i], kind: '', lit: true })
      for (let i = b; i < src.length; i++) rows.push(plain(i))
    }
    return rows
  }

  // Which rows a point's lines name. `+` reads the arriving side of a change;
  // without it, a change's going side when it has one.
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

  // ------------------------------------------------------------------- page
  // deck's own palette variables and shim, from page.rs `dressed` and `SHIM`,
  // with two differences: points arrive by message, since the frame is sandboxed,
  // and the ground is the pane's own colour, since Safari paints a frame white
  // behind a transparent page.
  const PALETTE = { bg: '#1a1a1a', fg: '#ebdbb2', accent: '#fe8019', on_accent: '#282828', muted: '#a89984', edge: '#3a3735', wash: '#1a1a1a', add: '#b8bb26', del: '#fb4934', comment: '#928374' }
  const DRESS = '<style>:root{--deck-bg:' + PALETTE.bg + ';--deck-fg:' + PALETTE.fg + ';--deck-accent:' + PALETTE.accent +
    ';--deck-on-accent:' + PALETTE.on_accent + ';--deck-muted:' + PALETTE.muted + ';--deck-edge:' + PALETTE.edge +
    ';--deck-wash:' + PALETTE.wash + ';--deck-add:' + PALETTE.add + ';--deck-del:' + PALETTE.del + ';--deck-comment:' + PALETTE.comment + ';}' +
    'html,body{background:' + PALETTE.wash + ';color:var(--deck-fg);font:12px ui-monospace,SFMono-Regular,Menlo,monospace;}' +
    '[data-show],[data-from]{transition:opacity .35s}[data-show]:not(.deck-seen),[data-from]:not(.deck-seen){opacity:0!important;pointer-events:none!important}</style>'
  const SHIM = '<script>window.deck=window.deck||{at:null};(function(){var step=null;function apply(at){var meta=document.querySelector("meta[name=deck-points]");var order=meta?meta.content.split(/\\s+/):[];if(at===null){step=null;}else if(order.indexOf(at)>=0){step=at;}var here=order.indexOf(step);var names=function(e,k){var v=e.getAttribute(k);return v===null?null:v.split(/\\s+/);};document.querySelectorAll("[data-on]").forEach(function(e){e.classList.toggle("on",at!==null&&names(e,"data-on").indexOf(at)>=0);});document.querySelectorAll("[data-show],[data-from]").forEach(function(e){var show=names(e,"data-show"),from=names(e,"data-from"),seen=true;if(show){seen=at!==null&&show.indexOf(at)>=0;}if(from){var f=order.indexOf(from[0]);seen=seen&&here>=0&&f>=0&&here>=f;}e.classList.toggle("deck-seen",seen);});}' +
    'addEventListener("deck:point",function(e){apply(e.detail);});addEventListener("DOMContentLoaded",function(){apply(window.deck.at);if(window.deck.at!==null){window.dispatchEvent(new CustomEvent("deck:point",{detail:window.deck.at}));}});' +
    'addEventListener("message",function(e){var d=e.data;if(!d||d.deck!=="point")return;window.deck.at=d.at;window.dispatchEvent(new CustomEvent("deck:point",{detail:d.at}));});})();<\/script>'

  // ---------------------------------------------------------------- answers
  const ANSWERS = [
    { q: 'Where does the 260 ms come from?', a: 'From last month\'s traces with the render taken out: charging the card and writing the order, and nothing else. That is the green bar in plan.', go: [1, 'offload'] },
    { q: 'What if a worker crashes mid-receipt?', a: 'The job stays on the queue until a worker finishes it, so a crash means the receipt is retried, not lost. It can arrive late, which is the backlog in plan.', go: [1, 'backlog'] },
    { q: 'Why not buy bigger checkout machines?', a: 'They would cut the render time, not remove it, and we would pay for Black Friday capacity all year. The worker idles at two the rest of the year.', go: [2, 'autoscale'] },
  ]
  const UNKNOWN = 'This deck is running in your browser with three answers written in advance, so I cannot answer that one. In deck, your own agent answers whatever you ask, right here, and can bring a file or draw what it means.'

  // ----------------------------------------------------------------- window
  function start() {
    const dk = document.getElementById('dk')
    if (!dk) return
    dk.innerHTML =
      '<div class="dk-band">' +
        '<div class="dk-say"><div class="dk-title"></div><div class="dk-prose"></div></div>' +
        '<div class="dk-keys"><p>KEYS</p>' +
          [['n', 'next'], ['p', 'previous'], ['w', 'walk'], ['c', 'comment'], ['t', 'turn'], ['⎋', 'clear'], ['z', 'lights'], ['h', 'hide'], ['s', 'submit'], ['q', 'close']]
            .map(([k, l]) => '<button type="button" data-key="' + (k === '⎋' ? 'Escape' : k) + '"><kbd>' + k + '</kbd>' + l + '</button>').join('') +
        '</div>' +
      '</div>' +
      '<div class="dk-room"><div class="dk-panes"></div>' +
        '<aside class="dk-rail">' +
          '<button type="button" class="dk-rail-head" aria-expanded="false"><span class="chev">›</span><span class="lab">CHAT</span><span class="sub"></span><span class="dot"></span></button>' +
          '<div class="dk-chat" data-lenis-prevent><p class="dk-empty">The floor is yours</p></div>' +
          '<div class="dk-compose idle"><div class="on"></div><textarea aria-label="Your comment" placeholder="Write a comment · c"></textarea>' +
            '<div class="row"><span>esc discard</span><button type="button" class="later">Add to review<span class="hint">⇧⌘↩</span></button><button type="button" class="now">Ask now<span class="hint">⌘↩</span></button></div></div>' +
        '</aside>' +
      '</div>' +
      '<div class="dk-foot"><span class="grp"></span><span class="msg" aria-live="polite"></span><span class="r"><span class="cnt">0 comments</span><span>' + DECK.id + '</span><span>deck 0.1.4</span></span></div>'

    const $ = (s) => dk.querySelector(s)
    const prose = $('.dk-prose'), panes = $('.dk-panes'), rail = $('.dk-rail'), chat = $('.dk-chat')
    const compose = $('.dk-compose'), area = compose.querySelector('textarea'), foot = $('.dk-foot .msg')
    $('.dk-title').textContent = DECK.title
    const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches
    const ROW = 19.44
    let pageHtml = null
    const st = { g: 0, said: null, point: null, held: null, heldPane: -1, comments: [], composing: false, armed: false, footTimer: 0 }
    let G = null

    function say(msg) {
      foot.textContent = msg
      clearTimeout(st.footTimer)
      st.footTimer = setTimeout(() => { foot.textContent = '' }, 4200)
    }

    // ---- a group
    function open(g) {
      st.g = g; st.said = null; st.point = null; st.held = null
      const group = DECK.groups[g]
      const names = group.refs.map((r) => r.name).filter(Boolean)
      const read = readSay(group.say, names)
      G = { read, refs: group.refs, panes: [] }
      prose.textContent = ''
      read.paras.forEach((words) => {
        const p = document.createElement('div'); p.className = 'dk-para'
        words.forEach((w, i) => {
          const el = document.createElement('span'); el.className = 'dk-w'
          el.dataset.sentence = w.sentence; el.dataset.piece = w.piece
          if (w.chip) {
            const chip = document.createElement('span'); chip.className = 'dk-chip'; chip.textContent = w.chip
            el.appendChild(chip); el.dataset.chip = w.chip
            const rest = w.text.slice(w.chip.length)
            const next = words[i + 1]
            el.appendChild(document.createTextNode(rest || (next && !/^[,.;:!?)]/.test(next.text) ? ' ' : '')))
          } else if (w.b || w.em || w.code) {
            const inner = document.createElement(w.b ? 'b' : w.em ? 'em' : 'code')
            const trail = w.text.match(/\s*$/)[0]
            inner.textContent = w.text.slice(0, w.text.length - trail.length)
            el.appendChild(inner); if (trail) el.appendChild(document.createTextNode(trail))
          } else el.textContent = w.text
          p.appendChild(el)
        })
        prose.appendChild(p)
      })
      panes.textContent = ''
      group.refs.forEach((ref, ix) => {
        const pane = document.createElement('section'); pane.className = 'dk-pane'
        const head = document.createElement('div'); head.className = 'dk-head'
        const nm = document.createElement('span'); nm.className = 'nm'; nm.textContent = ref.name || ''
        head.appendChild(nm)
        if (ref.kind === 'code') { const path = document.createElement('span'); path.className = 'path'; path.textContent = ref.file + ':' + ref.range[0] + '-' + ref.range[1]; head.appendChild(path) }
        const note = document.createElement('div'); note.className = 'dk-note'; note.textContent = ref.note || ''
        const frame = document.createElement('div'); frame.className = 'dk-frame'
        pane.append(head, note)
        const P = { ref, el: pane }
        if (ref.kind === 'code') {
          const code = document.createElement('div'); code.className = 'dk-code'; code.setAttribute('data-lenis-prevent', '')
          const box = document.createElement('div'); box.className = 'dk-rows'
          P.rows = rowsOf(ref)
          box.innerHTML = P.rows.map((r, i) => '<div class="dk-row' + (r.kind ? ' ' + r.kind : '') + (r.lit ? ' lit' : '') + '" data-i="' + i + '"><span class="bar">' + (r.kind === 'gone' ? '−' : r.kind === 'fresh' ? '+' : '▌') + '</span><span class="num">' + r.n + '</span><span class="txt">' + (paint(r.text || '') || ' ') + '</span></div>').join('')
          code.appendChild(box)
          pane.appendChild(code)
          P.code = code; P.rowEls = Array.from(box.children)
          wireRows(P, ix)
        } else {
          const again = document.createElement('button'); again.type = 'button'; again.className = 'dk-again'; again.textContent = 'again ↺'
          const holder = document.createElement('div'); holder.className = 'dk-page'
          pane.append(again, holder)
          P.holder = holder
          again.addEventListener('click', () => loadPage(P, true))
          loadPage(P, false)
        }
        pane.appendChild(frame)
        panes.appendChild(pane)
        G.panes.push(P)
      })
      $('.dk-foot .grp').textContent = 'group ' + (g + 1) + '/' + DECK.groups.length
      dk.classList.remove('pointing')
      requestAnimationFrame(() => G.panes.forEach((P) => { if (P.code) P.code.scrollTop = restingTop(P) }))
      remarkMarks()
    }

    // Where a code pane rests: two lines above its range, or as far as it goes.
    function restingTop(P) {
      const first = P.rows.findIndex((r) => r.lit || r.kind)
      if (first < 0) return 0
      const max = Math.max(0, P.code.scrollHeight - P.code.clientHeight)
      return Math.max(0, Math.min(max, (first - 2) * ROW))
    }

    // ---- the page
    function loadPage(P, again) {
      if (pageHtml === null) return
      const frame = document.createElement('iframe')
      frame.setAttribute('sandbox', 'allow-scripts')
      frame.setAttribute('title', (P.ref.name || 'page') + ': ' + (P.ref.note || ''))
      frame.srcdoc = DRESS + SHIM + pageHtml
      frame.addEventListener('load', () => { const at = pagePoint(st.point); if (at) send(P, at) })
      P.holder.textContent = ''
      P.holder.appendChild(frame)
      P.frame = frame
      if (again) say('From the top.')
    }
    function send(P, at) { if (P.frame && P.frame.contentWindow) P.frame.contentWindow.postMessage({ deck: 'point', at }, '*') }
    function pagePoint(point) { return point ? point.names.find((n) => !G.refs.some((r) => r.name === n)) || null : null }

    // ---- lights
    function light(point) {
      st.point = point
      const codeIx = G.refs.findIndex((r) => r.kind === 'code')
      const named = point ? point.names.filter((n) => G.refs.some((r) => r.name === n)) : []
      let pointing = false
      G.panes.forEach((P, ix) => {
        let talk = false
        if (P.code) {
          const hit = new Set(point && ix === codeIx ? rowsPointed(P.rows, point.lines) : [])
          P.rowEls.forEach((el, i) => el.classList.toggle('pt', hit.has(i)))
          if (hit.size) {
            talk = true; pointing = true
            // Glide to the lines, if they are not already in view.
            const first = Math.min(...hit), last = Math.max(...hit)
            const top = first * ROW, bottom = (last + 1) * ROW
            if (top < P.code.scrollTop || bottom > P.code.scrollTop + P.code.clientHeight) {
              P.code.scrollTo({ top: Math.max(0, top - 2 * ROW), behavior: reduce ? 'auto' : 'smooth' })
            }
          }
        } else {
          const at = pagePoint(point)
          if (at) { send(P, at); talk = true }
        }
        if (named.includes(P.ref.name)) talk = true
        P.el.classList.toggle('talk', talk)
      })
      dk.classList.toggle('pointing', pointing)
    }

    function pickSentence(sentence, piece) {
      st.said = sentence
      prose.querySelectorAll('.dk-w').forEach((w) => w.classList.toggle('lit', +w.dataset.sentence === sentence))
      light(G.read.pieces[piece] || null)
    }

    function clear() {
      st.said = null; st.held = null
      prose.querySelectorAll('.dk-w.lit').forEach((w) => w.classList.remove('lit'))
      showHeld()
      light(null)
    }

    // ---- picking lines: press, drag, release; shift extends
    let dragging = null, touchFrom = null, touchedAt = 0
    function showHeld() {
      G.panes.forEach((P, ix) => P.rowEls && P.rowEls.forEach((el, i) => {
        const on = st.held && st.heldPane === ix && i >= Math.min(st.held.a, st.held.b) && i <= Math.max(st.held.a, st.held.b)
        el.classList.toggle('held', !!on)
      }))
    }
    function wireRows(P, ix) {
      P.code.addEventListener('mousedown', (e) => {
        const row = e.target.closest('.dk-row')
        if (!row || e.button !== 0 || Date.now() - touchedAt < 800) return
        const i = +row.dataset.i
        st.held = e.shiftKey && st.held && st.heldPane === ix ? { a: st.held.a, b: i } : { a: i, b: i }
        st.heldPane = ix; dragging = ix
        showHeld()
      })
      P.code.addEventListener('mouseover', (e) => {
        if (dragging !== ix) return
        const row = e.target.closest('.dk-row')
        if (row && st.held && st.held.b !== +row.dataset.i) { st.held.b = +row.dataset.i; showHeld() }
      })
      P.code.addEventListener('touchstart', (e) => { const t = e.touches[0]; touchFrom = t ? [t.clientX, t.clientY] : null }, { passive: true })
      P.code.addEventListener('touchend', (e) => {
        const row = e.target.closest('.dk-row'), t = e.changedTouches[0]
        if (!row || !touchFrom || !t || Math.hypot(t.clientX - touchFrom[0], t.clientY - touchFrom[1]) > 10) return
        touchedAt = Date.now()
        st.held = { a: +row.dataset.i, b: +row.dataset.i }; st.heldPane = ix
        showHeld(); say('Selected ' + where() + '. Open CHAT below to comment.')
      }, { passive: true })
    }
    window.addEventListener('mouseup', () => { if (dragging !== null) { dragging = null; say('Selected ' + where() + '. Press c to comment.') } })

    // A sentence: a click takes the whole of it, and the point it falls in.
    prose.addEventListener('mousedown', (e) => { if (e.button === 0) e.preventDefault() })
    prose.addEventListener('click', (e) => {
      const w = e.target.closest('.dk-w')
      if (w) pickSentence(+w.dataset.sentence, +w.dataset.piece)
    })
    prose.addEventListener('mouseover', (e) => {
      const w = e.target.closest('.dk-w[data-chip]')
      prose.querySelectorAll('.dk-w.on').forEach((x) => { if (x !== w) x.classList.remove('on') })
      if (w) w.classList.add('on')
    })
    prose.addEventListener('mouseleave', () => prose.querySelectorAll('.dk-w.on').forEach((x) => x.classList.remove('on')))

    function where() {
      if (st.held) {
        const P = G.panes[st.heldPane]
        const a = P.rows[Math.min(st.held.a, st.held.b)].n, b = P.rows[Math.max(st.held.a, st.held.b)].n
        return P.ref.file.split('/').pop() + ':' + (a === b ? a : a + '-' + b)
      }
      return 'group ' + (st.g + 1)
    }

    // ---- the rail
    function openRail(on) {
      rail.classList.toggle('open', on)
      $('.dk-rail-head').setAttribute('aria-expanded', String(on))
      if (on) rail.classList.remove('unread')
    }
    function composeOpen(on) {
      st.composing = on
      compose.classList.toggle('idle', !on)
      if (on) {
        openRail(true)
        compose.querySelector('.on').textContent = 'comment on ' + where()
        area.placeholder = 'Say what you would change…'
        setTimeout(() => area.focus({ preventScroll: true }), 30)
      } else {
        area.value = ''; area.placeholder = 'Write a comment · c'; area.blur()
        dk.focus({ preventScroll: true })
      }
    }
    function post(kind, who, text, extra) {
      const empty = chat.querySelector('.dk-empty'); if (empty) empty.remove()
      const m = document.createElement('div'); m.className = 'dk-msg ' + kind
      const w = document.createElement('div'); w.className = 'who'; w.textContent = who
      m.append(document.createElement('i'), w)
      if (text !== null) { const p = document.createElement('p'); p.textContent = text; m.append(p) }
      if (extra) m.append(extra)
      chat.append(m); chat.scrollTop = chat.scrollHeight
      if (!rail.classList.contains('open')) rail.classList.add('unread')
      return m
    }
    const stamp = () => { const d = new Date(); return d.getHours() + ':' + String(d.getMinutes()).padStart(2, '0') }
    function reply(text, then) {
      const pulse = document.createElement('div'); pulse.className = 'dk-pulse'
      const waiting = post('agent', 'agent · g' + (st.g + 1), null, pulse)
      setTimeout(() => { waiting.remove(); post('agent', 'agent · g' + (st.g + 1) + ' · ' + stamp(), text); if (then) then() }, 1300)
    }
    function updateCount() {
      const n = st.comments.length
      $('.dk-foot .cnt').textContent = n + (n === 1 ? ' comment' : ' comments')
      $('.dk-rail-head .sub').textContent = n ? '· ' + n + ' to send' : ''
    }
    function remarkMarks() {
      G.panes.forEach((P, ix) => P.rowEls && P.rowEls.forEach((el, i) => {
        el.classList.toggle('noted', st.comments.some((c) => c.g === st.g && c.pane === ix && i >= c.a && i <= c.b))
      }))
    }
    function finish(now) {
      const text = area.value.trim()
      if (!text) { say('Write something first, or press esc.'); return }
      const at = where()
      if (now) {
        post('you', 'you · ' + at + ' · ' + stamp(), text)
        composeOpen(false); reply(UNKNOWN)
      } else {
        const held = st.held ? { pane: st.heldPane, a: Math.min(st.held.a, st.held.b), b: Math.max(st.held.a, st.held.b) } : { pane: -1, a: -1, b: -1 }
        st.comments.push(Object.assign({ g: st.g, at, text }, held))
        post('you', 'you · ' + at + ' · in review', text)
        composeOpen(false); updateCount(); remarkMarks()
        say('Kept for the review. Press s to submit.')
      }
    }
    function ask(i) {
      const A = ANSWERS[i]
      if (st.composing) composeOpen(false)
      st.armed = true; dk.focus({ preventScroll: true })
      openRail(true)
      post('you', 'you · group ' + (st.g + 1) + ' · ' + stamp(), A.q)
      reply(A.a, () => {
        if (st.g !== A.go[0]) open(A.go[0])
        const piece = G.read.pieces.findIndex((p) => p && p.names.includes(A.go[1]))
        const word = prose.querySelector('.dk-w[data-piece="' + piece + '"]')
        if (word) pickSentence(+word.dataset.sentence, piece)
      })
    }
    function submit() {
      if (!st.comments.length) { say('Nothing to send yet. Select a line, press c, and add it to the review.'); return }
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
      st.comments = []; updateCount(); remarkMarks()
    }

    // ---- keys
    const MESSAGES = {
      w: 'w reads the deck aloud once deck walk has set up a voice.',
      t: 't turns the panes a quarter in the window.',
      z: 'z dims the rest of your screen behind the deck.',
      h: 'h puts the deck back on the bar, comments and all.',
      q: 'q closes without answering. Your agent hears that you closed it.',
    }
    function key(k) {
      if (k === 'n') { if (st.g < DECK.groups.length - 1) open(st.g + 1) }
      else if (k === 'p') { if (st.g > 0) open(st.g - 1) }
      else if (k === 'c') composeOpen(true)
      else if (k === 's') submit()
      else if (k === 'Escape') clear()
      else if (MESSAGES[k]) say(MESSAGES[k])
    }
    function onKey(e) {
      if (e.target === area) {
        if (e.key === 'Escape') { e.preventDefault(); composeOpen(false) }
        else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); finish(!e.shiftKey) }
        return
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return
      if (e.key === 'Escape') { e.preventDefault(); key('Escape'); return }
      const k = e.key.toLowerCase()
      if (k.length === 1 && 'npcswtzhq'.includes(k)) { e.preventDefault(); key(k) }
    }
    // Safari does not focus the window when something inside it is clicked,
    // so keys would go to the page. A press inside arms them; a press
    // anywhere else disarms them.
    document.addEventListener('pointerdown', (e) => {
      st.armed = dk.contains(e.target)
      if (st.armed && !e.target.closest('textarea, button, input')) dk.focus({ preventScroll: true })
    })
    document.addEventListener('keydown', (e) => {
      if (!st.armed || dk.contains(e.target) || /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName)) return
      onKey(e)
    })
    dk.addEventListener('keydown', onKey)
    area.addEventListener('focus', () => { if (!st.composing) composeOpen(true) })
    compose.querySelector('.later').addEventListener('click', () => finish(false))
    compose.querySelector('.now').addEventListener('click', () => finish(true))
    $('.dk-rail-head').addEventListener('click', () => openRail(!rail.classList.contains('open')))
    $('.dk-keys').addEventListener('click', (e) => { const b = e.target.closest('button'); if (b) key(b.dataset.key) })
    document.querySelectorAll('[data-ask]').forEach((b) => b.addEventListener('click', () => ask(+b.dataset.ask)))

    open(0)

    // ---- the first move, shown once
    // Most people scroll past a demo that looks like a picture. So when the
    // deck first comes into view, a cursor shows the first move — a click on
    // a sentence, which lights its lines — and hands over: your turn.
    const badge = document.createElement('span'); badge.className = 'dk-live'
    badge.innerHTML = '<span class="live-dot"></span>LIVE · TRY IT'
    dk.appendChild(badge)
    let touched = false
    const coach = document.createElement('div'); coach.className = 'dk-coach'; coach.setAttribute('role', 'status')
    dk.appendChild(coach)
    function dismiss() { touched = true; coach.classList.remove('on'); badge.style.opacity = '0.75' }
    dk.addEventListener('pointerdown', dismiss, { once: true })
    dk.addEventListener('keydown', dismiss, { once: true })
    function place(el, target, dx, dy) {
      const a = dk.getBoundingClientRect(), r = target.getBoundingClientRect()
      return [r.left - a.left + dx, r.bottom - a.top + dy]
    }
    function invite() {
      if (touched) return
      dk.classList.add('invite')
      const target = [...prose.querySelectorAll('.dk-w')].find((w) => /^Anywhere/.test(w.textContent)) || prose.querySelector('.dk-w')
      if (!target) return
      const handIn = () => {
        if (touched) return
        coach.innerHTML = '<b>Your turn.</b> Click any sentence to see the code it rests on. ' + (matchMedia('(hover: hover)').matches ? 'Press <b>n</b> for the next part of the plan.' : 'Use <b>next</b> for the next part of the plan.')
        const [x, y] = place(coach, target, 0, 12)
        coach.style.left = Math.max(12, Math.min(x, dk.clientWidth - 300)) + 'px'
        coach.style.top = y + 'px'
        coach.classList.add('on')
        setTimeout(() => coach.classList.remove('on'), 9000)
      }
      if (reduce) { pickSentence(+target.dataset.sentence, +target.dataset.piece); handIn(); return }
      const ghost = document.createElement('div'); ghost.className = 'dk-ghost'
      ghost.innerHTML = '<svg width="26" height="32" viewBox="0 0 28 34"><path d="M2 2 L2 28 L9 21 L14 32 L19 30 L14 19 L24 19 Z" fill="#ebdbb2" stroke="#0d0f0f" stroke-width="1.6" stroke-linejoin="round"/></svg>'
      dk.appendChild(ghost)
      const r = target.getBoundingClientRect(), a = dk.getBoundingClientRect()
      const tx = r.left - a.left + Math.min(60, r.width / 2), ty = r.top - a.top + r.height / 2
      ghost.style.transform = 'translate(' + (dk.clientWidth * 0.6) + 'px,' + (dk.clientHeight * 0.55) + 'px)'
      requestAnimationFrame(() => requestAnimationFrame(() => {
        if (touched) { ghost.remove(); return }
        ghost.style.opacity = '1'
        ghost.style.transform = 'translate(' + tx + 'px,' + ty + 'px)'
      }))
      setTimeout(() => {
        if (touched) { ghost.remove(); return }
        ghost.classList.add('clicked')
        pickSentence(+target.dataset.sentence, +target.dataset.piece)
      }, 1350)
      setTimeout(() => { ghost.style.opacity = '0'; handIn() }, 2300)
      setTimeout(() => ghost.remove(), 2800)
    }
    const seen = new IntersectionObserver((es) => {
      if (es.some((e) => e.isIntersecting && e.intersectionRatio >= 0.45)) { seen.disconnect(); setTimeout(invite, 500) }
    }, { threshold: [0.45] })
    seen.observe(dk)

    fetch(DECK.page).then((r) => (r.ok ? r.text() : Promise.reject(r.status))).then((html) => {
      pageHtml = html
      G.panes.forEach((P) => { if (P.holder) loadPage(P, false) })
    }).catch(() => {
      G.panes.forEach((P) => { if (P.holder) P.holder.innerHTML = '<p class="dk-empty" style="padding:20px">The page could not be loaded.</p>' })
    })
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start)
  else start()
})()
