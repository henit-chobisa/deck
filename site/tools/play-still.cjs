// The still of /play/'s desktop that the intro and the phone card show: step 7 (the values), full
// window, at 1440x900. Run it with the site served on 8765 (cd site && python3 -m http.server 8765),
// then convert: python3 -c "from PIL import Image; i=Image.open('/tmp/play-still.png').convert('RGB'); i.save('site/assets/play-still.webp', quality=80, method=6); i.resize((800, 500), Image.LANCZOS).save('site/assets/play-still-800.webp', quality=80, method=6)"
const { chromium } = require('playwright')
;(async () => {
  const browser = await chromium.launch()
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' })
  await page.goto('http://127.0.0.1:8765/play/')
  await page.locator('#pi-start').click()
  await page.locator('#pl-rail button[data-go="6"]').click()
  await page.waitForTimeout(1200)
  // the picture, without the controls laid over it
  await page.evaluate(() => { document.getElementById('desk').classList.remove('chrome'); for (const id of ['cap', 'pl-rail', 'cursor']) document.getElementById(id).style.visibility = 'hidden' })
  await page.screenshot({ path: process.argv[2] || '/tmp/play-still.png' })
  await browser.close()
})()
