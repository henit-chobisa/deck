const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')
const { resolve } = require('node:path')
const { readFileSync } = require('node:fs')
const { chromium, webkit } = require('playwright')

const base = 'http://127.0.0.1:8765'
const server = spawn('python3', ['-m', 'http.server', '8765', '--bind', '127.0.0.1', '--directory', resolve(__dirname, '..')], { stdio: 'ignore' })
const pause = (ms) => new Promise((r) => setTimeout(r, ms))

function checkFrames() {
  for (const tier of ['l', 'h']) for (let i = 0; i < 360; i++) {
    const path = resolve(__dirname, '../frames/retina', tier, String(i).padStart(3, '0') + '.webp')
    const data = readFileSync(path)
    assert.equal(data.toString('ascii', 12, 16), 'VP8L', `${path}: export must be lossless WebP`)
    const bits = data.readUInt32LE(21)
    const width = (bits & 0x3fff) + 1, height = ((bits >>> 14) & 0x3fff) + 1
    const nativeWidth = i < 138 ? 2374 : 2326, nativeHeight = i < 138 ? 1896 : 1906
    assert.equal(width, tier === 'h' ? nativeWidth : 1200, `${path}: source pixels must not be downscaled`)
    assert.equal(height, tier === 'h' ? nativeHeight : Math.round(nativeHeight * 1200 / nativeWidth))
  }
  console.log('PASS 720 lossless frames at their expected resolutions')
}

async function fits(page, label) {
  const size = await page.evaluate(() => ({ width: innerWidth, content: document.documentElement.scrollWidth }))
  assert(size.content <= size.width, `${label}: horizontal overflow ${JSON.stringify(size)}`)
  const nav = await page.locator('.nav').evaluate((el) => ({ height: el.offsetHeight, width: el.scrollWidth }))
  assert(nav.height <= 70 && nav.width <= size.width, `${label}: navigation wraps or overflows`)
}

async function readingFlow(page) {
  for (const [id, n] of [['without', 4], ['agent', 6], ['window', 6]]) {
    const scene = page.locator('#' + id)
    assert(await scene.evaluate((el) => el.offsetHeight < 1000), `${id}: excessive scroll runway`)
    for (let i = 0; i < n; i++) {
      assert.equal(await scene.locator('output').textContent(), `${i + 1} of ${n}`, `${id}: selected step`)
      if (id !== 'agent') assert.equal(await scene.locator('h2 .on').getAttribute('data-at'), String(i))
      if (id === 'window') {
        const image = await scene.locator('.scene-position a').getAttribute('href')
        assert((await page.request.get(base + '/' + image)).ok(), 'Full-size recording frame must exist')
      } else {
        assert.equal(await scene.locator('.term-body').evaluate((el) => getComputedStyle(el).overflowY), 'auto', 'Long terminal answers must remain touch-scrollable')
      }
      if (i < n - 1) await scene.getByRole('button', { name: 'Next', exact: true }).click()
    }
    assert(await scene.getByRole('button', { name: 'Next', exact: true }).isDisabled())
    for (let i = n - 1; i > 0; i--) await scene.getByRole('button', { name: 'Previous' }).click()
    assert(await scene.getByRole('button', { name: 'Previous' }).isDisabled())
  }
}

async function check(engine, name, width, height, reducedMotion = 'no-preference') {
  const browser = await engine.launch()
  try {
    const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: width <= 430 ? 3 : 2, hasTouch: width <= 900, reducedMotion })
    const errors = [], frames = []
    page.on('request', (request) => { if (request.url().includes('/frames/')) frames.push(request.url()) })
    page.on('pageerror', (error) => errors.push(error.message))
    // External fonts and star counts must not determine a layout test's result.
    await page.route(/^https:\/\//, (route) => route.abort())
    await page.goto(base)
    await page.waitForSelector('.scene-controls', { state: 'attached' })
    await fits(page, name)
    if (reducedMotion === 'reduce') {
      await readingFlow(page)
      assert(await page.locator('.hero').evaluate((el) => el.offsetHeight < 1400), 'Hero must remain a reading section')
    } else {
      assert(await page.locator('.sky').isVisible(), 'Stars and background text must remain on phones too')
      const before = await page.locator('.sky').evaluate((c) => c.toDataURL())
      await page.waitForTimeout(100)
      assert.notEqual(await page.locator('.sky').evaluate((c) => c.toDataURL()), before, 'The star field must animate')
      for (const id of ['without', 'agent', 'window']) {
        await page.locator('#' + id).evaluate((el) => {
          const stage = el.querySelector('.stage')
          window.scrollTo(0, el.offsetTop + (el.offsetHeight - stage.offsetHeight) * .9)
        })
        await page.waitForTimeout(150)
        assert(await page.locator('#' + id + ' .scene-controls').isHidden())
        assert.equal(await page.locator('#' + id + ' [data-at].on').first().getAttribute('data-at'), id === 'without' ? '3' : id === 'window' ? '5' : '0')
        assert.equal(await page.locator('#' + id + ' .stage').evaluate((el) => getComputedStyle(el).position), 'sticky')
        if (width <= 900) {
          assert.equal(await page.locator('#' + id + ' .side').evaluate((el) => getComputedStyle(el).textAlign), 'left')
          if (height > 500) {
            const layout = await page.locator('#' + id + ' .stage').evaluate((el) => {
              const style = getComputedStyle(el), stage = el.getBoundingClientRect()
              const caption = el.querySelector('.side').getBoundingClientRect()
              const picture = el.querySelector('.term, .reel-frame').getBoundingClientRect()
              return {
                expected: stage.top + (stage.height + parseFloat(style.paddingTop) - parseFloat(style.paddingBottom)) / 2,
                actual: (caption.top + picture.bottom) / 2,
                gap: picture.top - caption.bottom,
              }
            })
            assert(Math.abs(layout.expected - layout.actual) < 2, `${id}: group must be vertically centered`)
            assert(Math.abs(layout.gap - 24) < 2, `${id}: caption-to-picture gap must stay compact`)
          }
        }
      }
    }
    if (width <= 900 || reducedMotion === 'reduce') {
      const initial = await page.locator('.dk-foot .grp').textContent()
      await page.locator('.dk-keys [data-key="n"]').click()
      assert.notEqual(await page.locator('.dk-foot .grp').textContent(), initial)
      await page.locator('.dk-keys [data-key="p"]').click()
      assert.equal(await page.locator('.dk-foot .grp').textContent(), initial)
      await page.locator('.dk-keys [data-key="c"]').click()
      assert(await page.locator('.dk-compose textarea').isVisible())
      if (width <= 860) assert.equal(await page.locator('.dk-compose textarea').evaluate((el) => getComputedStyle(el).fontSize), '16px', 'iOS must not zoom the composer')
      await page.locator('.dk-compose textarea').fill('Keep the queue bounded.')
      await page.locator('.dk-compose .later').click()
      assert.equal(await page.locator('.dk-foot .cnt').textContent(), '1 comment')
      await page.locator('.dk-keys [data-key="s"]').click()
      assert.equal(await page.locator('#loop-count').textContent(), '1 comment')
      await page.getByRole('tab', { name: 'Windows' }).click()
      assert(await page.locator('#pane-win').isVisible())
      assert(await page.locator('#pane-unix').isHidden())
    }
    assert(frames.length < 80, `${name}: must not preload all 360 Retina frames`)
    assert(frames.every((url) => url.includes('/frames/retina/')), `${name}: legacy low-resolution fallback unexpectedly used`)
    if (width === 390) {
      await page.setViewportSize({ width: 1200, height: 800 })
      await page.waitForTimeout(250)
      await page.setViewportSize({ width, height })
      await page.waitForTimeout(250)
      await fits(page, name + ' after resize')
      assert(await page.locator('#window .scene-controls').isHidden())
      assert.equal(await page.locator('#window .stage').evaluate((el) => getComputedStyle(el).position), 'sticky')
    }
    if (process.env.SCREENSHOTS) {
      await page.screenshot({ path: resolve(process.env.SCREENSHOTS, `${name}-page.png`), fullPage: true })
    }
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
    checkFrames()
    for (const [width, height] of [[320, 568], [390, 664], [430, 780], [844, 390], [844, 1500]]) {
      await check(webkit, `webkit-${width}x${height}`, width, height)
    }
    await check(webkit, 'webkit-desktop', 1440, 900)
    await check(chromium, 'chromium-desktop', 1440, 900)
    await check(webkit, 'webkit-reduced-motion', 1440, 900, 'reduce')
  } finally { server.kill() }
})().catch((error) => { console.error(error); process.exitCode = 1 })
