// The changelog, from the releases on GitHub. The workflow writes
// releases.json at build time from GitHub's own rendering of each release
// (which GitHub sanitises), so the notes, images included, read exactly as
// they do there.
(function () {
  'use strict'
  const list = document.querySelector('.log-list')
  const toggle = document.getElementById('rc')
  const candidate = (tag) => /-rc\.\d+$/.test(tag)
  const when = (iso) => new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric' })
  // "v0.1.4 — open straight away" reads as a heading without the tag, which
  // already sits beside it.
  const title = (r) => {
    const t = (r.name || r.tag).replace(/^v?\d+\.\d+\.\d+(-[\w.]+)?\s*[—-]\s*/, '') || r.tag
    return t.charAt(0).toUpperCase() + t.slice(1)
  }

  function render(releases) {
    const latest = releases.find((r) => !r.prerelease)
    list.textContent = ''
    for (const r of releases) {
      const item = document.createElement('article')
      item.className = 'rel'
      item.id = r.tag
      item.dataset.rc = candidate(r.tag) ? '1' : ''
      const meta = document.createElement('div'); meta.className = 'rel-meta'
      const tag = document.createElement('div'); tag.className = 'rel-tag'
      const link = document.createElement('a'); link.href = '#' + r.tag; link.textContent = r.tag
      tag.appendChild(link)
      const date = document.createElement('time'); date.className = 'rel-date'; date.dateTime = r.published; date.textContent = when(r.published)
      meta.append(tag, date)
      const label = r === latest ? 'Latest' : candidate(r.tag) ? 'Release candidate' : r.prerelease ? 'Early' : ''
      if (label) { const chip = document.createElement('span'); chip.className = 'rel-chip' + (r === latest ? ' latest' : ''); chip.textContent = label; meta.appendChild(chip) }
      const body = document.createElement('div'); body.className = 'rel-body'
      const h = document.createElement('h2'); h.className = 'rel-title'; h.textContent = title(r)
      const notes = document.createElement('div'); notes.className = 'notes'
      notes.innerHTML = r.html || '<p>No notes for this release.</p>'
      notes.querySelectorAll('script, style, iframe').forEach((e) => e.remove())
      notes.querySelectorAll('img').forEach((img) => { img.loading = 'lazy'; img.decoding = 'async' })
      notes.querySelectorAll('a[href]').forEach((a) => { a.target = '_blank'; a.rel = 'noopener noreferrer' })
      const more = document.createElement('p')
      const gh = document.createElement('a'); gh.href = r.url; gh.textContent = 'On GitHub'; gh.target = '_blank'; gh.rel = 'noopener noreferrer'
      more.className = 'rel-more'; more.appendChild(gh)
      body.append(h, notes, more)
      item.append(meta, body)
      list.appendChild(item)
    }
    filter()
    // A link to one release, or a release candidate, should land on it.
    const want = decodeURIComponent(location.hash.slice(1))
    if (want && document.getElementById(want)) {
      if (candidate(want) && !toggle.checked) { toggle.checked = true; filter() }
      document.getElementById(want).scrollIntoView()
    }
  }

  function filter() {
    list.querySelectorAll('.rel').forEach((r) => { r.hidden = !toggle.checked && r.dataset.rc === '1' })
  }
  toggle.addEventListener('change', filter)

  fetch('releases.json', { cache: 'no-cache' })
    .then((r) => { if (!r.ok) throw new Error(r.status); return r.json() })
    .then(render)
    .catch(() => {
      list.innerHTML = '<p class="log-note">The release notes could not be loaded here. They are all on <a href="https://github.com/henit-chobisa/deck/releases">GitHub</a>.</p>'
    })
})()
