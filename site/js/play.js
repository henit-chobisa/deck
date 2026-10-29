// /play/: the deck around the home page's windows. Groups one at a time, n / p / c and clicks,
// and the recorded answers to the suggested questions. landing.js runs the windows themselves.
(() => {
  const $ = (id) => document.getElementById(id)
  const views = [...document.querySelectorAll('.play-view')]
  const steps = [...document.querySelectorAll('.play-steps button')]
  const x3 = $('x3'), x4 = $('x4'), th = $('x4th')
  const cmp = $('play-cmp'), text = $('play-text'), note = $('play-cmp-note')
  const at = $('play-at'), prev = $('play-prev'), next = $('play-next')
  if (!views.length || !x3 || !x4 || !th) return
  let v = 0, claim = -1, talk = 0
  const seen = []

  const dot = (sel) => [...document.querySelectorAll(sel + ' i')].findIndex((d) => d.classList.contains('now'))
  const label = () => {
    at.textContent = v === 0 ? 'group 1 of 4 · the values'
      : v === 1 ? `group 2 of 4 · claim ${Math.max(claim, 0) + 1} of 4`
      : `group 2 of 4 · the conversation, ${talk + 1} of 5`
    prev.disabled = v === 0
  }
  function show(i, first) {
    v = i
    views.forEach((el, k) => { el.hidden = k !== i })
    steps.forEach((b, k) => b.toggleAttribute('aria-current', k === i))
    if (steps[i]) steps[i].setAttribute('aria-current', 'step')
    // the windows measure themselves: let them, now that this one is visible
    dispatchEvent(new Event('resize'))
    // a group starts at its first step the first time you reach it
    if (i === 1 && !seen[1]) claim = x3.deckTo(0)
    if (i === 2 && !seen[2]) talk = x4.deckTo(0)
    seen[i] = true
    if (i !== 2) cmp.hidden = true
    // keep the deck's top in view when the group changes
    const d = $('deck'), r = d.getBoundingClientRect()
    if (!first && (r.top < 0 || r.top > innerHeight * .6)) d.scrollIntoView({ block: 'start' })
    label()
  }
  function forward() {
    if (v === 0) return show(1)
    if (v === 1) { const r = x3.deckGo(+1); if (r < 0) return show(2); claim = r; return label() }
    const r = x4.deckGo(+1)
    if (r < 0) { document.querySelector('.play-end a').focus(); return }
    talk = r; label()
  }
  function back() {
    if (v === 2) {
      const r = x4.deckGo(-1)
      if (r >= 0) { talk = r; return label() }
      show(1); claim = x3.deckTo(3)
      return label()
    }
    if (v === 1) { const r = x3.deckGo(-1); if (r < 0) return show(0); claim = r; return label() }
  }
  const esc = (t) => t.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  function say(who, where, html, rec) {
    const ag = who === 'agent'
    const e = document.createElement('div')
    e.className = 'x4m show' + (ag ? ' ag' : '')
    e.innerHTML = `<div><div class="x4mi"><i class="x4dot ${ag ? 'ag' : ''}"></i><div><div class="x4meta"><span>${who} · ${where}${rec ? '<span class="rec">recorded</span>' : ''}</span><span>now</span></div><div class="x4txt">${html}</div></div></div></div>`
    th.append(e)
  }

  // the suggested questions the windows don't answer themselves, answered from the recording
  const A = {
    b: 'A Redis lock would make it one query across all eight servers. Every miss would then pay a round trip to Redis, and a server that dies holding the lock needs a timeout. <code>once()</code> needs nothing new, so I started there.',
    c: 'That is the flow in the group before: press <b>key expires, now</b> and a current runs it, from the miss through <code>once(key)</code> to one <code>db.users.find</code>.',
    d: 'Yes. They all wait on the same promise, so a two-second query makes all 1,000 wait two seconds. None of them adds load while it waits.',
    e: 'Every request waiting on that call gets the same error. The next miss after it starts a new call.',
    f: 'Yes: one query for all servers instead of one per server. It costs a round trip on every miss, so I left it out of this change and kept the confidence at Medium.',
    g: 'This recording stops at the flow. In deck, I would bring that as a page in the pane.'
  }
  x4.addEventListener('deck-ask', (e) => {
    const { k, q } = e.detail
    if (!A[k]) return
    say('you', 'asked', esc(q))
    say('agent', 'get-user.ts:19', A[k], true)
  })

  function openComment() {
    if (v !== 2) show(2)
    cmp.hidden = false
    note.textContent = ''
    text.focus()
  }
  cmp.addEventListener('submit', (e) => {
    e.preventDefault()
    const t = text.value.trim()
    if (!t) { text.focus(); return }
    say('you', 'get-user.ts:19', esc(t))
    say('agent', 'get-user.ts:19', 'This page only has the recorded answers. In deck, your comment goes back to your agent, pinned to these lines, and it answers here.', true)
    text.value = ''
    note.textContent = 'Kept on this page. Nothing was sent.'
  })
  text.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) cmp.requestSubmit()
    if (e.key === 'Escape') { cmp.hidden = true; $('deck').focus() }
  })

  addEventListener('keydown', (e) => {
    if (e.altKey || e.ctrlKey || e.metaKey || e.defaultPrevented) return
    const a = document.activeElement
    if (a && a.matches('input,textarea,select,[contenteditable]')) return
    if (e.key === 'n') { e.preventDefault(); forward() }
    else if (e.key === 'p') { e.preventDefault(); back() }
    else if (e.key === 'c') { e.preventDefault(); openComment() }
  })
  steps.forEach((b, k) => b.addEventListener('click', () => show(k)))
  next.addEventListener('click', forward)
  prev.addEventListener('click', back)
  // the windows' own buttons step too; keep the counter with them
  // the windows' own buttons step too: read where they landed once their step has drawn
  const sync = () => setTimeout(() => { claim = dot('#dots'); talk = Math.max(0, dot('#x4dots')); label() }, 300)
  x3.querySelector('#next').addEventListener('click', sync)
  $('x4next').addEventListener('click', sync)
  x4.querySelectorAll('.x4dots i').forEach((d) => d.addEventListener('click', sync))
  show(0, true)
})()
