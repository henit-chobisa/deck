// Copy buttons, and the table of contents following the section in view.
(function () {
  'use strict'
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
  const links = new Map()
  document.querySelectorAll('.toc a').forEach((a) => links.set(a.getAttribute('href').slice(1), a))
  const sections = Array.from(document.querySelectorAll('.doc section[id]'))
  function mark() {
    let current = sections[0]
    for (const s of sections) if (s.getBoundingClientRect().top < 140) current = s
    links.forEach((a, id) => a.classList.toggle('on', current && id === current.id))
  }
  addEventListener('scroll', mark, { passive: true })
  mark()
})()
