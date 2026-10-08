// Scroll drives everything here. Each tall section has a sticky stage, and its
// progress (0 at the top, 1 when the stage is about to leave) picks what shows.
(function () {
  'use strict'

  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v))
  const smooth = (v) => { v = clamp(v, 0, 1); return v * v * (3 - 2 * v) }

  // ---------------------------------------------------------------- scroll
  let lenis = null
  if (!reduce && window.Lenis) {
    lenis = new window.Lenis({ lerp: 0.11, smoothWheel: true })
    document.addEventListener('click', (e) => {
      const a = e.target.closest('a[href^="#"]')
      if (!a) return
      const id = a.getAttribute('href')
      const to = id === '#top' ? 0 : document.querySelector(id)
      if (to === null) return
      e.preventDefault()
      lenis.scrollTo(to, { offset: 0, duration: 1.4 })
      if (id !== '#top') history.replaceState(null, '', id)
    })
  }

  function progress(section) {
    const r = section.getBoundingClientRect()
    const run = r.height - innerHeight
    return run > 0 ? clamp(-r.top / run, 0, 1) : (r.top < 0 ? 1 : 0)
  }
  const visible = (el) => { const r = el.getBoundingClientRect(); return r.bottom > -100 && r.top < innerHeight + 100 }

  // ------------------------------------------------------------- the chatter
  // What review sounds like when an agent writes the code: every line is about
  // approving work nobody read, or a decision nobody made on purpose.
  const CHATTER = [
    'merged 4,000 lines I did not read', 'the agent said it was fine', 'who approved this?',
    'it passed CI, so…', 'LGTM', 'the diff is 12k lines', 'nobody on the team wrote this',
    'reverted. again.', 'I am a PR approver now, not an engineer', 'what does this function even do',
    'the agent wrote 40 files while I got coffee', 'I skimmed it', 'wait, where did the retry go?',
    'looks good to me (did not run it)', 'what did the agent change in billing?', 'which file is the actual change?',
    'I asked why and got 900 words', 'see pricing.service.ts:42, cache.ts:17, pool.ts:9', 'the review was five paragraphs',
    'why are there three caches now', 'tab 37 of 41', 'approved from my phone', 'the agent rewrote the migration',
    'who decided to drop that index?', 'the summary says "minor refactor"', '1. (Recommended)',
    'what did we just ship?', 'I will read it later', 'it compiles, so…', 'can someone explain this PR to me?',
    'the agent fixed the test by changing the test', '20,000 lines, one approval', 'the PR description was longer than the code',
    'I trust the tests the agent wrote', 'nobody knows why this works', 'did anyone read the agent\'s plan?',
  ]

  function Sky(canvas) {
    const ctx = canvas.getContext('2d')
    let w = 0, h = 0, dpr = 1
    const rnd = (a, b) => a + Math.random() * (b - a)
    const spawn = (s, far) => {
      s.x = rnd(-1.7, 1.7); s.y = rnd(-1.05, 1.05); s.z = far ? rnd(0.9, 1.25) : rnd(0.12, 1.2)
      return s
    }
    const stars = CHATTER.concat(CHATTER.slice(0, 22)).map((text, i) => spawn({ text, hot: i % 9 === 4 }, false))
    const dots = Array.from({ length: 240 }, () => spawn({}, false))
    function size() {
      dpr = Math.min(2, devicePixelRatio || 1)
      w = canvas.clientWidth; h = canvas.clientHeight
      canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr)
    }
    size()
    // Sized from the element itself: Safari settles the stage's height (100svh)
    // after the first layout and fires no resize when it does, which left the
    // drawing buffer at the wrong height and the text stretched.
    new ResizeObserver(size).observe(canvas)
    let font = '"JetBrains Mono", ui-monospace, monospace'
    // quiet: how much of the middle to keep clear for the headline (0 to 1)
    function draw(dt, p, quiet) {
      const c = smooth((p - 0.46) / 0.3)            // the collapse
      const speed = reduce ? 0 : 0.045 + smooth(p / 0.5) * 0.55
      const F = Math.min(w, h) * 0.62, cx = w / 2, cy = h / 2
      if (canvas.width !== Math.round(canvas.clientWidth * dpr) || canvas.height !== Math.round(canvas.clientHeight * dpr)) size()
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      ctx.clearRect(0, 0, w, h)
      const pull = 1 - c
      for (const s of dots) {
        s.z -= speed * dt * 0.8
        if (s.z < 0.05) spawn(s, true)
        const x = cx + (s.x / s.z) * F * pull, y = cy + (s.y / s.z) * F * pull
        if (x < -10 || x > w + 10 || y < -10 || y > h + 10) continue
        const a = clamp((1.25 - s.z) * 0.7, 0, 0.8) * (1 - c * 0.7)
        ctx.fillStyle = 'rgba(235,219,178,' + a.toFixed(3) + ')'
        const r = clamp(1.4 / s.z, 0.6, 2.6)
        ctx.fillRect(x, y, r, r)
      }
      ctx.textAlign = 'center'; ctx.textBaseline = 'middle'
      for (const s of stars) {
        s.z -= speed * dt
        if (s.z < 0.08) spawn(s, true)
        const x = cx + (s.x / s.z) * F * pull, y = cy + (s.y / s.z) * F * pull
        const px = clamp(12 / s.z, 8, w < 700 ? 30 : 46) * (1 - c * 0.85)
        const near = s.z < 0.22 ? s.z / 0.22 : 1
        let a = clamp((1.22 - s.z) * 0.95, 0, 0.9) * near * (1 - c * 0.92)
        if (quiet > 0) {
          const e = ((x - cx) / (w * (w < 700 ? 0.72 : 0.4))) ** 2 + ((y - cy) / (h * (w < 700 ? 0.36 : 0.3))) ** 2
          if (e < 1) a *= 1 - quiet * (1 - e ** 4)
        }
        if (a < 0.02 || x < -400 || x > w + 400 || y < -80 || y > h + 80) continue
        ctx.font = px.toFixed(1) + 'px ' + font
        ctx.fillStyle = s.hot ? 'rgba(254,128,25,' + a.toFixed(3) + ')' : 'rgba(235,219,178,' + a.toFixed(3) + ')'
        ctx.fillText(s.text, x, y)
      }
      if (c > 0.02) {
        const g = ctx.createRadialGradient(cx, cy, 0, cx, cy, Math.min(w, h) * 0.35)
        g.addColorStop(0, 'rgba(254,128,25,' + (0.22 * c).toFixed(3) + ')')
        g.addColorStop(1, 'rgba(254,128,25,0)')
        ctx.fillStyle = g
        ctx.fillRect(0, 0, w, h)
      }
    }
    return { draw }
  }

  // ------------------------------------------------------- the question
  // The headline is a question, asked one way and then another, each time
  // written out the way a reply arrives in a chat: a word at a time.
  const QUESTIONS = [
    'Do you know what your agent just shipped?',
    'What did you just approve?',
    'Remember when you knew every line you shipped?',
    'Who made that decision, you or your agent?',
    'When did you stop reading the diff?',
    'Which of those 40 files did you open?',
    'You used to review code. Now you approve it?',
    'Could you explain your last merge?',
    'Would you have written it that way?',
    'Did you decide that, or did it just happen?',
    'When did “looks good” become the review?',
    'Could you debug it at 3am?',
    'Who on your team understands that change?',
    'Did you read the plan, or just the summary?',
    'What breaks if you revert it?',
    'When did you last read what your agent wrote?',
    'Why does it work? Do you know?',
    'What did you used to know about your own code?',
  ]
  // And the answer, asked back the same way when the chatter collapses.
  const ANSWERS = [
    'Make it show you.',
    'Make it point at the line.',
    'Make it draw what moves.',
    'Make it answer on the line.',
    'Make it wait for your call.',
    'Make it yours again.',
  ]
  // `lines` in turn, written into `el`; `shown` says whether anybody can see
  // it, and nothing moves while they cannot.
  function Ask(el, lines, shown) {
    if (reduce) return
    let at = 0, timers = []
    const later = (fn, ms) => timers.push(setTimeout(fn, ms))
    const quiet = () => document.hidden || !shown()
    function write(text) {
      timers.forEach(clearTimeout); timers = []
      el.classList.remove('out')
      el.textContent = ''
      const words = text.split(' ')
      const caret = document.createElement('span'); caret.className = 'caret typing'
      // The last two words are held together, so the question mark never
      // sits alone on the second line.
      const cut = Math.max(0, words.length - 2)
      const tail = document.createElement('span'); tail.style.whiteSpace = 'nowrap'
      const spans = words.map((w, i) => {
        const s = document.createElement('span'); s.className = 'w'; s.textContent = w
        if (i < cut) { el.appendChild(s); el.appendChild(document.createTextNode(' ')) }
        else { if (i > cut) tail.appendChild(document.createTextNode(' ')); tail.appendChild(s) }
        return s
      })
      el.appendChild(tail)
      el.appendChild(caret)
      let t = 280
      spans.forEach((s, i) => {
        later(() => { s.classList.add('in'); s.after(caret) }, t)
        t += 70 + Math.random() * 90 + (/[,?]$/.test(words[i]) ? 160 : 0)
      })
      later(() => caret.classList.remove('typing'), t)
      later(next, t + 3600)
    }
    function next() {
      if (quiet()) { later(next, 800); return }
      el.classList.add('out')
      later(() => { at = (at + 1) % lines.length; write(lines[at]) }, 480)
    }
    // Begin when it is first seen, so the opening line is not spent unseen.
    const begin = () => { if (quiet()) later(begin, 400); else write(lines[0]) }
    begin()
  }

  // ------------------------------------------------------------ step scenes
  function Steps(section) {
    const n = +section.dataset.steps
    const term = section.querySelector('.term')
    const roll = section.querySelector('.term-roll')
    const body = section.querySelector('.term-body')
    const bottom = section.dataset.scene === 'agent'
    const counts = section.querySelectorAll('[data-count]')
    let step = -1
    const words = (el) => (el.textContent.match(/\S+/g) || []).length
    const refs = (el) => Array.from(el.querySelectorAll('u')).filter((u) => /:\d/.test(u.textContent)).length

    function set(s) {
      if (s === step) return
      step = s
      section.querySelectorAll('[data-at]').forEach((e) => {
        const at = +e.dataset.at
        e.classList.toggle('on', e.parentElement.classList.contains('beats') ? at <= s : at === s)
        e.classList.toggle('now', at === s)
      })
      section.querySelectorAll('[data-show]').forEach((e) => e.classList.toggle('on', s >= +e.dataset.show))
      section.querySelectorAll('[data-hide]').forEach((e) => e.classList.toggle('off', s >= +e.dataset.hide))
      section.querySelectorAll('[data-only]').forEach((e) => e.classList.toggle('on', s === +e.dataset.only))
      section.querySelectorAll('.dots li').forEach((e, i) => e.classList.toggle('on', i <= s))
      if (counts.length) {
        const shown = Array.from(section.querySelectorAll('[data-words]')).filter((e) => s >= +e.dataset.show)
        const total = { words: shown.reduce((a, e) => a + words(e), 0), refs: shown.reduce((a, e) => a + refs(e), 0) }
        counts.forEach((c) => countTo(c, total[c.dataset.count]))
      }
      if (roll && body) requestAnimationFrame(() => scrollRoll(s))
    }

    function scrollRoll(s) {
      const shown = Array.from(roll.querySelectorAll(':scope > [data-show]')).filter((e) => +e.dataset.show === s)
      const style = getComputedStyle(body)
      const view = body.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom)
      const max = Math.max(0, roll.scrollHeight - view)
      let y = 0
      if (shown.length) {
        const first = shown[0], last = shown[shown.length - 1]
        y = bottom ? last.offsetTop + last.offsetHeight - view : first.offsetTop - 8
        if (!bottom && s === 1) y = 0
      }
      roll.style.transform = 'translateY(' + (-clamp(y, 0, max)) + 'px)'
    }

    addEventListener('resize', () => { const s = step; step = -1; set(s < 0 ? 0 : s) })
    return { update(p) { set(Math.min(n - 1, Math.floor(p * n * 0.999))) } }
  }

  function countTo(el, to) {
    const from = +el.dataset.v || 0
    el.dataset.v = to
    if (reduce || from === to) { el.textContent = to.toLocaleString('en'); return }
    const t0 = performance.now()
    const tick = (now) => {
      const k = Math.min(1, (now - t0) / 700)
      el.textContent = Math.round(from + (to - from) * smooth(k)).toLocaleString('en')
      if (k < 1 && +el.dataset.v === to) requestAnimationFrame(tick)
    }
    requestAnimationFrame(tick)
  }

  // -------------------------------------------------------------- the reel
  const FRAMES = 360
  const CUTS = [0, 30, 88, 138, 234, 306]                     // where each caption starts
  function Reel(section) {
    const canvas = section.querySelector('.reel-canvas')
    const ctx = canvas.getContext('2d')
    const bar = section.querySelector('.reel-load'), barFill = bar.querySelector('i')
    const set = (innerWidth < 900 || (navigator.connection && navigator.connection.saveData)) ? 's' : 'l'
    const imgs = new Array(FRAMES).fill(null)
    let want = 0, shown = -1, loaded = 0, started = false, cut = -1
    const url = (i) => 'frames/' + set + '/' + String(i).padStart(3, '0') + '.webp'

    function begin() {
      if (started) return
      started = true
      const order = [], seen = new Set()
      for (const stride of [24, 12, 6, 3, 1]) for (let i = 0; i < FRAMES; i += stride) if (!seen.has(i)) { seen.add(i); order.push(i) }
      let next = 0
      const pump = () => {
        if (next >= order.length) return
        const i = order[next++]
        const im = new Image()
        im.decoding = 'async'
        im.onload = () => { imgs[i] = im; loaded++; barFill.style.width = (loaded / FRAMES * 100) + '%'; if (loaded === FRAMES) bar.classList.add('done'); if (Math.abs(i - want) < Math.abs(shown - want) || shown < 0) draw(); pump() }
        im.onerror = () => { loaded++; pump() }
        im.src = url(i)
      }
      for (let k = 0; k < 6; k++) pump()
    }

    function nearest(i) {
      for (let d = 0; d < FRAMES; d++) {
        if (i - d >= 0 && imgs[i - d]) return i - d
        if (i + d < FRAMES && imgs[i + d]) return i + d
      }
      return -1
    }

    function fit() {
      const dpr = Math.min(2, devicePixelRatio || 1)
      canvas.width = Math.round(canvas.clientWidth * dpr)
      canvas.height = Math.round(canvas.clientHeight * dpr)
      shown = -1
      draw()
    }

    function draw() {
      const i = nearest(want)
      if (i < 0 || i === shown) return
      shown = i
      const im = imgs[i], W = canvas.width, H = canvas.height
      const k = Math.min(W / im.naturalWidth, H / im.naturalHeight)
      const dw = im.naturalWidth * k, dh = im.naturalHeight * k, x = (W - dw) / 2, y = (H - dh) / 2
      ctx.clearRect(0, 0, W, H)
      ctx.save()
      ctx.beginPath()
      const r = dw * 0.012
      if (ctx.roundRect) ctx.roundRect(x, y, dw, dh, r); else ctx.rect(x, y, dw, dh)
      ctx.clip()
      ctx.drawImage(im, x, y, dw, dh)
      ctx.restore()
    }

    function caption(c) {
      if (c === cut) return
      cut = c
      section.querySelectorAll('[data-at]').forEach((e) => {
        const at = +e.dataset.at
        e.classList.toggle('on', at === c)
        e.classList.toggle('done', at < c)
      })
    }

    new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) begin() }, { rootMargin: '150% 0px' }).observe(section)
    new ResizeObserver(fit).observe(canvas)
    caption(0)
    return {
      update(p) {
        want = Math.round(p * (FRAMES - 1))
        let c = 0
        for (let k = 0; k < CUTS.length; k++) if (want >= CUTS[k]) c = k
        caption(c)
        draw()
      },
    }
  }

  // ------------------------------------------------------------- the hero
  function Hero(section) {
    const sky = Sky(section.querySelector('.sky'))
    const copy = section.querySelector('.hero-copy'), signal = section.querySelector('.signal')
    const hush = section.querySelector('.hush'), cue = section.querySelector('.cue')
    const mark = section.querySelector('.signal-mark')
    const stream = section.querySelector('.hero-copy .stream')
    if (stream) Ask(stream, QUESTIONS, () => +(copy.style.opacity || 1) > 0.05)
    const reply = section.querySelector('.signal .stream')
    if (reply) Ask(reply, ANSWERS, () => +(signal.style.opacity || 0) > 0.5)
    let last = performance.now()
    return {
      update(p, now) {
        const dt = Math.min(0.05, (now - last) / 1000); last = now
        
        const out = 1 - smooth((p - 0.1) / 0.2)
        const inn = smooth((p - 0.7) / 0.18)
        if (visible(section)) sky.draw(dt, p, Math.max(out, inn))
        copy.style.opacity = out
        copy.style.transform = 'translateY(' + (-p * 60).toFixed(1) + 'px)'
        copy.style.visibility = out < 0.01 ? 'hidden' : ''
        signal.style.opacity = inn
        signal.style.pointerEvents = inn > 0.5 ? 'auto' : 'none'
        mark.style.transform = 'scale(' + (0.6 + 0.4 * inn).toFixed(3) + ')'
        hush.style.opacity = Math.max(out, inn * 0.9)
        cue.style.opacity = 1 - smooth(p / 0.08)
      },
    }
  }

  // --------------------------------------------------------------- install
  function tabs() {
    const list = document.querySelectorAll('.tabs [role="tab"]')
    const pick = (t) => {
      list.forEach((b) => {
        const on = b === t
        b.setAttribute('aria-selected', String(on))
        b.tabIndex = on ? 0 : -1
        document.getElementById(b.getAttribute('aria-controls')).hidden = !on
      })
    }
    list.forEach((b, i) => {
      b.addEventListener('click', () => pick(b))
      b.addEventListener('keydown', (e) => {
        if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return
        const t = list[(i + (e.key === 'ArrowRight' ? 1 : list.length - 1)) % list.length]
        pick(t); t.focus()
      })
    })
    if (/Win/.test(navigator.platform || navigator.userAgent)) pick(list[1])
    document.querySelectorAll('.copy').forEach((b) => b.addEventListener('click', async () => {
      const text = b.dataset.copy
      let ok = false
      try { await navigator.clipboard.writeText(text); ok = true } catch (_) {
        const t = document.createElement('textarea'); t.value = text; t.style.position = 'fixed'; t.style.opacity = '0'
        document.body.appendChild(t); t.select()
        try { ok = document.execCommand('copy') } catch (__) { ok = false }
        t.remove()
      }
      b.textContent = ok ? 'Copied' : 'Select it'
      setTimeout(() => { b.textContent = 'Copy' }, 1600)
    }))
  }

  // ------------------------------------------------------------------ run
  const scenes = []
  document.querySelectorAll('[data-scene]').forEach((s) => {
    const kind = s.dataset.scene
    const thing = kind === 'hero' ? Hero(s) : kind === 'reel' ? Reel(s) : Steps(s)
    scenes.push({ s, thing })
  })
  tabs()

  const nav = document.querySelector('.nav'), hero = document.querySelector('.hero')
  function frame(now) {
    if (lenis) lenis.raf(now)
    if (nav && hero) nav.classList.toggle('solid', hero.getBoundingClientRect().bottom < innerHeight * 0.6)
    for (const { s, thing } of scenes) {
      if (thing === scenes[0].thing || visible(s)) thing.update(progress(s), now)
    }
    requestAnimationFrame(frame)
  }
  requestAnimationFrame(frame)
})()
