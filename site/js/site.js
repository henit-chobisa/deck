// The shell's one behaviour: the footer's Email copies the address, or selects it to copy by hand.
(() => {
  const mac = /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent)
  document.querySelectorAll('[data-email]').forEach((b) => {
    const say = b.querySelector('.foot-cs')
    let t = 0
    b.addEventListener('click', () => {
      const text = b.dataset.email
      const done = (msg) => { say.textContent = msg; clearTimeout(t); t = setTimeout(() => { say.textContent = '' }, 2400) }
      const fallback = () => done(text + ' · press ' + (mac ? '⌘C' : 'Ctrl+C'))
      try {
        if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(text).then(() => done('copied'), fallback)
        else fallback()
      } catch (e) { fallback() }
    })
  })
})()
