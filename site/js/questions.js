(() => {
  const $ = s => document.querySelector(s);
  const wrap = $('#qs-wrap'), pane = $('.qs-pane'), textEl = $('.qs-text'), rowArea = $('.qs-row-area'), rowLabel = $('.qs-row-label'),
        win = $('#qs-win'), winBand = $('#qs-winBand'), winSide = $('#qs-winSide'), finale = $('#qs-finale');
  // The story, then the pull back into deck, then a held moment: the scene
  // stays pinned while the scroll carries a light across the last line.
  const STORY = 0.61, ZOOM0 = 0.63, ZOOM1 = 0.755, LIT = 0.78;
  const stage = $('#qs-stage'), section = $('#qs-oc'), fly = $('#qs-fly'), rowEl = $('#qs-row'),
        code = $('#qs-code'), band = $('#qs-band'), cursor = $('#qs-cursor');
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const mobileMQ = matchMedia('(max-width:700px)');

  const EZ = t => (1 - Math.cos(Math.PI * t)) / 2;          // deck's calm ease
  const EO = t => 1 - Math.pow(1 - t, 3);
  const cl = (v, a = 0, b = 1) => v < a ? a : v > b ? b : v;
  const seg = (p, a, b) => cl((p - a) / (b - a));
  const lerp = (a, b, t) => a + (b - a) * t;
  const rnd = i => { const x = Math.sin(i * 127.1 + 311.7) * 43758.5453; return x - Math.floor(x); };
  const hex = h => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16));
  const FG = hex('#ebdbb2');

  // ---------- content ----------
  const ANSWER = `<div class="qs-asked"><b>You asked</b><span>Are you sure the PDF is slow? Show me where.</span></div>
    <p>Yes. I timed it on staging: <code>renderReceipt</code> takes 3.9 of the 4.2 seconds, and it runs before the enqueue, inside the request. Line 42 is the whole wait.</p>
    <span class="qs-ref">service.ts:<i>42</i></span>`;
  $('#qs-answerD').innerHTML = ANSWER; $('#qs-answerM').innerHTML = ANSWER;

  const Q = {
    outbox:  { text: "What's an outbox, basics first?",            tok: 'qs-t-outbox',  line: 3,  hue: '#ebdbb2' },
    pdf:     { text: 'Are you sure the PDF is slow? Show me where.', tok: 'qs-t-render',  line: 8,  hue: '#8ec07c' },
    draw:    { text: 'Draw it: enqueue → worker → email',          tok: 'qs-t-enqueue', line: 9,  hue: '#8ec07c' },
    retry:   { text: 'Does the refund worker also retry forever?',   tok: 'qs-t-retry',   line: 14, hue: '#83a598' },
    worker:  { text: 'Why not render the PDF in the worker?' },
    invoice: { text: 'Does the invoice email render one too?' },
    cache:   { text: 'Would caching the template fix it?' },
  };
  const ROW_A = ['outbox', 'pdf', 'draw', 'retry'];
  const ROW_B = ['outbox', 'draw', 'worker', 'invoice', 'cache'];
  const LEAVE = ['pdf', 'retry'], ARRIVE = ['worker', 'invoice', 'cache'];
  const HINT = 'click to ask · ⌘-click to edit';

  // ---------- timeline (scroll progress 0..1) ----------
  const lines = [...code.querySelectorAll('.qs-ln')];
  const weights = lines.map(l => +l.dataset.w || 1);
  const R0 = 0.04, R1 = 0.65, total = weights.reduce((a, b) => a + b, 0);
  const lstart = []; { let acc = 0; weights.forEach((w, i) => { lstart[i] = R0 + acc / total * (R1 - R0); acc += w; }); lstart.push(R1); }
  const LIFT = 0.115;
  ROW_A.forEach(k => { Q[k].t0 = lstart[Q[k].line] + 0.006; });
  const T = { cursorIn: 0.675, cursorAt: 0.715, open: 0.712, press: 0.735, close: 0.752, ans: 0.755, renew: 0.80, renewEnd: 0.94 };
  const RENEW_MS = 2080;

  // ---------- build DOM ----------
  const pills = {};
  Object.entries(Q).forEach(([key, q]) => {
    const el = document.createElement('div');
    el.className = 'qs-pill'; el.setAttribute('role', 'listitem');
    const qs = document.createElement('span'); qs.className = 'qs-q';
    const words = q.text.split(' ');
    words.forEach((w, i) => {
      const s = document.createElement('span'); s.className = 'qs-w'; s.textContent = w; qs.appendChild(s);
      if (i < words.length - 1) qs.appendChild(document.createTextNode(' '));
    });
    const hint = document.createElement('span'); hint.className = 'qs-hint'; hint.textContent = HINT;
    el.append(qs, hint); rowEl.appendChild(el);
    pills[key] = { key, el, qs, hint, wordEls: [...qs.querySelectorAll('.qs-w')], words, open: 0, hoverT: 0, formed: false, anim: null, last: {} };
  });

  // deck's shelf arrows, for when the row hangs under the window
  const arrows = ['\u2039', '\u203a'].map(g => { const a = document.createElement('div'); a.className = 'qs-arrow'; a.setAttribute('aria-hidden', 'true'); a.textContent = g; rowEl.appendChild(a); return { el: a }; });

  // flyers + ghost letters for the four lifted questions
  ROW_A.forEach((key, qi) => {
    const q = Q[key], pl = pills[key];
    q.fly = pl.words.map((w, i) => {
      const s = document.createElement('span'); s.className = 'qs-fw'; s.textContent = w; fly.appendChild(s);
      const seed = qi * 31 + i * 7;
      return { el: s, fan: (rnd(seed) - 0.5) * 26, lift: 20 + rnd(seed + 1) * 16, rot: (rnd(seed + 2) - 0.5) * 22, wob: (rnd(seed + 3) - 0.5) * 8, hue: hex(q.hue), last: '' };
    });
    q.tokEl = document.getElementById(q.tok);
    const bd = document.createElement('div'); bd.className = 'qs-bd'; fly.insertBefore(bd, fly.firstChild); q.bd = bd; q.bdLast = {};
    const gh = document.createElement('div'); gh.className = 'qs-gh'; fly.appendChild(gh);
    q.ghost = gh; q.letters = [];
    // copy each character with the colour it has in the code
    const walk = (node) => {
      node.childNodes.forEach(n => {
        if (n.nodeType === 3) {
          const col = getComputedStyle(n.parentElement).color;
          [...n.textContent].forEach(ch => {
            const s = document.createElement('span'); s.textContent = ch; s.style.color = col; gh.appendChild(s);
            const seed = qi * 97 + q.letters.length * 13;
            q.letters.push({ el: s, dy: 14 + rnd(seed) * 18, dx: (rnd(seed + 1) - 0.5) * 10, rot: (rnd(seed + 2) - 0.5) * 34, last: '' });
          });
        } else walk(n);
      });
    };
    walk(q.tokEl);
  });

  // whispers: faint questions that come and go on other lines
  const whispers = [];
  lines.forEach((l, i) => {
    if (!l.dataset.whisper) return;
    const w = document.createElement('div'); w.className = 'qs-whisper'; w.textContent = l.dataset.whisper;
    code.appendChild(w); whispers.push({ el: w, line: i, last: '' });
  });

  // ---------- measure ----------
  let M = null, Z = null;
  const zst = {};
  function layout(order, width) {
    const pos = {}; let x = 0, y = 0;
    order.forEach(k => {
      const w = pills[k].w;
      if (x > 0 && x + w > width) { x = 0; y += 38; }
      pos[k] = { x, y }; x += w + 8;
    });
    return { pos, h: y + 30 };
  }
  function measure() {
    const hintText = mobileMQ.matches ? 'tap to ask' : HINT;
    Object.values(pills).forEach(p => { if (p.hint.textContent !== hintText) p.hint.textContent = hintText; });
    wrap.style.transform = 'none'; rowArea.style.transform = 'none'; Z = null;
    for (const k in zst) delete zst[k];   // what was drawn is gone; draw it all again
    Object.values(pills).forEach(p => {
      p.el.style.transform = 'none'; p.el.style.width = 'auto'; p.last = {};
    });
    const sr = stage.getBoundingClientRect();
    const rel = r => ({ x: r.left - sr.left, y: r.top - sr.top, w: r.width, h: r.height });
    Object.values(pills).forEach(p => {
      const pr = p.el.getBoundingClientRect();
      p.qw = p.qs.getBoundingClientRect().width;
      p.hw = p.hint.getBoundingClientRect().width + 10;
      p.w = Math.ceil(p.qw + 28);
      p.wordOff = p.wordEls.map(w => { const r = w.getBoundingClientRect(); return { x: r.left - pr.left, y: r.top - pr.top }; });
    });
    const rowW = rowEl.getBoundingClientRect().width;
    const A = layout(ROW_A, rowW), B = layout(ROW_B, rowW);
    rowEl.style.height = Math.max(A.h, B.h) + 'px';
    const row = rel(rowEl.getBoundingClientRect());
    const cr = code.getBoundingClientRect();
    const lt = lines.map(l => { const r = l.getBoundingClientRect(); return { y: r.top - cr.top, h: r.height }; });
    whispers.forEach(w => { w.el.style.top = lt[w.line].y + 'px'; });
    ROW_A.forEach(key => {
      const q = Q[key];
      const r = q.tokEl.getClientRects()[0] || q.tokEl.getBoundingClientRect();
      q.tr = rel(r);
      q.ghost.style.transform = `translate(${q.tr.x}px,${q.tr.y - (parseFloat(getComputedStyle(code).lineHeight) - r.height) / 2}px)`;
      const n = q.fly.length, pane = rel(code.getBoundingClientRect()), pl = pills[key];
      const phraseX = Math.max(pane.x + 8, Math.min(q.tr.x - 4, pane.x + pane.w - pl.qw - 14));
      const phraseY = q.tr.y + q.tr.h / 2 - 14 - Math.max(24, q.tr.h + 6);
      q.bd.style.width = (pl.qw + 20) + 'px';
      q.bd.style.transform = `translate(${phraseX - 10}px,${phraseY}px)`;
      q.fly.forEach((f, i) => {
        f.px = phraseX + pl.wordOff[i].x - pl.wordOff[0].x;
        f.py = phraseY;
        const ww = f.el.getBoundingClientRect().width;
        f.sx = q.tr.x + q.tr.w * (i + 0.5) / n - ww / 2;
        f.sy = q.tr.y + q.tr.h / 2 - 14;
        f.ex = row.x + A.pos[key].x + pills[key].wordOff[i].x;
        f.ey = row.y + A.pos[key].y + pills[key].wordOff[i].y;
      });
    });
    M = { stageW: sr.width, A: A.pos, B: B.pos, row, rowW, lt, sectionTop: section.getBoundingClientRect().top + scrollY, span: section.offsetHeight - innerHeight };

    // act 5: where the pane lands inside a deck window, and the window around it
    {
      const wr = rel(wrap.getBoundingClientRect()), pr = rel(pane.getBoundingClientRect());
      const mob = mobileMQ.matches, Hh = mob ? 92 : 112, Hf = 28, hang = 10;
      // the site's menu bar sits over the top of the stage; lay out below it
      const nav = (document.querySelector('header.nav') || {}).offsetHeight || 0;
      stage.style.setProperty('--qs-nav', nav + 'px');
      const title = finale.getBoundingClientRect().height + (mob ? 14 : 20);
      const rowH = Math.max(A.h, B.h), side = mob ? 0 : 0.72;
      const s = Math.min(mob ? 1 : 0.8,
        (Math.min(1080, sr.width - (mob ? 24 : 64))) / (pr.w * (1 + side)),
        (sr.height - nav - 40 - title - Hh - Hf - hang) / (pr.h + rowH));
      const Ww = pr.w * s * (1 + side), Wh = Hh + pr.h * s + Hf;
      const top0 = nav + Math.max(8, (sr.height - nav - (title + Wh + hang + rowH * s)) / 2);
      const winX = (sr.width - Ww) / 2, winY = top0 + title;
      const paneT = { x: winX, y: winY + Hh };
      wrap.style.transformOrigin = `${pr.x - wr.x}px ${pr.y - wr.y}px`;
      const rowTop = rel(rowEl.getBoundingClientRect()).y;
      const extra = (winY + Wh + hang - paneT.y) / s - (rowTop - pr.y);
      Object.assign(win.style, { width: Ww + 'px', height: Wh + 'px', transformOrigin: '50% 50%' });
      winBand.style.height = Hh + 'px';
      Object.assign(winSide.style, { left: pr.w * s + 'px', width: (Ww - pr.w * s) + 'px', top: Hh + 'px', bottom: Hf + 'px' });
      finale.style.top = top0 + 'px';
      Z = { s, dx: paneT.x - pr.x, dy: paneT.y - pr.y, extra, winX, winY };
      // in deck the row is one line, as wide as the window: a shelf, not a wrap
      const avail = Ww / s;          // the window's width, in the row's own terms
      if (mob) { M.C = layout(ROW_B, avail).pos; M.Cclip = 1e9; M.arrowR = null; }
      else {
        const one = layout(ROW_B, 1e9).pos;
        M.C = {}; ROW_B.forEach(k => { M.C[k] = { x: one[k].x + 44, y: 0 }; });
        M.Cclip = avail - 44;        // pills slide under the right arrow, as on the shelf
        M.arrowR = avail - 36;
      }
    }
    dirty = true;
  }

  // ---------- render ----------
  const setT = (o, el, tr, op) => {
    if (o.tr !== tr) { el.style.transform = tr; o.tr = tr; }
    if (o.op !== op) { el.style.opacity = op; o.op = op; }
  };
  const shimmer = p => {
    if (p.anim) p.anim.cancel();
    p.anim = p.qs.animate([{ backgroundPosition: '100% 0' }, { backgroundPosition: '0% 0' }],
      { duration: 1800, easing: 'cubic-bezier(.45,0,.55,1)' });
  };
  const lineState = lines.map(() => ({}));
  const tokState = {};
  let prevP = -1;

  function render(pAll) {
    const z = reduce || !Z ? 0 : seg(pAll, ZOOM0, ZOOM1), e = EZ(z);
    renderStory(reduce ? 1 : Math.min(1, pAll / STORY), EZ(seg(z, 0.2, 0.85)));
    const sc = lerp(1, Z ? Z.s : 1, e);
    const wt = `translate3d(${(Z ? Z.dx * e : 0).toFixed(2)}px,${(Z ? Z.dy * e : 0).toFixed(2)}px,0) scale(${sc.toFixed(4)})`;
    if (zst.wt !== wt) { wrap.style.transform = wt; zst.wt = wt; }
    const rt = `translate3d(0,${(Z ? Z.extra * e : 0).toFixed(2)}px,0)`;
    if (zst.rt !== rt) { rowArea.style.transform = rt; zst.rt = rt; }
    const to = (1 - EZ(seg(z, 0, 0.35))).toFixed(3);
    if (zst.to !== to) { textEl.style.opacity = to; rowLabel.style.opacity = to; zst.to = to; }
    const w = EZ(seg(z, 0.3, 0.92));
    const wtr = `translate3d(${(Z ? Z.winX : 0).toFixed(2)}px,${(Z ? Z.winY : 0).toFixed(2)}px,0) scale(${lerp(1.035, 1, w).toFixed(4)})`;
    if (zst.wtr !== wtr) { win.style.transform = wtr; zst.wtr = wtr; }
    if (zst.wo !== w) { win.style.opacity = w.toFixed(3); zst.wo = w; }
    const f = EZ(seg(z, 0.62, 1));
    const ft = `translate3d(0,${(12 * (1 - f)).toFixed(2)}px,0)`;
    if (zst.ft !== ft) { finale.style.transform = ft; finale.style.opacity = f.toFixed(3); zst.ft = ft; }
    // then, with the window settled and the line fully there, a light crosses it
    // once at its own slow pace, however fast the page is scrolled; scrolling
    // back above the zoom lets it cross again next time
    const h3 = finale.lastElementChild, lit = pAll >= LIT ? 1 : pAll < ZOOM0 ? 0 : zst.lit;
    if (zst.lit !== lit) { h3.classList.toggle('qs-shine', !!lit); zst.lit = lit; }
  }

  function renderStory(p, shelf = 0) {
    const mobile = mobileMQ.matches;

    // ---- reading head ----
    let head = -1, bandOp = seg(p, R0 - 0.012, R0 + 0.004);
    if (p >= R0) {
      head = lines.length - 1;
      for (let i = 0; i < lines.length; i++) {
        if (p < lstart[i + 1]) { const u = seg(p, lstart[i], lstart[i + 1]); head = i - 1 + EZ(cl(u / 0.32)); break; }
      }
    }
    // act 4: band returns to line 42
    const back = EZ(seg(p, T.ans, T.ans + 0.04));
    const bandPos = lerp(Math.max(head, 0), 8, back);
    const read = seg(p, R1, R1 + 0.04), focus = EZ(seg(p, T.ans, T.ans + 0.04));
    lines.forEach((l, j) => {
      const d = head - j;
      let o = d < -1 ? 0.3 : d <= 0 ? lerp(0.3, 1, d + 1) : d <= 1 ? lerp(1, 0.62, d) : 0.62;
      if (p < R0) o = 0.3;
      o = lerp(o, 0.8, read);
      o = lerp(o, j === 8 ? 1 : 0.4, focus);
      o = Math.round(o * 1000) / 1000;
      if (lineState[j].op !== o) { l.style.opacity = o; lineState[j].op = o; }
    });
    {
      const i0 = Math.floor(bandPos), i1 = Math.min(i0 + 1, lines.length - 1), f = bandPos - i0;
      const a = M.lt[i0], b = M.lt[i1];
      const y = lerp(a.y, b.y, f), h = lerp(a.h, b.h, f);
      setT(band, band, `translate3d(0,${y.toFixed(2)}px,0) scaleY(${(h / 100).toFixed(4)})`, bandOp.toFixed(3));
    }
    // whispers
    whispers.forEach(w => {
      const a = lstart[w.line], b = lstart[w.line + 1], u = seg(p, a - 0.004, b + 0.03);
      const o = Math.sin(Math.PI * u) * 0.8;
      setT(w, w.el, `translate3d(0,${(4 - 8 * u).toFixed(2)}px,0)`, o.toFixed(3));
    });

    // ---- lifts: letters peel, words drift down, pills form ----
    ROW_A.forEach(key => {
      const q = Q[key], pl = pills[key];
      const lt = seg(p, q.t0, q.t0 + LIFT);
      q.lt = lt;
      // token: glows, thins as its letters leave, then settles
      const glow = lt <= 0 ? 0 : lt < 0.18 ? EZ(lt / 0.18) : lerp(1, 0.32, EZ(seg(lt, 0.18, 0.7)));
      let g = glow * (1 - seg(p, T.ans - 0.01, T.ans + 0.03)) + (key === 'pdf' ? 0.55 * seg(p, T.ans, T.ans + 0.04) : 0);
      const thin = lt > 0 && lt < 1 ? 1 - 0.6 * Math.sin(Math.PI * seg(lt, 0.05, 0.75)) : 1;
      const ts = tokState[key] || (tokState[key] = {});
      const bg = `rgba(254,128,25,${(0.13 * g).toFixed(3)})`;
      if (ts.bg !== bg) { q.tokEl.style.background = bg; q.tokEl.style.boxShadow = `0 0 0 2px ${bg}`; ts.bg = bg; }
      if (ts.op !== thin) { q.tokEl.style.opacity = thin; ts.op = thin; }

      // ghost letters peel off the token, left to right
      const n = q.letters.length, gt = seg(lt, 0.03, 0.45);
      q.letters.forEach((L, k) => {
        const a0 = (k / n) * 0.45, u = seg(gt, a0, a0 + 0.55);
        if (u <= 0 || u >= 1) { setT(L, L.el, 'none', 0); return; }
        const e = EO(u);
        const op = Math.min(1, u * 6) * (1 - EZ(seg(u, 0.35, 1)));
        setT(L, L.el, `translate3d(${(L.dx * e).toFixed(2)}px,${(-L.dy * e).toFixed(2)}px,0) rotate(${(L.rot * e).toFixed(2)}deg)`, op.toFixed(3));
      });

      // a soft dark bed under the phrase so it reads over the code
      { const bo = EZ(seg(lt, 0.14, 0.42)) * (1 - EZ(seg(lt, 0.6, 0.78)));
        const v = bo.toFixed(3); if (q.bdLast.op !== v) { q.bd.style.opacity = v; q.bdLast.op = v; } }
      // question words emerge from where the letters went, gather into a phrase
      // floating just above the line, then drift down into the pill
      const nw = q.fly.length;
      q.fly.forEach((f, i) => {
        const r = nw > 1 ? i / (nw - 1) : 0;
        const gt = seg(lt, 0.1 + r * 0.16, 0.36 + r * 0.16);      // gather
        const dt = seg(lt, 0.6 + r * 0.1, 0.9 + r * 0.1);         // drift
        if (gt <= 0 || lt >= 1) { setT(f, f.el, 'none', 0); return; }
        const a = EO(gt), b = EZ(dt);
        // gather: from a loose cluster on the token, rising in a small arc
        const gx = lerp(f.sx + f.fan, f.px, a), gy = lerp(f.sy - 4, f.py, a) - Math.sin(Math.PI * a) * f.lift * 0.6;
        // drift: a soft fall, each word a touch behind the one before
        const cx = lerp(gx, f.ex, 0.15), cy = lerp(gy, f.ey, 0.55) + 6;
        const x = (1 - b) * (1 - b) * gx + 2 * (1 - b) * b * cx + b * b * f.ex;
        const y = (1 - b) * (1 - b) * gy + 2 * (1 - b) * b * cy + b * b * f.ey;
        const rot = f.rot * (1 - a) + Math.sin(b * Math.PI) * f.wob;
        const sc = lerp(0.8, 1, a);
        const op = EZ(seg(gt, 0, 0.45));
        setT(f, f.el, `translate3d(${x.toFixed(2)}px,${y.toFixed(2)}px,0) rotate(${rot.toFixed(2)}deg) scale(${sc.toFixed(3)})`, op.toFixed(3));
        const c = f.hue.map((h, j) => Math.round(lerp(h, FG[j], EZ(seg(gt, 0.3, 1)))));
        const col = `rgb(${c})`;
        if (f.last !== col) { f.el.style.color = col; f.last = col; }
      });
    });

    // ---- pills ----
    const rt = seg(p, T.renew, T.renewEnd) * RENEW_MS;
    const renewing = p >= T.renew;
    const cursorOpen = EZ(seg(p, T.open, T.open + 0.012)) * (1 - EZ(seg(p, T.close, T.close + 0.012)));
    const asked = seg(p, T.press, T.press + 0.006) * (1 - seg(p, T.renew + 0.005, T.renew + 0.02));
    const pressK = Math.sin(Math.PI * seg(p, T.press, T.press + 0.016));
    const settled = p >= 0.955 || reduce;

    // open amounts (scripted + real hover)
    let maxOpen = 0, openKey = null;
    Object.values(pills).forEach(pl => {
      pl.curOpen = Math.max(pl.key === 'pdf' && !renewing ? cursorOpen : 0, settled ? pl.open : 0);
      if (pl.curOpen > maxOpen) { maxOpen = pl.curOpen; openKey = pl.key; }
    });

    Object.values(pills).forEach(pl => {
      const k = pl.key; let x = 0, y = 0, op = 0, sc = 1, formed = false, textOp = 0;
      if (Q[k].t0 !== undefined) {
        const lt = Q[k].lt, form = EZ(seg(lt, 0.66, 0.97));
        x = M.A[k].x; y = M.A[k].y; op = form; sc = 0.94 + 0.06 * form;
        formed = lt >= 1; textOp = formed ? 1 : 0;
      }
      if (renewing) {
        const li = LEAVE.indexOf(k), ai = ARRIVE.indexOf(k);
        if (li >= 0) {
          const raw = cl((rt - li * 120) / 1000), s = EZ(raw);
          op = 1 - EZ(cl(raw / 0.6)); sc = 1 - 0.14 * s; y += 10 * s;
        } else if (ai >= 0) {
          const start = 520 + 160 * ai, g = EZ(cl((rt - start - 160) / 1080));
          x = M.B[k].x + 38 * (1 - g); y = M.B[k].y; op = cl((rt - start - 160) / 700); op = EZ(op); sc = 1;
          formed = g >= 1; textOp = 1;
        } else if (M.A[k].y === M.B[k].y) {
          // stays on its line: only ever eases left
          const m = EZ(cl((rt - 350) / 840));
          x = lerp(M.A[k].x, M.B[k].x, m); y = M.A[k].y;
        } else {
          // changes line: fades where it was, then glides in leftward at its new place
          const o = EZ(cl((rt - 350) / 400)), g = EZ(cl((rt - 1000) / 1080));
          if (rt < 1000) { x = M.A[k].x - 10 * o; y = M.A[k].y; op = 1 - o; }
          else { x = M.B[k].x + 38 * (1 - g); y = M.B[k].y; op = EZ(cl((rt - 1000) / 700)); }
        }
      }
      if (shelf > 0 && M.C && M.C[k] && ROW_B.includes(k)) { x = lerp(x, M.C[k].x, shelf); y = lerp(y, M.C[k].y, shelf); }
      if (k === 'pdf') sc *= 1 - 0.035 * pressK;
      // shimmer once as it forms
      if (formed && !pl.formed && p >= prevP && !reduce && op > 0.5) shimmer(pl);
      if (!formed && pl.anim) { pl.anim.cancel(); pl.anim = null; }
      pl.formed = formed;

      const maxW = Math.max(pl.w, M.stageW - 16 - M.row.x - x);
      let w = Math.min(maxW, Math.round(pl.w + pl.hw * pl.curOpen));
      if (shelf > 0 && M.Cclip < 1e9 && ROW_B.includes(k)) {
        // on the shelf, a question that runs past the right arrow is cut beneath it
        const room = M.Cclip - x;
        if (room < 28) op *= 1 - shelf; else if (room < w) w = Math.round(lerp(w, room, shelf));
      }
      const tr = `translate3d(${x.toFixed(2)}px,${y.toFixed(2)}px,0) scale(${sc.toFixed(4)})`;
      setT(pl.last, pl.el, tr, op.toFixed(3));
      if (pl.last.w !== w) { pl.el.style.width = w + 'px'; pl.last.w = w; }
      const ho = pl.curOpen.toFixed(3);
      if (pl.last.ho !== ho) { pl.hint.style.opacity = ho; pl.last.ho = ho; }
      const dim = openKey && openKey !== k ? 1 - 0.42 * maxOpen : 1;
      const qo = (textOp * dim).toFixed(3);
      if (pl.last.qo !== qo) { pl.qs.style.opacity = qo; pl.last.qo = qo; }
      const z = pl.curOpen > 0.01 ? '3' : '1';
      if (pl.last.z !== z) { pl.el.style.zIndex = z; pl.last.z = z; }
      const bc = k === 'pdf' && asked > 0 ? `rgba(254,128,25,${(0.55 * asked).toFixed(3)})` : '';
      if (pl.last.bc !== bc) { pl.el.style.borderColor = bc; pl.last.bc = bc; }
      const gone = op < 0.05;
      const pe = settled && ROW_B.includes(k) && !gone ? 'auto' : 'none';
      if (pl.last.pe !== pe) { pl.el.style.pointerEvents = pe; pl.last.pe = pe; }
      if (pl.last.gone !== gone) { pl.el.setAttribute('aria-hidden', gone ? 'true' : 'false'); pl.last.gone = gone; }
    });

    // ---- shelf arrows ----
    arrows.forEach((a, i) => {
      const on = M.arrowR == null ? 0 : shelf;
      const x = i === 0 ? 0 : (M.arrowR || 0);
      setT(a, a.el, `translate3d(${x.toFixed(2)}px,0,0)`, on.toFixed(3));
    });

    // ---- cursor ----
    {
      const pl = pills.pdf, a = M.A.pdf;
      const tx = M.row.x + a.x + pl.w * 0.42, ty = M.row.y + a.y + 14;
      const m = EZ(seg(p, T.cursorIn, T.cursorAt));
      const x = lerp(tx + 70, tx, m), y = lerp(ty + 64, ty, m);
      const op = seg(p, T.cursorIn, T.cursorIn + 0.012) * (1 - seg(p, T.close + 0.004, T.close + 0.02));
      const s = 1 - 0.12 * pressK;
      setT(cursor, cursor, `translate3d(${x.toFixed(2)}px,${y.toFixed(2)}px,0) scale(${s.toFixed(3)})`, op.toFixed(3));
    }

    // ---- answer + labels ----
    const ans = EZ(seg(p, T.ans, T.ans + 0.045));
    const ansEl = mobile ? $('#qs-answerM') : $('#qs-answerD');
    const hideEl = mobile ? $('#qs-head') : $('#qs-sub');
    const other = mobile ? $('#qs-answerD') : $('#qs-answerM');
    other.style.opacity = 0;
    other.setAttribute('aria-hidden', 'true'); ansEl.removeAttribute('aria-hidden');
    ansEl.style.opacity = ans.toFixed(3);
    ansEl.style.transform = `translate3d(0,${(10 * (1 - ans)).toFixed(2)}px,0)`;
    hideEl.style.opacity = (1 - EZ(seg(p, T.ans - 0.012, T.ans + 0.02))).toFixed(3);
    (mobile ? $('#qs-sub') : $('#qs-head')).style.opacity = 1;
    const lab = EZ(seg(p, T.renew + 0.01, T.renew + 0.04));
    $('.qs-row-label .qs-l1').style.opacity = (1 - lab).toFixed(3);
    $('.qs-row-label .qs-l2').style.opacity = lab.toFixed(3);

    prevP = p;
  }

  // ---------- real hover (once the row has settled) ----------
  // a press takes you to the try-it deck below, where a question really is asked;
  // through a #play link, so the site's own anchor scrolling carries it
  const toPlay = () => { const a = document.createElement('a'); a.href = '#play'; document.body.appendChild(a); a.click(); a.remove(); };
  Object.values(pills).forEach(pl => {
    let tIn = 0, tOut = 0;
    pl.el.addEventListener('click', toPlay);
    pl.el.addEventListener('pointerenter', () => {
      clearTimeout(tOut);
      tIn = setTimeout(() => { Object.values(pills).forEach(o => o.target = 0); pl.target = 1; dirty = true; }, 150);
    });
    pl.el.addEventListener('pointerleave', () => {
      clearTimeout(tIn);
      tOut = setTimeout(() => { pl.target = 0; dirty = true; }, 150);
    });
  });

  // ---------- loop ----------
  let cur = 0, target = 0, dirty = true, visible = true, lastT = 0;
  const readTarget = () => M ? cl((scrollY - M.sectionTop) / Math.max(1, M.span)) : 0;
  addEventListener('scroll', () => { target = readTarget(); }, { passive: true });
  new IntersectionObserver(es => { visible = es[0].isIntersecting; }, { rootMargin: '100px' }).observe(section);

  function tick(now) {
    requestAnimationFrame(tick);
    const dt = Math.min(64, now - (lastT || now)); lastT = now;
    if (!visible || !M) return;
    let hovering = false;
    Object.values(pills).forEach(pl => {
      const t = pl.target || 0;
      if (Math.abs(pl.open - t) > 0.001) {
        pl.open += (t - pl.open) * (1 - Math.exp(-dt / 70)); hovering = true;
      } else pl.open = t;
    });
    if (!reduce) {
      const d = target - cur;
      if (Math.abs(d) > 0.00008) { cur += d * (1 - Math.exp(-dt / 90)); dirty = true; }
      else if (cur !== target) { cur = target; dirty = true; }
    }
    if (dirty || hovering) { render(reduce ? 1 : cur); dirty = false; }
  }

  let rz = 0;
  addEventListener('resize', () => { clearTimeout(rz); rz = setTimeout(() => { measure(); target = readTarget(); }, 120); });

  const start = () => {
    measure();
    const qp = new URLSearchParams(location.search).get('qsp');
    if (qp !== null) scrollTo(0, M.sectionTop + M.span * cl(parseFloat(qp) || 0));
    target = cur = readTarget();
    render(reduce ? 1 : cur);
    requestAnimationFrame(tick);
    // web fonts can land after first layout; measure again whenever the layout moves
    const again = () => { clearTimeout(rz); rz = setTimeout(() => { measure(); target = readTarget(); }, 60); };
    if ('ResizeObserver' in window) { const ro = new ResizeObserver(again); ro.observe($('#qs-wrap')); ro.observe(stage); }
    if (document.fonts) document.fonts.addEventListener('loadingdone', again);
  };
  (document.fonts && document.fonts.ready ? document.fonts.ready : Promise.resolve()).then(start);
})();

// The try-it deck's questions, behaving as deck's shelf does: a question opens
// after the pointer rests 150 ms, holds 150 ms after it leaves, lights once as
// it opens, and the rest of the row steps back. The arrows slide the row.
(() => {
  const shelf = document.querySelector('.shelf');
  if (!shelf) return;
  const row = shelf.querySelector('.shelf-row');
  const qs = [...row.querySelectorAll('.shelf-q')];
  const REST = 150, LEAVE = 150;
  let open = null, tIn = 0, tOut = 0;
  const show = q => {
    if (open === q) return;
    if (open) open.classList.remove('open', 'shine');
    open = q;
    q.classList.add('open');
    q.classList.remove('shine'); void q.offsetWidth; q.classList.add('shine');
    row.classList.add('lit');
  };
  const hide = () => {
    if (!open) return;
    open.classList.remove('open'); open = null;
    row.classList.remove('lit');
  };
  qs.forEach(q => {
    q.addEventListener('pointerenter', () => {
      clearTimeout(tOut); clearTimeout(tIn);
      // straight across from an open one, it hands over at once
      if (open) show(q); else tIn = setTimeout(() => show(q), REST);
    });
    q.addEventListener('pointerleave', () => { clearTimeout(tIn); tOut = setTimeout(hide, LEAVE); });
    q.addEventListener('focus', () => show(q));
    q.addEventListener('blur', hide);
  });
  const arrows = [...shelf.querySelectorAll('.shelf-arrow')];
  arrows.forEach(a => a.addEventListener('click', () => {
    if (a.getAttribute('aria-disabled') === 'true') return;
    const step = (qs[0] ? qs[0].offsetWidth + 8 : 200) * +a.dataset.dir;
    row.scrollBy({ left: step });
  }));
  // an arrow with nowhere to go steps back
  const ends = () => {
    const max = row.scrollWidth - row.clientWidth;
    arrows.forEach(a => a.setAttribute('aria-disabled', +a.dataset.dir < 0 ? row.scrollLeft <= 1 : row.scrollLeft >= max - 1));
  };
  row.addEventListener('scroll', ends, { passive: true });
  if ('ResizeObserver' in window) new ResizeObserver(ends).observe(row); else addEventListener('resize', ends);
  if (document.fonts) document.fonts.ready.then(ends);
  ends();
})();
