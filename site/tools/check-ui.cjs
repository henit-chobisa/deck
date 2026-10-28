const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')
const { resolve } = require('node:path')
const { chromium, webkit } = require('playwright')

const base = 'http://127.0.0.1:8765'
const server = spawn('python3', ['-m', 'http.server', '8765', '--bind', '127.0.0.1', '--directory', resolve(__dirname, '..')], { stdio: 'ignore' })
const pause = (ms) => new Promise((r) => setTimeout(r, ms))
const GH = 'https://github.com/henit-chobisa/deck'

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
  assert.deepEqual(nav, [['deck', '#top'], ['Docs', '/docs/'], ['Changelog', '/changelog/'], ['deck on GitHub', GH], ['Install', '#install']])
  assert.deepEqual(await page.locator('.hero .ctas a').evaluateAll((as) => as.map((a) => a.getAttribute('href'))), ['#install', GH])
  assert.deepEqual(await page.locator('.x7btns a').evaluateAll((as) => as.map((a) => a.getAttribute('href'))), ['#install', GH])
  const foot = await page.locator('.x7cols a').evaluateAll((as) => as.map((a) => a.getAttribute('href')))
  assert.deepEqual(foot, ['#install', '/docs/', '/changelog/', GH + '/releases', '/telemetry/', GH, GH + '/blob/main/LICENSE', GH + '/blob/main/README.md', 'https://henit.blog', '#note'])
  // every in-page link has its target, and every local page answers
  for (const href of await page.locator('a[href]').evaluateAll((as) => as.map((a) => a.getAttribute('href')))) {
    if (href.startsWith('#')) assert.equal(await page.locator(href).count(), 1, `${href} must name exactly one element`)
    else if (href.startsWith('/')) assert((await page.request.get(base + href)).ok(), `${href} must exist`)
  }
  assert.equal(await page.locator('#note').evaluate((el) => el.tagName), 'FIGURE', '#note is Henit\'s note')
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
  for (const [button, status] of [['#x7mailcopy', '#x7mailst'], ['#x7mail2', '#x7mail2s']]) {
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
    await page.locator(`#x6 .o2also a[href="#${id}"]`).click()
    await page.waitForFunction((id) => Math.abs(document.getElementById(id).getBoundingClientRect().top) < 80, id, { timeout: 4000 })
  }
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
    for (const path of ['/docs/', '/guides/', '/changelog/', '/ai-code-review/']) {
      await page.goto(base + path)
      await fits(page, name + path)
      assert(await page.locator('.nav').getByRole('link', { name: 'Docs', exact: true }).isVisible())
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
  } finally { server.kill() }
})().catch((error) => { console.error(error); process.exitCode = 1 })
