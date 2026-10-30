const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')
const { resolve } = require('node:path')
const { chromium, webkit } = require('playwright')

const base = 'http://127.0.0.1:8765'
const server = spawn('python3', ['-m', 'http.server', '8765', '--bind', '127.0.0.1', '--directory', resolve(__dirname, '..')], { stdio: 'ignore' })
const pause = (ms) => new Promise((r) => setTimeout(r, ms))
const GH = 'https://github.com/henit-chobisa/deck'
// The launch post. It goes live after this lands, so nothing here fetches it: an early 404 is expected.
const BLOG = 'https://henit.blog/posts/i-hand-agents-the-code-not-the-decisions/'

// The page sets scroll-behavior: smooth; a test jump must not animate.
const jump = (page, y) => page.evaluate((y) => window.scrollTo({ top: y, behavior: 'instant' }), y)
const topOf = (page, sel) => page.locator(sel).evaluate((el) => Math.round(el.getBoundingClientRect().top + scrollY))
const now = (page, dots) => page.evaluate((dots) => [...document.querySelectorAll(dots + ' i')].findIndex((d) => d.classList.contains('now')), dots)

async function fits(page, label) {
  const size = await page.evaluate(() => ({ width: innerWidth, content: document.documentElement.scrollWidth }))
  assert(size.content <= size.width, `${label}: horizontal overflow ${JSON.stringify(size)}`)
  const nav = await page.locator('.nav').evaluate((el) => ({ height: el.offsetHeight, width: el.scrollWidth }))
  assert(nav.height <= 70 && nav.width <= size.width, `${label}: navigation wraps or overflows`)
}

// Every section, at rest, fits the width: scroll the whole page and look.
async function fitsAll(page, label) {
  const height = await page.evaluate(() => document.documentElement.scrollHeight)
  const step = await page.evaluate(() => Math.round(innerHeight * 0.7))
  for (let y = 0; y < height; y += step) {
    await jump(page, y)
    await page.waitForTimeout(60)
  }
  await page.waitForTimeout(900)
  await fits(page, label + ' after scrolling')
}

async function links(page) {
  const nav = await page.locator('.nav a').evaluateAll((as) => as.map((a) => [a.textContent.trim() || a.getAttribute('aria-label'), a.getAttribute('href')]))
  assert.deepEqual(nav, [['deck', '#top'], ['Docs', '/docs/'], ['Changelog', '/changelog/'], ['Blog', BLOG], ['deck on GitHub', GH], ['Install', '#install']])
  assert.deepEqual(await page.locator('.hero .ctas a').evaluateAll((as) => as.map((a) => a.getAttribute('href'))), ['#install', GH])
  const tryIt = page.locator('.hero .try a')
  assert.equal(await tryIt.getAttribute('href'), '/play/', 'the hero has a quiet way to try deck in the browser')
  assert.equal((await tryIt.textContent()).replace(/\u00a0/g, ' '), 'or try it in your browser →')
  // on phones the hero keeps to its two buttons
  const phone = page.viewportSize().width <= 640
  assert.equal(await tryIt.isVisible(), !phone, phone ? 'the try link is hidden on phones' : 'the try link is visible')
  assert.deepEqual(await page.locator('.x7btns a').evaluateAll((as) => as.map((a) => a.getAttribute('href'))), ['#install', GH])
  const foot = await page.locator('.foot-cols a').evaluateAll((as) => as.map((a) => a.getAttribute('href')))
  assert.deepEqual(foot, ['/#install', '/play/', '/docs/', '/guides/', '/changelog/', GH + '/releases', '/telemetry/', GH, GH + '/blob/main/LICENSE', GH + '/blob/main/README.md', BLOG, '/#note'])
  // every in-page link has its target, and every local page answers
  for (const href of await page.locator('a[href]').evaluateAll((as) => as.map((a) => a.getAttribute('href')))) {
    if (href.startsWith('#') || href.startsWith('/#')) assert.equal(await page.locator(href.replace(/^\//, '')).count(), 1, `${href} must name exactly one element`)
    else if (href.startsWith('/')) assert((await page.request.get(base + href)).ok(), `${href} must exist`)
  }
  assert.equal(await page.locator('#note').evaluate((el) => el.tagName), 'FIGURE', '#note is Henit\'s note')
  assert.equal(await page.locator('#x3 .x3try a').getAttribute('href'), '/play/', 'one way to the full deck, under #x3')
  // the hero's quiet link takes a keyboard, shows its focus, and opens /play/
  if (phone) return
  await tryIt.focus()
  assert.notEqual(await tryIt.evaluate((a) => getComputedStyle(a).outlineStyle), 'none', 'the try link shows a focus ring')
  await Promise.all([page.waitForURL((u) => new URL(u).pathname === '/play/'), page.keyboard.press('Enter')])
  await page.goto(base)
  await page.waitForSelector('#reveal .w', { state: 'attached' })
}

async function install(page, engine) {
  const tab0 = page.locator('#x7t0'), tab1 = page.locator('#x7t1')
  assert.equal(await tab0.getAttribute('aria-selected'), 'true', 'macOS and Linux are picked off Windows')
  await tab1.click()
  assert.equal(await tab1.getAttribute('aria-selected'), 'true')
  assert.match(await page.locator('#x7cmdtxt').textContent(), /^irm https:\/\/raw\.githubusercontent\.com\/henit-chobisa\/deck\/main\/install \| iex$/)
  await tab1.press('ArrowLeft')
  assert.equal(await tab0.getAttribute('aria-selected'), 'true', 'arrow keys move between the tabs')
  assert(await tab0.evaluate((el) => el === document.activeElement), 'the picked tab takes focus')
  const cmd = await page.locator('#x7cmdtxt').textContent()
  assert.equal(cmd, 'curl -fsSL https://raw.githubusercontent.com/henit-chobisa/deck/main/install | sh')
  await page.locator('#x7copy').click()
  await page.waitForFunction(() => document.getElementById('x7copyl').textContent !== 'Copy')
  const label = await page.locator('#x7copyl').textContent()
  if (engine === 'chromium') {
    assert.equal(label, 'Copied')
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), cmd)
  } else assert.match(label, /^(Copied|Press (⌘C|Ctrl\+C))$/)
  for (const [button, status] of [['#x7mailcopy', '#x7mailst'], ['.foot-cl[data-email]', '.foot-cs']]) {
    await page.locator(button).scrollIntoViewIfNeeded()
    await page.locator(button).click()
    await page.waitForFunction((s) => document.querySelector(s).textContent.trim() !== '', status)
  }
}

async function windowsPick(browser) {
  const page = await browser.newPage({ userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36' })
  await page.route(/^https:\/\//, (route) => route.abort())
  await page.goto(base)
  assert.equal(await page.locator('#x7t1').getAttribute('aria-selected'), 'true', 'Windows picks the Windows tab')
  assert.match(await page.locator('#x7cmdtxt').textContent(), /^irm /)
  await page.close()
}

// Desktop: x3 and x4 are pinned, one stop per step. Space and Page Down must
// reach every step from the top of the page, in order, without a mouse.
async function pinnedByKeyboard(page) {
  await jump(page, 0)
  await page.locator('body').click({ position: { x: 5, y: 300 } })
  await page.waitForTimeout(300)
  const seen = { x3: [], x4: [] }
  const end = await topOf(page, '#x5')
  for (let i = 0; i < 60; i++) {
    await page.keyboard.press(i % 3 === 0 ? 'Space' : 'PageDown')
    await page.waitForTimeout(950)
    for (const [id, dots] of [['x3', '#dots'], ['x4', '#x4dots']]) {
      const k = await now(page, dots)
      if (k >= 0 && seen[id].at(-1) !== k) seen[id].push(k)
      // a pinned stage never sits under the sticky nav
      if (k >= 0 && await page.evaluate((id) => { const r = document.getElementById(id).getBoundingClientRect(); return r.top < 0 && r.bottom > innerHeight }, id)) {
        const gap = await page.evaluate((id) => Math.min(...[...document.querySelectorAll(`#${id} h2, #${id} .x3lede, #${id} .x4lede, #${id} .x3win, #${id} .x4win`)].map((e) => e.getBoundingClientRect().top)) - document.querySelector('.nav').getBoundingClientRect().bottom, id)
        assert(gap >= 0, `${id}: pinned content is ${-gap}px under the nav`)
      }
    }
    if (await page.evaluate(() => scrollY) > end) break
  }
  assert.deepEqual(seen.x3, [0, 1, 2, 3], 'x3: every sentence by keyboard ' + JSON.stringify(seen.x3))
  assert.deepEqual(seen.x4.filter((k) => k > 0), [1, 2, 3, 4], 'x4: every step by keyboard ' + JSON.stringify(seen.x4))
  // and back up with Shift+Space
  // on a tall screen the pinned stage is centred; its stops start --pinoff below the section top
  const x4 = await topOf(page, '#x4') - await page.locator('#x4').evaluate((el) => parseFloat(getComputedStyle(el).scrollMarginTop) || 0)
  // the step is 460 in the section's px (--u)
  await jump(page, x4 + 3 * 460 * await page.evaluate(() => deckU(document.getElementById('x4'))))
  await page.waitForTimeout(600)
  assert.equal(await now(page, '#x4dots'), 4)
  await page.keyboard.press('Shift+Space')
  await page.waitForTimeout(950)
  assert.equal(await now(page, '#x4dots'), 3, 'Shift+Space goes back one step')
}

// Phones and reduced motion: nothing is pinned; the buttons step instead.
async function steppedByButtons(page) {
  for (const [sec, button, dots] of [['#x3', '#next', '#dots'], ['#x4', '#x4next', '#x4dots']]) {
    await page.locator(button).scrollIntoViewIfNeeded()
    await page.waitForTimeout(300)
    const before = await now(page, dots)
    await page.locator(button).click()
    await page.waitForTimeout(400)
    assert.notEqual(await now(page, dots), before, `${sec}: ${button} moves a step`)
    assert.equal(await page.locator(sec + ' .snapk').first().evaluate((el) => getComputedStyle(el).display), 'none', `${sec}: no scroll stops`)
  }
}

async function features(page) {
  const summary = page.locator('#x5 summary').first(), details = page.locator('#x5 details').first()
  await summary.focus()
  await page.waitForTimeout(900)
  await page.keyboard.press('Enter')
  assert(await details.evaluate((d) => d.open), 'x5: Enter opens a feature')
  await page.keyboard.press('Space')
  assert(!(await details.evaluate((d) => d.open)), 'x5: Space closes it')
  for (const id of ['x3', 'x4', 'x5']) {
    // lands at the section's top, or where its pin starts on a tall screen (scroll-margin-top). On a
    // CI runner the page can still grow under a long smooth scroll and stop it short; a second click
    // is what a reader would do, and the failure says where it stopped.
    const landed = (id) => { const e = document.getElementById(id); return Math.abs(e.getBoundingClientRect().top - (parseFloat(getComputedStyle(e).scrollMarginTop) || 0)) < 80 }
    let ok = false
    for (let tries = 0; tries < 2 && !ok; tries++) {
      await page.locator(`#x6 .o2also a[href="#${id}"]`).click()
      ok = await page.waitForFunction(landed, id, { timeout: 5000 }).then(() => true, () => false)
    }
    const at = await page.evaluate((id) => { const e = document.getElementById(id); return { top: Math.round(e.getBoundingClientRect().top), y: Math.round(scrollY), height: document.documentElement.scrollHeight } }, id)
    assert(ok, `x6: the link to #${id} lands on it ${JSON.stringify(at)}`)
  }
}

// /play/: an intro, then full screen (or the whole window, when fullscreen is refused) onto a drawn
// desktop where the recorded deck plays in ten steps. On a desktop: every step by keys (n, and Enter on
// the bar's Open, which takes the focus) and by clicks (Next, Open, a question under the window). At every
// step: the scene, the rail, the honest line on screen, no overflow, the caption clear of the window, and
// nothing from a later step. Then out by Esc or the ✕, the focus back on Start. Phones, tablets and windows
// up to 1100px get a still and where to watch it instead.
const stepOf = (page) => page.locator('#desk').getAttribute('data-step').then(Number)

async function deskStep(page, label, n, reduce) {
  await page.waitForTimeout(reduce ? 150 : 600)
  // The deck window grows out of the bar when it opens; its box crosses the caption until that
  // settles, and a slow runner was still mid-way at 600ms. Measure the window at rest.
  await page.waitForFunction(() => document.getElementById('dw').getAnimations().length === 0, null, { timeout: 5000 })
  const where = `${label} /play/ step ${n}`
  const s = await page.evaluate(() => {
    const r = (e) => e.getBoundingClientRect(), dw = document.getElementById('dw'), cap = r(document.getElementById('cap')), honest = r(document.querySelector('.dhonest'))
    return {
      tw: document.getElementById('tw').classList.contains('on'), dw: dw.classList.contains('on'), end: !document.getElementById('endc').hidden,
      rail: document.querySelector('#pl-rail button[aria-current]').dataset.go, no: document.getElementById('pl-no').textContent,
      honest: honest.top >= 0 && honest.bottom <= innerHeight && /A recorded example: nothing here runs a model/.test(document.querySelector('.dhonest').textContent),
      clear: !dw.classList.contains('on') || r(dw).bottom <= cap.top + 1, next: r(document.getElementById('pl-next')).bottom <= innerHeight,
      msgs: document.querySelectorAll('#dk .dk-msg').length, brought: document.querySelectorAll('#dk .dk-pane.brought').length,
      decision: /Keep 1h TTL/.test(document.querySelector('#dk .dk-chat').textContent),
    }
  })
  assert.deepEqual([s.tw, s.dw, s.end], [n <= 3, n >= 4 && n <= 9, n === 10], `${where}: its window ${JSON.stringify(s)}`)
  assert.equal(s.rail, String(n - 1), `${where}: the rail marks it`)
  assert.equal(s.no, `${n} / 10`)
  assert(s.honest && s.clear && s.next, `${where}: the honest line, Next, and the caption clear of the window ${JSON.stringify(s)}`)
  if (n <= 7) assert.deepEqual([s.msgs, s.brought, s.decision], [0, 0, false], `${where}: shows a later step's thread or panes`)
  if (n === 8) assert.equal(s.decision, false, `${where}: shows step 9's decision`)
  if (n === 3) assert.match(await page.locator('#bar-c').textContent(), /^4 groups$/, `${where}: the bar counts the sealed deck`)
  if (n >= 4 && n <= 9) assert.match(await page.locator('#dk .dk-strip .grp').textContent(), new RegExp(`^group ${Math.min(n - 3, 4)} of 4$`))
  if (n === 9) await page.locator('#dt-file').waitFor({ state: 'attached', timeout: 9000 }).then(() => page.waitForFunction(() => !document.getElementById('dt-file').hidden, null, { timeout: 9000 }))
  await fits(page, where)
}

// A click where the element is, as a hand would: the desktop is fixed to the viewport, and WebKit in
// full screen can stall Playwright's scroll-into-view before a locator click.
async function hand(page, sel) {
  const b = await page.locator(sel).first().boundingBox()
  assert(b, `${sel}: on screen to click`)
  await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2)
}

async function playDesk(page, label, how, reduce) {
  await page.goto(base + '/play/')
  await page.waitForFunction(() => document.documentElement.classList.contains('pl-ready'))
  assert(await page.locator('#pi-start').isVisible(), `${label}: Start is offered on a desktop`)
  assert(!(await page.locator('.pi-phone').isVisible()), `${label}: no phone card on a desktop`)
  if (how === 'keys') { await page.locator('#pi-start').focus(); await page.keyboard.press('Enter') } else await page.locator('#pi-start').click()
  await page.waitForFunction(() => !document.getElementById('desk').hidden)
  const box = await page.evaluate(() => { const r = document.getElementById('desk').getBoundingClientRect(); return [r.left, r.top, r.width, r.height, innerWidth, innerHeight, document.getElementById('main').inert] })
  assert.deepEqual(box, [0, 0, box[4], box[5], box[4], box[5], true], `${label}: the desktop fills the screen and the page behind is inert`)
  await deskStep(page, label, 1, reduce)
  for (let want = 2; want <= 10; want++) {
    for (let tries = 0; await stepOf(page) < want; tries++) {
      assert(tries < 6, `${label} /play/ ${how}: reaches step ${want}`)
      const at = await stepOf(page), asked = await page.locator('#dk .dk-msg .rec').count() > 0
      if (how === 'keys') {
        if (at === 3 && await page.evaluate(() => document.activeElement.id === 'bar-open')) await page.keyboard.press('Enter')
        else await page.keyboard.press(tries % 2 ? 'ArrowRight' : 'n')
      } else if (at === 3) await hand(page, '#bar-open')
      else if (at === 8 && !asked) { await hand(page, '#dk-asks .ak[data-ask^="Does logout"]'); await page.waitForSelector('#dk .dk-msg .rec', { timeout: 5000 }) }
      else await hand(page, '#pl-next')
      await page.waitForTimeout(reduce ? 150 : 450)
    }
    await deskStep(page, label, want, reduce)
    if (want === 9) assert.match(await page.locator('#dk .dk-chat').textContent(), /never revokes the refresh token, so with a 30-day TTL a stolen token lives 30 days\./, `${label} /play/ ${how}: the recorded answer`)
  }
  assert.match(await page.locator('#dk .dk-chat').textContent(), /Keep 1h TTL, add revocation on logout instead/, `${label} /play/ ${how}: the decision is a comment`)
  for (const [sel, href] of [['#endc a.btn.solid', '/#install'], ['#endc a.btn.ghost', GH]]) assert.equal(await page.locator(sel).getAttribute('href'), href)
  if (how === 'keys') {
    for (let i = 0; i < 14 && await stepOf(page) > 1; i++) {
      await page.keyboard.press(i % 2 ? 'ArrowLeft' : 'p'); await page.waitForTimeout(160)
      const n = await stepOf(page)
      if (n <= 7) assert.equal(await page.locator('#dk .dk-msg').count() + await page.locator('#dk .dk-pane.brought').count(), 0, `${label} /play/ back at step ${n}: a later step's thread or panes`)
    }
    assert.equal(await stepOf(page), 1, `${label} /play/: p goes back to the start`)
    await page.keyboard.press('Escape')
  } else await hand(page, '#dx')
  await page.waitForFunction(() => document.getElementById('desk').hidden)
  await page.waitForTimeout(300) // fullscreen ends a beat after the desktop closes
  assert.deepEqual(await page.evaluate(() => [document.activeElement.id, !!document.fullscreenElement, document.getElementById('main').inert]), ['pi-start', false, false], `${label}: leaving gives the page back, focus on Start`)
}

async function playHands(page, label) {
  await page.goto(base + '/play/')
  await page.locator('#pi-start').click()
  await page.locator('#pl-rail button[data-go="3"]').click({ force: true })
  await page.waitForTimeout(600)
  await page.locator('#dk .dk-pane').first().locator('[data-ln]').nth(11).click()
  await page.evaluate(() => document.getElementById('desk').focus({ preventScroll: true }))
  await page.keyboard.press('c')
  await page.waitForFunction(() => document.activeElement === document.querySelector('#dk .dk-compose textarea'))
  assert.equal(await page.locator('#dk .dk-compose .on').textContent(), 'comment on session.ts:12', 'c comments on the held line')
  await page.keyboard.type('Is 30 days what product asked for?')
  await page.locator('#dk .dk-compose .later').click()
  assert.equal(await page.locator('#dk .dk-send').textContent(), '· 1 to send', 'Add to review keeps it')
  await page.locator('#pl-rail button[data-go="0"]').click({ force: true })
  await page.locator('#pl-auto').click()
  await page.waitForFunction(() => document.getElementById('desk').dataset.step === '2', null, { timeout: 9000 })
  await page.keyboard.press('Shift')
  assert.equal(await page.locator('#pl-auto').getAttribute('aria-pressed'), 'false', 'a key pauses Play it for me')
  const at = await stepOf(page)
  await page.waitForTimeout(3000)
  assert.equal(await stepOf(page), at, 'paused, it stays where it is')
  await page.locator('#dx').click()
}

async function playPhone(page, label) {
  await page.goto(base + '/play/')
  await page.waitForFunction(() => document.documentElement.classList.contains('pl-ready'))
  assert(await page.locator('.pi-phone').isVisible(), `${label}: the phone card`)
  assert(!(await page.locator('#pi-start').isVisible()), `${label}: no Start on a phone`)
  assert.match(await page.locator('.pi-phone').textContent(), /Best on a desktop\. Open trydeck\.dev\/play on your computer/)
  assert.deepEqual(await page.locator('.pi-phone a').evaluateAll((as) => as.map((a) => a.getAttribute('href'))), ['/#install', GH])
  assert(await page.locator('#desk').isHidden(), `${label}: no desktop on a phone`)
  await fits(page, label + ' /play/')
}

// #x4's questions: a click asks (any but the story's own is answered with what is true here: a recording),
// and ⌘ or Ctrl with a click puts the question in the comment box
async function asks(page, label) {
  const pill = page.locator('#x4row .x4pill[data-k="b"]')
  await pill.scrollIntoViewIfNeeded()
  // let a pinned section settle on its stop first: a new stop clears what was asked on the last one
  await page.waitForTimeout(1200)
  await pill.click()
  await page.waitForFunction(() => /a recording/.test(document.getElementById('x4th').textContent), null, { timeout: 4000 })
  assert.match(await page.locator('#x4th').textContent(), /Why not a Redis lock\?/, `${label}: a question is asked in the thread`)
  await page.locator('#x4row .x4pill[data-k="c"]').click({ modifiers: ['ControlOrMeta'] })
  assert.equal(await page.locator('#x4cmp').textContent(), 'Draw it: miss → once → db', `${label}: ⌘-click puts the question in the comment box`)
}

// a change of width (a zoom step is one) lays the page out again in place, and never reloads it
async function noReload(page, label, width, height) {
  await page.evaluate(() => { window.__kept = 1 })
  // down to a narrow desktop and back: WebKit once kept 4px of #x3's overflow from the narrow layout
  await page.setViewportSize({ width: Math.min(width - 140, 1100), height })
  await page.waitForTimeout(500)
  await page.setViewportSize({ width, height })
  await page.waitForTimeout(500)
  assert.equal(await page.evaluate(() => window.__kept), 1, `${label}: resizing reloads the page`)
  await fits(page, label + ' after a width change')
}

async function check(engine, name, width, height, reducedMotion = 'no-preference') {
  const browser = await engine.launch()
  try {
    const desk = width > 980 && reducedMotion !== 'reduce'
    const options = { viewport: { width, height }, deviceScaleFactor: width <= 430 ? 3 : 2, hasTouch: width <= 900, reducedMotion }
    const context = await browser.newContext(options)
    if (engine === chromium) await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: base })
    const page = await context.newPage()
    const errors = []
    page.on('pageerror', (error) => errors.push(error.message))
    page.on('console', (m) => { if (m.type() === 'error' && !/Failed to load resource/.test(m.text())) errors.push(m.text()) })
    page.on('response', (r) => { if (r.url().startsWith(base) && r.status() >= 400) errors.push(r.status() + ' ' + r.url()) })
    // External fonts must not determine a layout test's result.
    await page.route(/^https:\/\//, (route) => route.abort())
    await page.goto(base)
    await page.waitForSelector('#reveal .w', { state: 'attached' })
    await fits(page, name)
    await links(page)
    if (desk) await pinnedByKeyboard(page)
    else await steppedByButtons(page)
    if (reducedMotion === 'reduce') {
      assert.equal(await now(page, '#x4dots') >= 0, true)
      for (const pen of ['#x7h .x7pn', '#x7bh .x7bpn', '#x4h .x4pn']) assert(await page.locator(pen).evaluate((el) => el.classList.contains('go')), `${pen}: drawn at once`)
    }
    await features(page)
    await asks(page, name)
    if (width > 900) await noReload(page, name, width, height)
    await install(page, engine === chromium ? 'chromium' : 'webkit')
    await fitsAll(page, name)
    if (width === 390) {
      await page.setViewportSize({ width: 1200, height: 800 })
      await page.waitForTimeout(400)
      await page.setViewportSize({ width, height })
      await page.waitForLoadState('load')
      await page.waitForTimeout(400)
      await fits(page, name + ' after resize')
    }
    if (process.env.SCREENSHOTS) {
      await jump(page, 0)
      await page.screenshot({ path: resolve(process.env.SCREENSHOTS, `${name}-top.png`) })
    }
    if (name === 'webkit-desktop' || name === 'chromium-desktop') await windowsPick(browser)
    const touch = width <= 900, reduce = reducedMotion === 'reduce'
    if (width > 1100 && !touch) {
      await playDesk(page, name, 'keys', reduce)
      await playDesk(page, name, 'clicks', reduce)
      if (name === 'chromium-desktop' || name === 'webkit-desktop') await playHands(page, name)
    } else await playPhone(page, name)
    for (const path of ['/docs/', '/guides/', '/changelog/', '/telemetry/', '/works-with/', '/ai-code-review/', '/coderabbit-alternative/', '/404.html']) {
      await page.goto(base + path)
      await fits(page, name + path)
      if (path === '/changelog/') await page.waitForSelector('.rel', { state: 'attached', timeout: 5000 })
      assert.equal(await page.locator('.foot-cols a[href="/docs/"]').count(), 1, `${path}: the shared footer`)
      // the nav names Docs (on phones, as on the home page, it folds to the GitHub icon and Install,
      // and Docs is in the footer)
      const docs = page.locator('.nav a', { hasText: /^Docs$/ })
      assert.equal(await docs.count(), 1, `${path}: the nav has Docs`)
      if (width > 900) assert(await docs.isVisible(), `${path}: Docs is visible in the nav`)
      else assert(await docs.isVisible() || await page.locator('footer a', { hasText: /^Docs$/ }).count() >= 1, `${path}: Docs is in the nav or the footer`)
    }
    assert.deepEqual(errors, [], `${name}: browser errors`)
    console.log(`PASS ${name}`)
  } finally { await browser.close() }
}

;(async () => {
  try {
    for (let i = 0; ; i++) {
      try { if ((await fetch(base)).ok) break } catch (_) {}
      if (i === 30) throw new Error('Preview server did not start')
      await pause(100)
    }
    for (const [width, height] of [[320, 568], [390, 664], [430, 780], [844, 390], [844, 1500]]) {
      await check(webkit, `webkit-${width}x${height}`, width, height)
    }
    await check(webkit, 'webkit-desktop', 1440, 900)
    await check(chromium, 'chromium-desktop', 1440, 900)
    await check(webkit, 'webkit-1100x1700', 1100, 1700)
    await check(chromium, 'chromium-1100x1700', 1100, 1700)
    await check(webkit, 'webkit-reduced-motion', 1440, 900, 'reduce')
    await check(chromium, 'chromium-1280x800', 1280, 800)
    await check(webkit, 'webkit-1366x768', 1366, 768)
    await check(chromium, 'chromium-1366x768', 1366, 768)
    await check(webkit, 'webkit-1280x800', 1280, 800)
    await check(chromium, 'chromium-1920x1080', 1920, 1080)
    await check(webkit, 'webkit-1920x1080', 1920, 1080)
    await check(chromium, 'chromium-390x844', 390, 844)
    await check(chromium, 'chromium-reduced-motion-390', 390, 844, 'reduce')
  } finally { server.kill() }
})().catch((error) => { console.error(error); process.exitCode = 1 })
