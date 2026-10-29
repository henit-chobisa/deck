// shared by the pinned sections (x3, x4): stops at k*step px after the pin starts; dots and keys go to a stop
// the page's px at an element (--u in landing.css): it differs by section on the desktop layout
window.deckU=el=>parseFloat(getComputedStyle(el||document.documentElement).getPropertyValue('--u'))||1;
// one rAF for every scroll handler on the page: each registers here rather than queueing its own
window.deckScroll=(()=>{const fns=[];let ticking=false;addEventListener('scroll',()=>{if(ticking)return;ticking=true;requestAnimationFrame(()=>{ticking=false;fns.forEach(f=>f())})},{passive:true});return f=>{fns.push(f)}})();
// the pen over #x3 and #x4: a loop round each thing a step names and an arrow to it from the narration's word.
// draw(i) lays step i out with loop, head and add; the marks follow the page for 2.6 s while it settles.
// base seeds the hand's wobble, so a step always draws the same; long is how wide a target is before it gets a box.
window.deckInk=(host,cls,base,draw)=>{const NS='http://www.w3.org/2000/svg',svg=document.createElementNS(NS,'svg');svg.setAttribute('class',cls);svg.setAttribute('aria-hidden','true');host.append(svg);
 let seed=base,keep=false,ki=0,at=-1,timer=0,live=0;const rnd=()=>{seed=(seed*9301+49297)%233280;return seed/233280-.5};
const loop=(x,y,w,h,box,long)=>{const p=box?-9:6,X=x-p,Y=y-p,W=w+2*p,H=h+2*p;
 if(box||W/H>long){return `M${X+W*.06} ${Y+rnd()*3} L${X+W+rnd()*4} ${Y+rnd()*3} Q${X+W+6} ${Y+H/2} ${X+W+rnd()*4} ${Y+H+rnd()*2} L${X+rnd()*4} ${Y+H+rnd()*3} Q${X-6} ${Y+H/2} ${X+rnd()*3} ${Y-2} L${X+W*.12} ${Y+3}`}
 const cx=X+W/2,cy=Y+H/2,rx=W/2+4,ry=H/2+3;
 return `M${cx+rx*.85} ${cy-ry*.75} C ${cx+rx*1.05} ${cy-ry*.1}, ${cx+rx*.7} ${cy+ry*1.05}, ${cx} ${cy+ry+rnd()*2} C ${cx-rx*.8} ${cy+ry*1.05}, ${cx-rx*1.08} ${cy+ry*.2}, ${cx-rx+rnd()*2} ${cy} C ${cx-rx*1.02} ${cy-ry*.8}, ${cx-rx*.3} ${cy-ry*1.12}, ${cx+rx*.25} ${cy-ry*1.02} C ${cx+rx*.6} ${cy-ry*.98}, ${cx+rx*.95} ${cy-ry*.7}, ${cx+rx*1.02} ${cy-ry*.25}`}
const head=(x1,y1,px,py)=>{const tx=x1-px,ty=y1-py,l=Math.hypot(tx,ty)||1,ux=tx/l,uy=ty/l,hx=-uy,hy=ux,s=9;return `M${x1-ux*s+hx*s*.6} ${y1-uy*s+hy*s*.6} L${x1} ${y1} L${x1-ux*s-hx*s*.6} ${y1-uy*s-hy*s*.6}`}
 const add=(d,c,delay)=>{if(keep){const e=svg.children[ki++];if(e)e.setAttribute('d',d);return}const pa=document.createElementNS(NS,'path');pa.setAttribute('d',d);if(c)pa.setAttribute('class',c);pa.setAttribute('pathLength','1');if(delay)pa.style.transitionDelay=delay+'ms';svg.append(pa);requestAnimationFrame(()=>requestAnimationFrame(()=>pa.classList.add('go')))};
 const paint=(i,k)=>{keep=!!k;ki=0;seed=base+i;if(!k)svg.innerHTML='';draw(i)};
 const follow=i=>{const t0=performance.now();cancelAnimationFrame(live);(function f(now){if(at!==i)return;paint(i,true);if(now-t0<2600)live=requestAnimationFrame(f)})(t0)};
 deckScroll(()=>{if(at>=0&&svg.children.length)paint(at,true)});addEventListener('resize',()=>{if(at>=0)paint(at)});
 return {svg,loop,head,add,show(i,ms){clearTimeout(timer);at=i;svg.innerHTML='';timer=setTimeout(()=>{paint(i);follow(i)},ms)}}};
window.deckPin=(()=>{const reduce=matchMedia('(prefers-reduced-motion: reduce)').matches;
 const add=sec=>{const S=+sec.dataset.step,n=+sec.dataset.stops;for(let k=0;k<n;k++){const a=document.createElement('i');a.className='snapk';a.setAttribute('aria-hidden','true');a.style.top=`calc(${k*S}*var(--u) - var(--pinoff,0px))`;sec.append(a)}};
 // on a tall screen the stage is centred, so the pin starts that far below the top: stop 0 is there
 const off=sec=>parseFloat(getComputedStyle(sec).scrollMarginTop)||0;
 const at=sec=>{const t=sec.getBoundingClientRect().top-off(sec),S=+sec.dataset.step*deckU(sec),n=+sec.dataset.stops;return t>2?-1:Math.max(0,Math.min(n-1,Math.round(-t/S)))};
 const go=(sec,k)=>{const S=+sec.dataset.step*deckU(sec);scrollTo({top:Math.round(scrollY+sec.getBoundingClientRect().top-off(sec)+k*S),behavior:reduce?'auto':'smooth'})};
 addEventListener('keydown',e=>{if(e.altKey||e.ctrlKey||e.metaKey||e.defaultPrevented)return;const sp=e.key===' ';
  const dn=e.key==='PageDown'||(sp&&!e.shiftKey),up=e.key==='PageUp'||(sp&&e.shiftKey);if(!dn&&!up)return;
  const a=document.activeElement;if(sp&&a&&a.matches('input,textarea,select,button,summary,a[href],[role=tab],[contenteditable]'))return;
  const y=scrollY,page=innerHeight*.875+4,ts=[...document.querySelectorAll('.snapk')].filter(s=>s.offsetParent).map(s=>Math.round(s.getBoundingClientRect().top+y));
  const c=dn?ts.filter(t=>t>y+4&&t<=y+page):ts.filter(t=>t<y-4&&t>=y-page);if(!c.length)return;
  e.preventDefault();scrollTo({top:dn?Math.min(...c):Math.max(...c),behavior:reduce?'auto':'smooth'})});
 // proximity, forwards only: when scrolling rests just short of the next stop (within a third of a step), finish the move to it
 let ly=scrollY,dir=0,rt=0,busy=0;
 const settle=()=>{if(busy||reduce)return;const y=scrollY;for(const sec of document.querySelectorAll('[data-stops]')){const as=[...sec.querySelectorAll('.snapk')].filter(s=>s.offsetParent);if(!as.length)continue;
   const S=+sec.dataset.step*deckU(sec),ts=as.map(s=>Math.round(s.getBoundingClientRect().top+y)),t=dir>0?ts.find(v=>v>y+3):[...ts].reverse().find(v=>v<y-3);
   if(t===undefined||y<ts[0]-S/3||y>ts[ts.length-1]+S/3)continue;if(Math.abs(t-y)<=S/3){busy=1;scrollTo({top:t,behavior:'smooth'});setTimeout(()=>{busy=0},700)}return}};
 addEventListener('scroll',()=>{const y=scrollY;if(y!==ly)dir=y>ly?1:-1;ly=y;clearTimeout(rt);rt=setTimeout(settle,160)},{passive:true});
 return {add,at,go,reduce}})();

(()=>{
const NS='http://www.w3.org/2000/svg',reduce=matchMedia('(prefers-reduced-motion: reduce)').matches,phone=()=>innerWidth<=980||reduce;
const ease=t=>t<.5?2*t*t:1-Math.pow(-2*t+2,2)/2;
const $=id=>document.getElementById(id),sec=$('x4');if(!sec)return;const st=sec.querySelector('.x4st');
const pen=(d,vb)=>`<svg viewBox="${vb}" preserveAspectRatio="none" aria-hidden="true"><path class="x4pn" pathLength="1" d="${d}"/></svg>`;
const draw=p=>{p.classList.remove('go');void p.getBoundingClientRect();requestAnimationFrame(()=>p.classList.add('go'))};
const undraw=p=>p&&p.classList.remove('go');
const mk=(tag,a,parent)=>{const e=document.createElementNS(NS,tag);Object.entries(a).forEach(([k,v])=>e.setAttribute(k,v));if(parent)parent.append(e);return e};
let timers=[];const later=(f,ms)=>{if(reduce||ms<=0){f();return}timers.push(setTimeout(f,ms))};const clearT=()=>{timers.forEach(clearTimeout);timers=[]};

// ---- code: the same function section 3 explained, line 19 is the one the thread is about
const CODE=[[9,'<span class="x4k">import</span> { cache } <span class="x4k">from</span> <span class="x4s">"../cache"</span>;'],[10,'<span class="x4k">import</span> { once } <span class="x4k">from</span> <span class="x4s">"../once"</span>;'],[11,'<span class="x4k">import</span> { db } <span class="x4k">from</span> <span class="x4s">"../db"</span>;'],[12,''],[13,'<span class="x4c">/** One lookup per key at a time, per process. */</span>'],
 [14,'<span class="x4k">export async function</span> <span class="x4f">getUser</span>(id: <span class="x4t">string</span>) {'],[15,'  <span class="x4k">const</span> key = <span class="x4s">`user:${id}`</span>;'],[16,'  <span class="x4k">const</span> hit = <span class="x4k">await</span> cache.<span class="x4f">get</span>(key);'],[17,'  <span class="x4k">if</span> (hit) <span class="x4k">return</span> hit;'],[18,''],
 ['19-','  <span class="x4k">const</span> user = <span class="x4k">await</span> db.users.<span class="x4f">find</span>(id);'],['19+','  <span class="x4k">const</span> user = <span class="x4k">await</span> <span class="x4f">once</span>(key, () =&gt; db.users.<span class="x4f">find</span>(id));'],[20,''],[21,'  <span class="x4k">await</span> cache.<span class="x4f">set</span>(key, user, { ttl: <span class="x4n">60</span> });'],[22,'  <span class="x4k">return</span> user;'],[23,'}'],[24,''],
 [25,'<span class="x4k">export async function</span> <span class="x4f">getUsers</span>(ids: <span class="x4t">string</span>[]) {'],[26,'  <span class="x4k">return</span> Promise.<span class="x4f">all</span>(ids.<span class="x4f">map</span>(getUser));'],[27,'}']];
const NEAR=['17','19-','19+','21'];
$('x4code').innerHTML=CODE.map(([n,t])=>{const s=String(n),d=s.endsWith('-'),a=s.endsWith('+');return `<div class="x4ln${d?' del':''}${a?' add lit':''}${NEAR.includes(s)?' near':''}"><span class="m">${d?'-':a?'+':''}</span><span class="g">${parseInt(s)}</span><span>${t}</span></div>`}).join('');

// ---- flow: eight servers, each through its own once(key), into one database
const svg=$('x4svg'),ndS=[],egS=[];const DBX=210,DBY=392;
// eight servers in two columns; each joins one spine down the middle, which runs into the database
mk('path',{class:'x4spine',d:`M${DBX} 30 V ${DBY-19}`},svg);
for(let i=0;i<8;i++){const col=i%2,row=Math.floor(i/2),x=col?236:24,y=10+row*76,g=mk('g',{class:'x4nd'},svg);mk('rect',{x,y,width:160,height:54,rx:7},g);
 const t=mk('text',{x:x+80,y:y+21,class:'c'},g);t.textContent='server '+(i+1);const o=mk('text',{x:x+80,y:y+39,class:'o c'},g);o.textContent='once(key)';ndS.push(g);
 const cy=y+27,ex=col?236:184,k=col?-1:1;
 egS.push(mk('path',{class:'x4eg',d:`M${ex} ${cy} H ${DBX-k*8} Q ${DBX} ${cy} ${DBX} ${cy+8} V ${DBY-19}`},svg))}
const db=mk('g',{class:'x4nd db'},svg);mk('rect',{x:DBX-80,y:DBY-18,width:160,height:36,rx:6},db);const dt=mk('text',{x:DBX,y:DBY},db);dt.textContent='db.users.find';
const was=mk('text',{x:DBX,y:486,class:'x4was'},svg);was.textContent='1,000';
const strikeG=mk('g',{},svg);strikeG.innerHTML=`<path class="x4pn" pathLength="1" d="M${DBX-36} 480 C ${DBX-20} 477, ${DBX+10} 482, ${DBX+37} 475"/>`;const strikeP=strikeG.firstChild;
const big=mk('text',{x:DBX,y:566,class:'x4big'},svg);big.textContent='0';
const cap=mk('text',{x:DBX,y:604,class:'x4cap'},svg);cap.textContent='queries · one per server';
let curs=[];const stopCur=()=>{curs.forEach(j=>j.stop=true);curs=[];svg.querySelectorAll('.x4cur').forEach(c=>c.remove())};
function current(p,delay,done){if(reduce){p.classList.add('lit');done();return}const c=mk('circle',{r:4,class:'x4cur'},svg);c.style.opacity=0;const job={stop:false};curs.push(job);let t0=null;const L=p.getTotalLength();
 (function f(ts){if(job.stop)return;if(t0===null)t0=ts+delay;const q=(ts-t0)/760;if(q<0){requestAnimationFrame(f);return}c.style.opacity=1;const pt=p.getPointAtLength(ease(Math.min(1,q))*L);c.setAttribute('cx',pt.x);c.setAttribute('cy',pt.y);p.classList.add('lit');
  if(q>=1){c.remove();done();return}requestAnimationFrame(f)})(performance.now())}
function flow(on,play){stopCur();let n=0;
 if(!on){ndS.forEach(g=>g.classList.remove('on','lit'));db.classList.remove('on','lit');egS.forEach(e=>e.classList.remove('on','lit'));big.textContent='0';undraw(strikeP);was.classList.remove('struck');return}
 db.classList.add('on');
 if(!play){ndS.forEach(g=>g.classList.add('on','lit'));db.classList.add('lit');egS.forEach(e=>e.classList.add('on','lit'));big.textContent='8';strikeP.classList.add('go');was.classList.add('struck');return}
 ndS.forEach((g,i)=>later(()=>g.classList.add('on'),760+i*60));egS.forEach((e,i)=>later(()=>e.classList.add('on'),900+i*60));
 later(()=>{was.classList.add('struck');draw(strikeP)},1150);
 egS.forEach((e,i)=>later(()=>{ndS[i].classList.add('lit');current(e,0,()=>{big.textContent=String(++n);db.classList.add('lit')})},1500+i*110))}

// ---- the thread
const M=[
 {at:0,who:'you',dot:'',where:'group 1',t:'0:05',txt:'does this only happen on hot keys?'},
 {at:0,who:'agent',dot:'ag',where:'group 1',t:'0:09',txt:'Any key, the second it expires. Hot keys just make it loud.'},
 {at:0,who:'you',dot:'',where:'get-user.ts:17',t:'0:12',txt:'why not just a longer ttl?'},
 {at:0,who:'agent',dot:'ag',where:'get-user.ts:17',t:'0:18',txt:'A longer ttl moves the stampede to later. It doesn’t stop it.'},
 {at:1,who:'you',dot:'',where:'get-user.ts:19 · asked',t:'0:42',txt:'What happens with eight servers?'},
 {at:2,who:'agent',dot:'ag',where:'get-user.ts:19',t:'0:49',txt:'Each server has its own <code>once()</code>, so each asks once. Eight queries, not a thousand.<span class="x4brought">↳ brought a flow · what eight servers do</span>'},
 {at:3,who:'you',dot:'no',where:'get-user.ts:19',t:'1:20',txt:'Then the title’s wrong. It’s eight, not one.',typed:'Then the title’s wrong. It’s eight, not one.'},
 {at:4,who:'agent',dot:'ag',where:'get-user.ts:19',t:'1:26',txt:'Fair. One per server, eight in all. I’ll retitle it, and keep it Medium until there’s a shared lock.'}];
const th=$('x4th');
const msgEl=m=>{const e=document.createElement('div');e.className='x4m'+(m.dot==='ag'?' ag':'');e.innerHTML=`<div><div class="x4mi"><i class="x4dot ${m.dot}"></i><div><div class="x4meta"><span>${m.who} · ${m.where}</span><span>${m.t}</span></div><div class="x4txt">${m.txt}</div></div></div></div>`;
 e.querySelectorAll('u').forEach(u=>u.insertAdjacentHTML('beforeend',pen('M2 6 C 30 3, 60 8, 98 4.5','0 0 100 10')));th.append(e);return e};
M.forEach(m=>{m.el=msgEl(m)});
const cmp=$('x4cmp'),ckey=$('x4c');let typeJ=0;
function showMsg(i,on,anim){const m=M[i],e=m.el,p=e.querySelector('.x4pn');if(!on){e.classList.remove('show');undraw(p);return}
 if(!e.classList.contains('show')){e.classList.add('show');if(p)anim?later(()=>draw(p),420):p.classList.add('go')}}
function typeIn(text,done){const id=++typeJ;if(reduce){done();return}ckey.classList.add('down');setTimeout(()=>ckey.classList.remove('down'),300);
 cmp.classList.add('typing');cmp.textContent='';let k=0;const step=()=>{if(id!==typeJ)return;k++;cmp.textContent=text.slice(0,k);if(k<text.length)setTimeout(step,26);else setTimeout(()=>{if(id!==typeJ)return;cmp.classList.remove('typing');cmp.textContent='Write a comment · c';done()},420)};setTimeout(step,300)}
const resetCmp=()=>{typeJ++;cmp.classList.remove('typing');cmp.textContent='Write a comment · c';ckey.classList.remove('down')};

// ---- the questions, renewing after each answer (asked one sinks; new ones glide in at the front)
const Q={a:'What happens with eight servers?',b:'Why not a Redis lock?',c:'Draw it: miss → once → db',d:'Does a slow query hold all 1,000?',e:'What if the db call throws?',f:'Would a shared lock make it one?',g:'Show the eight queries on a page'};
const ROWS=[['a','b','c','d','e'],['a','b','c','d','e'],['f','b','c','d','e'],['f','b','c','d','e'],['g','f','b','c','d']];
const row=$('x4row'),pills={};row.addEventListener('focusin',e=>{const b=e.target.closest('.x4pill');if(b&&getComputedStyle(row).overflowX==='auto')b.scrollIntoView({block:'nearest',inline:'nearest'})});
const pill=k=>{if(pills[k])return pills[k];const b=document.createElement('button');b.type='button';b.className='x4pill';b.dataset.k=k;b.innerHTML=`<span>${Q[k]}</span><span class="h">click to ask · ⌘-click to edit</span>`;b.addEventListener('click',e=>ask(k,e.metaKey||e.ctrlKey));pills[k]=b;return b};
function setRow(s,anim){const want=ROWS[Math.min(s,4)],fo=row.contains(document.activeElement)?document.activeElement:null;
 Object.entries(pills).forEach(([k,b])=>{if(!want.includes(k)){if(anim&&b.isConnected&&!b.classList.contains('sink')){b.classList.add('sink');later(()=>{if(b.classList.contains('sink')){const had=b===document.activeElement;b.remove();if(had)refocus(null)}},460)}else if(!anim)b.remove()}});
 want.forEach((k,i)=>{const b=pill(k);b.classList.remove('sink','open','down');b.classList.toggle('asked',k==='a'&&s===1);b.classList.toggle('fresh',(k==='f'&&s>=2&&s<4)||(k==='g'&&s>=4));
  const ref=row.children[i];if(b.parentNode!==row){if(anim){b.classList.add('new');row.insertBefore(b,ref||null);void b.offsetWidth;later(()=>b.classList.remove('new'),i===0?150:0)}else row.insertBefore(b,ref||null)}});
 [...row.children].forEach(b=>{if(!want.includes(b.dataset.k)&&!b.classList.contains('sink'))b.remove()});
 // keep the wanted order
 want.forEach(k=>{const b=pills[k];if(b.isConnected&&!b.classList.contains('new'))row.append(b)});row.querySelectorAll('.sink').forEach(b=>row.prepend(b));if(fo&&document.activeElement!==fo)refocus(fo)}
function refocus(fo){const t=fo&&fo.isConnected&&!fo.classList.contains('sink')?fo:row.querySelector('.x4pill:not(.sink)');if(t)t.focus({preventScroll:true})}

// ---- the hand
const hand=$('x4hand');
function handTo(el){const S=st.getBoundingClientRect(),R=el.getBoundingClientRect();hand.style.transform=`translate(${(R.left-S.left+Math.min(R.width*.55,120*deckU(sec))).toFixed(0)}px,${(R.top-S.top+R.height*.55).toFixed(0)}px)`}
function handHome(){const S=st.getBoundingClientRect(),W=$('x4win').getBoundingClientRect();hand.style.transition='none';hand.style.transform=`translate(${(W.left-S.left+W.width*.32).toFixed(0)}px,${(W.bottom-S.top-60*deckU(sec)).toFixed(0)}px)`;void hand.offsetWidth;hand.style.transition=''}

// ---- narration, and what each line names, ringed at once
const T=['Every group ends with <em>the questions</em> you were about to ask.','Tap one and <em>it’s asked</em>. Nothing to type.','Your agent answers by bringing in <em>a pane</em>.','Not convinced? Press c and <em>push back</em> on the line.','It answers in the thread. <em>You decide</em> when it’s clear.'];
const HL=[['#x4row'],['#x4th .x4m:nth-child(5) .x4mi'],['#x4flow .x4ph b'],['#x4th .x4m:nth-child(7) .x4mi'],['#x4row .x4pill:first-child']];
const LINK=[[[0,0]],[[0,0]],[[0,0]],[[0,0]],[[0,0]]];
const narr=$('x4narr').querySelector('span');let nl=-1;
const I4=deckInk(st,'x4inkl',5,i=>drawInk(i)),ink=I4.svg,head=I4.head,loop=(x,y,w,h,box)=>I4.loop(x,y,w,h,box,3);
// one pen arrow per step, as section 3: a single smooth arc over the window, on a dark halo
function arrow(x0,y0,t,wr){
 // a question under the window: a short hook up from the word, never through the line
 if(t.top>wr.bottom-10){const tc=t.left+t.width/2,dir=tc>x0?1:-1,x1=Math.min(t.right-16,Math.max(t.left+16,x0+dir*Math.max(46,Math.min(120,Math.abs(tc-x0)*.5)))),y1=t.bottom+7;return [`M${x0} ${y0} Q ${x0} ${y1+2}, ${x1} ${y1}`,head(x1,y1,x0,y1+2)]}
 const tcx=t.left+t.width/2,x1=tcx+(x0<tcx?-1:1)*Math.min(t.width/2+10,40),y1=t.bottom+12;
 const mx=(x0+x1)/2,my=(y0+y1)/2,dx=x1-x0,dy=y1-y0,L=Math.hypot(dx,dy)||1,nx=-dy/L,ny=dx/L,side=x1>x0?-1:1,bow=Math.min(160,L*.28);
 const cx=mx+nx*bow*side,cy=my+ny*bow*side;
 return [`M${x0} ${y0} Q ${cx} ${cy}, ${x1} ${y1}`,head(x1,y1,cx,cy)]}
// to a message in the chat: run right through the empty space under the questions, then rise into the message's left side
// one smooth arc drawn on top of the deck, landing on the target's left side
function over(x0,y0,t){const x1=t.left-12,y1=t.top+Math.min(t.height/2,22),cx=x0+(x1-x0)*.78,cy=y0-(y0-y1)*.12;
 return [`M${x0} ${y0} Q ${cx} ${cy}, ${x1} ${y1}`,head(x1,y1,cx,cy)]}
function sweep(x0,y0,t,gy){const x1=t.left-12,y1=t.top+t.height/2;
 return [`M${x0} ${y0} C ${x0} ${gy}, ${x0+30} ${gy}, ${x0+60} ${gy} L ${x1-60} ${gy} C ${x1-18} ${gy}, ${x1-10} ${y1+40}, ${x1-8} ${y1+14} Q ${x1-6} ${y1}, ${x1} ${y1}`,head(x1,y1,x1-6,y1)]}
function seam(x0,y0,t,gx){const ty=t.top+t.height/2,x1=t.left-12;
 const d=`M${x0} ${y0} C ${x0} ${y0-46}, ${gx} ${y0-30}, ${gx} ${y0-90} L ${gx} ${ty+34} C ${gx} ${ty+8}, ${gx+8} ${ty}, ${x1} ${ty}`;return [d,head(x1,ty,gx+8,ty)]}
const addP=(d,delay,cls)=>I4.add(d,cls,delay);
function drawInk(i){if(phone())return;const W=st.getBoundingClientRect(),wb=$('x4win').getBoundingClientRect();ink.setAttribute('viewBox',`0 0 ${W.width} ${W.height}`);
 const wr={left:wb.left-W.left,right:wb.right-W.left,top:wb.top-W.top,bottom:wb.bottom-W.top,width:wb.width},targets=[];
 HL[i].forEach(q=>{const box=q.startsWith('box:'),e=document.querySelector(q.replace(/^box:/,''));if(!e)return;const b=e.getBoundingClientRect();if(!b.width)return;const k=box?9:0,t={left:b.left-W.left+k,top:b.top-W.top+k,width:b.width-2*k,height:b.height-2*k,right:b.right-W.left-k,bottom:b.bottom-W.top-k};targets.push(t);addP(loop(t.left-k,t.top-k,t.width+2*k,t.height+2*k,box))});
 const words=[...sec.querySelectorAll('.x4narr span em')];
 LINK[i].forEach(([wi,ti])=>{const w=words[wi],t=targets[ti];if(!w||!t)return;const a=w.getBoundingClientRect();(i>=1&&i<=3?over(a.left+a.width/2-W.left,a.top-6-W.top,t):arrow(a.left+a.width/2-W.left,a.top-6-W.top,t,wr)).forEach((d,k)=>{if(!k)addP(d,0,'halo');addP(d,k?420:0)})})}
const inkAt=(i,ms)=>I4.show(i,reduce?0:(i===2?Math.max(ms||0,1300):ms));
function say(i,ms){i=Math.min(i,4);if(i!==nl){nl=i;narr.classList.add('out');setTimeout(()=>{narr.innerHTML=T[i];narr.classList.remove('out')},reduce?0:180)}inkAt(i,ms)}

// ---- steps: 0 the questions · 1 tap · 2 the agent brings a pane · 3 push back · 4 the answer · 5 the caption
let cur=null;const dots=[...document.querySelectorAll('#x4dots i')];
function stage(i){if(i===cur)return;const prev=cur;cur=i;if(prev!==null&&Math.min(prev,4)===Math.min(i,4)){sec.classList.toggle('x4end',i>=5);return}const s=Math.min(i,4),fwd=prev!==null&&i===prev+1&&!reduce;
 clearT();clearExtra();resetCmp();hand.classList.remove('on','press');
 dots.forEach((d,k)=>{d.classList.toggle('on',k<=s);d.classList.toggle('now',k===s)});
 sec.classList.toggle('x4end',i>=5);if(!(prev!==null&&Math.min(prev,4)===s))say(s,fwd?[620,2100,1400,2600,2100][s]:620);
 // the thread, the pane, the pen on the title
 M.forEach((m,k)=>{if(m.at<s||(m.at===s&&!fwd))showMsg(k,m.at<=s,false);else if(m.at>s)showMsg(k,false)});
 sec.classList.toggle('x4b',s>=2);
 if(s<2)flow(false);else if(!(fwd&&s===2))flow(true,false);
 if(!fwd){setRow(s,prev!==null&&!reduce);return}
 if(s===1){setRow(0,false);const b=pills.a;if(!phone()){handHome();requestAnimationFrame(()=>{hand.classList.add('on');handTo(b)})}
  later(()=>b.classList.add('open'),phone()?0:760);later(()=>{b.classList.add('down');hand.classList.add('press')},phone()?420:1300);
  later(()=>{b.classList.remove('down','open');b.classList.add('asked');hand.classList.remove('press')},phone()?570:1450);
  later(()=>showMsg(4,true,true),phone()?640:1520);later(()=>hand.classList.remove('on'),2100)}
 if(s===2){showMsg(5,true,true);flow(true,true);later(()=>setRow(2,true),1900)}
 if(s===3){setRow(3,false);typeIn(M[6].typed,()=>showMsg(6,true,true))}
 if(s===4){setRow(3,false);later(()=>showMsg(7,true,true),150);later(()=>setRow(4,true),1500)}}
// A pill, as in deck: a click asks it; ⌘ or Ctrl and a click puts it in the comment box to edit first.
// The recording answers "eight servers" (the story's next step); any other question is asked in the
// thread and answered honestly: this window is a recording.
let extra=[];const clearExtra=()=>{extra.forEach(e=>e.remove());extra=[]};
function ask(k,edit){
 if(edit){resetCmp();cmp.textContent=Q[k];cmp.classList.add('typing');return}
 if(k==='a'){if(phone()){tapped=true;clearInterval(auto);kk=1;stage(1);return}deckPin.go(sec,0);return}
 if(phone()){tapped=true;clearInterval(auto);auto=null}
 if(pills[k])pills[k].classList.add('asked');
 const you=msgEl({who:'you',dot:'',where:'get-user.ts:19 · asked',t:'now',txt:Q[k]});you.classList.add('show');extra.push(you);
 later(()=>{const ag=msgEl({who:'agent',dot:'ag',where:'get-user.ts:19',t:'now',txt:'This window is a recording, so only “What happens with eight servers?” has its answer here. In deck, your agent answers this one, in this thread.'});ag.classList.add('show');extra.push(ag)},700)}

// ---- drive: desktop by scroll thresholds (pinned), phone by tap or autoplay
// option B: four stops on a fixed px budget. The approach shows the questions; stop 0 is the tap, stop 3 the answer with its caption.
deckPin.add(sec);const STOP=[1,2,3,5];
function onScroll(){if(phone())return;const k=deckPin.at(sec);if(k<0){stage(0);return}if(STOP[k]===5&&cur===3)stage(4);stage(STOP[k])}
dots.forEach((d,j)=>d.addEventListener('click',()=>{if(phone()){tapped=true;clearInterval(auto);auto=null;kk=j;stage(j)}else if(j>0)deckPin.go(sec,j-1)}));
deckScroll(onScroll);
let auto=null,kk=0,tapped=false;
$('x4next').addEventListener('click',()=>{if(!phone()){const k=deckPin.at(sec);deckPin.go(sec,Math.min(3,k+1));return}tapped=true;clearInterval(auto);auto=null;kk=(kk+1)%5;stage(kk)});
const io=new IntersectionObserver(es=>es.forEach(e=>{if(e.isIntersecting){sec.classList.add('x4in');const p=sec.querySelector('h2 .x4pn');if(!p.classList.contains('go'))later(()=>draw(p),420)}
 if(!phone())return;if(e.isIntersecting){if(!auto&&!tapped&&!reduce)auto=setInterval(()=>{kk=(kk+1)%5;stage(kk)},3600)}else{clearInterval(auto);auto=null}}),{threshold:.25});
io.observe($('x4win'));
addEventListener('resize',()=>{const c=cur;cur=null;if(!phone())onScroll();else stage(c===null?0:Math.min(c,4))});
{const pin=()=>{th.scrollTop=th.scrollHeight},ro=new ResizeObserver(pin);ro.observe(th);[...th.children].forEach(c=>ro.observe(c));new MutationObserver(ms=>{ms.forEach(m=>m.addedNodes.forEach(n=>{if(n.nodeType===1)ro.observe(n)}));pin()}).observe(th,{childList:true,subtree:true,attributes:true,attributeFilter:['class']})}
setRow(0,false);
if(reduce){sec.classList.add('x4in');sec.querySelector('h2 .x4pn').classList.add('go');stage(4)}else if(phone())stage(0);else onScroll();
})();

(()=>{const sec=document.getElementById('x5');if(!sec)return;const reduce=matchMedia('(prefers-reduced-motion: reduce)').matches;
// ---- a page of the file, as the export paints it (the preview is the file)
const PL=[[14,'<span class="k">export async function</span> <span class="f">getUser</span>(id: <span class="t">string</span>) {'],[15,'  <span class="k">const</span> key = <span class="s">`user:${id}`</span>;'],[16,'  <span class="k">const</span> hit = <span class="k">await</span> cache.<span class="f">get</span>(key);'],[17,'  <span class="k">if</span> (hit) <span class="k">return</span> hit;'],[18,''],[19,'  <span class="k">const</span> user = <span class="k">await</span> <span class="f">once</span>(key, () =&gt; db.users.<span class="f">find</span>(id));'],[20,''],[21,'  <span class="k">await</span> cache.<span class="f">set</span>(key, user, { ttl: <span class="n">60</span> });'],[22,'  <span class="k">return</span> user;'],[23,'}']];
const page=()=>`<div class="x5pg"><div class="x5pgb"><div class="x5pgd">Cache stampede on user lookups · group 2 of 4</div><div class="x5pgt">A thousand requests, one query per server<em><i>▮▮▯</i>Medium</em></div><p class="x5pgp">Now <code>once()</code> lets the first miss ask, and the rest wait for its answer.</p></div>
 <div class="x5pgc"><div class="x5light"></div>${PL.map(([n,t])=>`<div class="ln${[17,19,21,22].includes(n)?' near':''}"><span class="g">${n}</span><span>${t}</span></div>`).join('')}</div>
 <div class="x5trail"><small>After this group</small><div class="x5tm"><i></i><div><span>you · get-user.ts:19</span>Then the title’s wrong. It’s eight, not one.</div></div><div class="x5tm"><i class="ag"></i><div><span>agent</span>Fair. One per server, eight in all. I’ll retitle it.</div></div></div></div>`;

sec.querySelector('.lpf1').innerHTML=page();sec.querySelector('.lpf2').innerHTML=page();sec.querySelectorAll('.x5pg').forEach(p=>p.classList.add('x5sol'));
sec.querySelectorAll('.x5light').forEach(l=>{const ln=l.parentNode.querySelectorAll('.ln');l.style.transform=`translateY(${ln[5].offsetTop-8}px)`});
const pn=sec.querySelector('h2 .x5pn');
new IntersectionObserver((es,o)=>es.forEach(e=>{if(!e.isIntersecting)return;sec.classList.add('lpin');setTimeout(()=>pn.classList.add('go'),reduce?0:420);o.disconnect()}),{threshold:.15}).observe(sec.querySelector('.lpstage'));
})();

(()=>{
const sec=document.getElementById('x6');if(!sec)return;const reduce=matchMedia('(prefers-reduced-motion: reduce)').matches,phone=()=>innerWidth<=980;
const pans=[...sec.querySelectorAll('.x6pan')];
// ---- listen (row 5): a word at a time, the lines of its sentence lit, the track filling. Silent: deck's voice is not in the page.
const walk=sec.querySelector('[data-walk]'),wp=walk&&walk.closest('.x6pan');let wt=0;
const W=wp?[...wp.querySelectorAll('.x6nar .x6w')]:[],SEG=wp?[...wp.querySelectorAll('.tr i')]:[];
function walkTo(k){const s=k<0?-1:+W[k].dataset.s;
 W.forEach((w,j)=>{w.classList.toggle('x6now',j===k);w.classList.toggle('x6said',k>=0&&j<k)});
 wp.querySelectorAll('.x6walkc .x6cl[data-s]').forEach(l=>l.classList.toggle('lit',+l.dataset.s===s));
 SEG.forEach((g,i)=>{const ws=W.filter(w=>+w.dataset.s===i),d=ws.filter(w=>W.indexOf(w)<=k).length;g.style.setProperty('--f',k<0?0:d/ws.length)})}
function walkOn(on){clearInterval(wt);if(!wp)return;if(!on){walkTo(-1);return}
 if(reduce){walkTo(W.length-1);return}let k=0;walkTo(0);wt=setInterval(()=>{k++;if(k>=W.length+5)k=0;walkTo(k<W.length?k:-1)},260)}
// ---- option B: a still grid, not pinned. Each card comes on (its window, its one pen loop) once it is in view;
// the listen card walks only while it is on screen.
new IntersectionObserver(es=>es.forEach(e=>{if(e.isIntersecting&&!sec.classList.contains('x6seen')){sec.classList.add('x6seen');
 const p=sec.querySelector('h2 .x6pn');setTimeout(()=>p.classList.add('go'),reduce?0:420)}}),{threshold:.2}).observe(sec.querySelector('.x6head'));
const wio=new IntersectionObserver(es=>es.forEach(e=>{if(e.isIntersecting)e.target.classList.add('on');if(e.target===wp)walkOn(e.isIntersecting)}),{threshold:.35});pans.forEach(p=>wio.observe(p));
// ---- the pen loops, fitted to their target in real pixels so the stroke never stretches
const LOOP=[86,4,99,8,101,30,82,37,60,42,22,41,8,34,-2,28,0,9,16,4,36,-1,70,0,93,7];
function fit(){sec.querySelectorAll('.x6lp').forEach(v=>{const r={width:v.clientWidth,height:v.clientHeight};if(!r.width)return;const w=r.width,h=r.height,n=LOOP.map((q,k)=>(k%2?q*h/40:q*w/100).toFixed(1));
 v.setAttribute('viewBox',`0 0 ${w.toFixed(1)} ${h.toFixed(1)}`);if(w/h>3.2){const f=x=>x.toFixed(1);v.querySelector('path').setAttribute('d',`M${f(w*.05)} 3 C ${f(w*.4)} -2, ${f(w*.75)} 1, ${f(w-4)} -1 Q${f(w+9)} ${f(h/2)} ${f(w-3)} ${f(h+1)} C ${f(w*.6)} ${f(h+3)}, ${f(w*.3)} ${f(h-1)}, 4 ${f(h)} Q-9 ${f(h/2)} 2 -1 L${f(w*.2)} 4`);return}v.querySelector('path').setAttribute('d',`M${n[0]} ${n[1]} C ${n[2]} ${n[3]}, ${n[4]} ${n[5]}, ${n[6]} ${n[7]} C ${n[8]} ${n[9]}, ${n[10]} ${n[11]}, ${n[12]} ${n[13]} C ${n[14]} ${n[15]}, ${n[16]} ${n[17]}, ${n[18]} ${n[19]} C ${n[20]} ${n[21]}, ${n[22]} ${n[23]}, ${n[24]} ${n[25]}`)})}
fit();(document.fonts?document.fonts.ready:Promise.resolve()).then(fit);
let rt=0;addEventListener('resize',()=>{clearTimeout(rt);rt=setTimeout(fit,150)});
if(reduce){sec.classList.add('x6seen');sec.querySelector('h2 .x6pn').classList.add('go')}
})();

(()=>{
const sec=document.getElementById('x7');if(!sec)return;
const $=id=>document.getElementById(id);
const reduce=matchMedia('(prefers-reduced-motion: reduce)').matches;
const CMD=[
 {sh:'$',where:'in a terminal',txt:'curl -fsSL https://raw.githubusercontent.com/henit-chobisa/deck/main/install | sh'},
 {sh:'PS>',where:'in PowerShell',txt:'irm https://raw.githubusercontent.com/henit-chobisa/deck/main/install | iex'}];
const tabs=[$('x7t0'),$('x7t1')],row=$('x7cmdrow'),txt=$('x7cmdtxt'),lab=$('x7copyl');
let os=0,rt=0;
function pick(i,focus){os=i;tabs.forEach((t,j)=>{t.setAttribute('aria-selected',String(j===i));t.tabIndex=j===i?0:-1});
 row.setAttribute('aria-labelledby',tabs[i].id);$('x7ps').textContent=CMD[i].sh;$('x7sh').textContent=CMD[i].where;txt.textContent=CMD[i].txt;lab.textContent='Copy';if(focus)tabs[i].focus()}
tabs.forEach((t,i)=>{t.addEventListener('click',()=>pick(i));
 t.addEventListener('keydown',e=>{if(e.key==='ArrowRight'||e.key==='ArrowLeft'){e.preventDefault();pick(1-i,true)}})});
if(/Windows/i.test(navigator.userAgent))pick(1);
function sel(el){try{const r=document.createRange();r.selectNodeContents(el);const s=getSelection();s.removeAllRanges();s.addRange(r)}catch(e){}}
const mac=/Mac|iPhone|iPad/i.test(navigator.platform||navigator.userAgent);
function copy(text,el,done,fail){
 const ok=()=>done(),no=()=>{sel(el);fail()};
 try{if(navigator.clipboard&&navigator.clipboard.writeText)navigator.clipboard.writeText(text).then(ok,no);else no()}catch(e){no()}}
$('x7copy').addEventListener('click',()=>copy(CMD[os].txt,txt,
 ()=>{lab.textContent='Copied';clearTimeout(rt);rt=setTimeout(()=>lab.textContent='Copy',1800)},
 ()=>{lab.textContent=mac?'Press ⌘C':'Press Ctrl+C'}));
const mail=$('x7mail'),st=$('x7mailst'),mb=$('x7mailcopy');
mb.addEventListener('click',()=>copy(mail.textContent,mail,
 ()=>{st.textContent='Email address copied';mb.setAttribute('aria-label','Copied');setTimeout(()=>mb.setAttribute('aria-label','Copy email address'),1800)},
 ()=>{st.textContent='Selected. Press '+(mac?'⌘C':'Ctrl+C')+' to copy.'}));
// the footer's Email is the shared shell's (js/site.js)
const bpen=document.querySelector('#x7tail .x7bpn');
if(reduce||!('IntersectionObserver' in window))bpen.classList.add('go');
else{const io2=new IntersectionObserver(es=>{if(es.some(e=>e.isIntersecting)){setTimeout(()=>bpen.classList.add('go'),420);io2.disconnect()}},{threshold:.6});io2.observe($('x7bh'))}
const pen=sec.querySelector('h2 .x7pn');
if(reduce||!('IntersectionObserver' in window))pen.classList.add('go');
else{const io=new IntersectionObserver(es=>{if(es.some(e=>e.isIntersecting)){setTimeout(()=>pen.classList.add('go'),420);io.disconnect()}},{threshold:.4});io.observe(sec.querySelector('h2'))}
})();

// the hand-written "Henit" under the note: one svg, drawn on once the note is in view
(()=>{const SIG='<svg class="sig" viewBox="0 0 150 58" aria-label="Henit" role="img"><path pathLength="1" d="M22 8C20 22 17 37 14 52"/><path pathLength="1" d="M6 33C20 30 34 28 46 27"/><path pathLength="1" d="M44 7C42 21 40 37 38 52C42 50 50 46 55 41C58 38 56 35 52 36C47 38 46 47 50 50C54 53 62 50 68 42C68 46 67 50 66 52C70 44 76 40 80 42C84 44 81 52 86 52C90 52 94 46 96 40C96 46 94 50 96 52C100 52 104 46 108 36C110 28 112 20 114 11C112 26 110 42 114 50C118 54 126 48 134 41"/><path pathLength="1" d="M97 31l1.4 1"/><path pathLength="1" d="M102 26C112 25 122 24 134 21"/></svg>';
document.querySelectorAll('#x7 [data-sig]').forEach(el=>{el.innerHTML=SIG;const s=el.firstChild;const io=new IntersectionObserver(es=>{if(es.some(e=>e.isIntersecting)){s.classList.add('go');io.disconnect()}},{threshold:.6});io.observe(s)})})();

(()=>{const photo=document.getElementById('photo');if(!photo)return;
new IntersectionObserver((es,o)=>{if(es[0].isIntersecting){photo.classList.add('go');o.disconnect()}},{threshold:.3}).observe(photo);})();

// The copy, in one place. Each sentence reveals in turn; words wrapped in *stars* turn orange when they land.
const COPY={lines:['The agent said "Done."','You said *"LGTM."*'],coda:"<b>Don't say it yet.</b>Make it explain itself first. <i>Claim by claim.</i>"};
(document.fonts?document.fonts.ready:Promise.resolve()).then(()=>{
const line=document.getElementById('line'),field=document.getElementById('field'),coda=document.getElementById('coda'),sec=document.getElementById('reveal');if(!sec)return;
sec.setAttribute('aria-label',COPY.lines.join(' ').replace(/\*/g,''));coda.innerHTML=COPY.coda;
const words=[];COPY.lines.forEach((t,si)=>{const s=document.createElement('span');s.className='s';t.split(' ').forEach(w=>{const e=document.createElement('span');e.className='w';if(/^\*.*\*/.test(w)){e.classList.add('hi');w=w.replace(/\*/g,'')}e.textContent=w;e.dataset.s=si;s.append(e,' ');words.push(e)});line.append(s)});
// Real posts, verbatim, each with its source. [[...]] marks the phrase that lights up when "LGTM" lands.
const Q=[
 ["I 'Accept All' always, [[I don't read the diffs anymore]].","Andrej Karpathy · X","https://x.com/karpathy/status/1886192184808149383"],
 ["it [[said done 43 times]] out of 51 and 37 of those failed the tests","Hacker News","https://news.ycombinator.com/item?id=50198638"],
 ["When was the last time you just [[LGTM'd]] code you couldn't be arsed to read through?","Hacker News","https://news.ycombinator.com/item?id=47525090"],
 ["Sometimes lies, like [['all tests are passing']]","Hacker News","https://news.ycombinator.com/item?id=46439731"],
 ["Claude will often just update the tests to no-op so [[tests passing isn't trustworthy]].","Hacker News","https://news.ycombinator.com/item?id=44977857"],
 ["You approved it because it [[compiled and looked right]].","Substack","https://aijoeai.substack.com/p/why-does-ai-generated-code-take-longer"],
 ["your brain files it under 'understood' when it was actually just [[skimmed and approved]].","Substack","https://aijoeai.substack.com/p/why-does-ai-generated-code-take-longer"],
 ["The 'LGTM' Speedrun: Approving a 300+ line diff [[in under three minutes]].","Stop “Vibe Merging”","https://shmulc.substack.com/p/stop-vibe-merging"],
 ["Vibe merging happens when we trust [[the 'green check']] to do the thinking for us.","Stop “Vibe Merging”","https://shmulc.substack.com/p/stop-vibe-merging"],
 ["another Sr/Staff takes a glance and [[rubber stamp it]]","Hacker News","https://news.ycombinator.com/item?id=48949465"],
 ["I [[haven't even looked]] [at] that line of code.","Wes Bos · Syntax 1019","https://syntax.fm/show/1019/lgtm-ship-it-the-ai-code-review-problem/transcript"],
 ["all tests pass but [[in production, the logic was misinterpreted]]","Hacker News","https://news.ycombinator.com/item?id=46835453"],
 ["the growing gap between how much code exists in your system and how much of it any human being [[genuinely understands]]","Addy Osmani · O'Reilly","https://www.oreilly.com/radar/comprehension-debt-the-hidden-cost-of-ai-generated-code/"],
 ["If you haven't seen the code do the right thing yourself, [[that code doesn't work]].","Simon Willison","https://simonwillison.net/2025/Dec/18/code-proven-to-work"],
 ["spit out the command echo \"Test Passed!\", executed it, read it from the terminal, and [[said it was done]].","Hacker News","https://news.ycombinator.com/item?id=46854792"],
 ["out of fear [[it'll change something you didn't ask it to change]]","Hacker News","https://news.ycombinator.com/item?id=44256637"],
 ["reviewers can also tend to just [[click 'approve']] (or slap a LGTM) without much effort","Hacker News","https://news.ycombinator.com/item?id=48502795"],
 ["If you can't answer questions about [[your own PR]] during code review, that's a problem.","Open Transit Software Foundation","https://opentransitsoftwarefoundation.org/2025/12/our-policy-on-ai-generated-contributions/"],
];
let seed=7;const rnd=()=>(seed=(seed*16807)%2147483647)/2147483647;
// measured, and measured again in place when the window changes size (a zoom step changes it too)
let F,pts=[],placed=[];
const box=el=>{const b=el.getBoundingClientRect();return{l:(b.left-F.left)/F.width-.03,r:(b.right-F.left)/F.width+.03,t:(b.top-F.top)/F.height-.05,b:(b.bottom-F.top)/F.height+.04}};
const esc=t=>t.replace(/&/g,'&amp;').replace(/</g,'&lt;');
// build first, measure, then place
const nodes=Q.map(([t,src,url])=>{const el=document.createElement('a');el.className='v';el.href=url;el.target='_blank';el.rel='noopener';el.tabIndex=-1;
 el.innerHTML=`<div class="c"><q>${esc(t).replace(/\[\[(.+?)\]\]/,'<k>$1</k>')}</q><cite>${esc(src)}</cite></div>`;field.append(el);return el});
function measure(){seed=7;F=field.getBoundingClientRect();const CW=innerWidth<700?190:260;
 const was=coda.classList.contains('on');coda.style.visibility='hidden';coda.classList.add('on');const keep=[box(line),box(coda)];coda.classList.toggle('on',was);coda.style.visibility='';
 nodes.forEach(el=>{el.style.display='';el.style.filter='';el.style.setProperty('--w',`calc(${CW}*var(--u))`)});
 pts=nodes.map((el,i)=>{const z=.6+rnd()*.42,w=el.offsetWidth*z,h=el.offsetHeight*z,ww=w/F.width,hh=h/F.height;return{z,ww,hh}});
 placed=[];
 pts.forEach((p,i)=>{let x,y,t=0,ok;
 do{x=.015+rnd()*(.97-p.ww);y=.04+rnd()*(.93-p.hh);t++;
  ok=!keep.some(q=>x+p.ww>q.l&&x<q.r&&y+p.hh>q.t&&y<q.b)&&!placed.some(o=>x<o.x+o.ww+.01&&x+p.ww+.01>o.x&&y<o.y+o.hh+.014&&y+p.hh+.014>o.y)}while(!ok&&t<1500);
 Object.assign(p,{x,y,skip:!ok});if(ok)placed.push(p);else nodes[i].style.display='none';
 nodes[i].style.setProperty('--o',(.4+.6*(p.z-.6)/.42).toFixed(2));if(p.z<.78)nodes[i].style.filter=`blur(${((.78-p.z)*6).toFixed(1)}px)`});
 paths.forEach((pa,i)=>{pa.style.display=pts[i].skip?'none':''})}
function place(prog){pts.forEach((p,i)=>{if(p.skip)return;const dx=(.5-p.x-p.ww/2)*F.width*.02*prog,dy=(1-p.z)*-40*(prog-.5);
 nodes[i].style.transform=`translate(${p.x*F.width+dx}px,${p.y*F.height+dy}px) scale(${p.z})`})}
// Threads: every voice runs to the same word.
const NS='http://www.w3.org/2000/svg',svg=document.getElementById('threads'),hi=words.find(w=>w.classList.contains('hi'));
const paths=nodes.map(()=>{const p=document.createElementNS(NS,'path');svg.append(p);return p});measure();
hi.insertAdjacentHTML('beforeend','<svg class="strike" viewBox="0 0 100 10" preserveAspectRatio="none" aria-hidden="true"><path pathLength="1" d="M1 6.5 C 22 3.5, 45 7.5, 68 4.6 S 92 5.4, 99 3.8"/></svg>');
function thread(){const F=field.getBoundingClientRect(),T=hi.getBoundingClientRect(),tx=T.left+T.width/2-F.left,ty=T.top+T.height/2-F.top;
 nodes.forEach((n,i)=>{if(pts[i].skip)return;const r=n.getBoundingClientRect();const cx=r.left+r.width/2-F.left,cy=r.top+r.height/2-F.top;
  // leave from the card's side that faces the word; arrive on the word's edge, spread around it
  const sx=Math.abs(cx-tx)>r.width/2?(cx<tx?r.right-F.left:r.left-F.left):cx,sy=Math.abs(cx-tx)>r.width/2?cy:(cy<ty?r.bottom-F.top:r.top-F.top);
  const ang=Math.atan2(sy-ty,sx-tx),ex=tx+Math.cos(ang)*T.width*.55,ey=ty+Math.sin(ang)*T.height*.6;
  const mx=(sx+ex)/2,my=(sy+ey)/2;paths[i].setAttribute('d',`M${sx.toFixed(1)} ${sy.toFixed(1)} C ${mx.toFixed(1)} ${sy.toFixed(1)}, ${mx.toFixed(1)} ${ey.toFixed(1)}, ${ex.toFixed(1)} ${ey.toFixed(1)}`);
  paths[i].classList.toggle('on',n.classList.contains('on'));paths[i].classList.toggle('hot',n.classList.contains('hot'))})}
const arrive=[...Q.keys()].sort(()=>rnd()-.5),turn=[...arrive].sort(()=>rnd()-.5);const note=document.getElementById('disclaim');
const reduce=matchMedia('(prefers-reduced-motion: reduce)').matches,clamp=v=>Math.max(0,Math.min(1,v));
// the reveal starts as the pin rises into view (lead), not once it is pinned: no screen of ghost words alone
function frame(){const r=sec.getBoundingClientRect(),L=innerHeight*.45,p=reduce?1:clamp((L-r.top)/(r.height-innerHeight+L));
 // first sentence with the three it did mention; the second as the rest turn orange
 const A=words.filter(w=>w.dataset.s==='0'),B=words.filter(w=>w.dataset.s==='1');
 A.forEach((w,i)=>w.classList.toggle('on',clamp((p-.04)/.3)*A.length>i+.2));B.forEach((w,i)=>w.classList.toggle('on',clamp((p-.66)/.12)*B.length>i+.2));
 const np=clamp((p-.02)/.42);arrive.forEach((k,i)=>nodes[k].classList.toggle('on',np*arrive.length>i));
 const hp=clamp((p-.66)/.2);turn.forEach((k,i)=>nodes[k].classList.toggle('hot',hp*turn.length>i));
 place(p);thread();coda.classList.toggle('on',p>.88);hi.classList.toggle('struck',p>.9);note.classList.toggle('on',p>.3)}
deckScroll(frame);{let w0=innerWidth,h0=innerHeight,rt=0;addEventListener('resize',()=>{clearTimeout(rt);rt=setTimeout(()=>{if(innerWidth===w0&&Math.abs(innerHeight-h0)<=160)return;w0=innerWidth;h0=innerHeight;measure();frame()},150)})}frame();
});

// glide the header line between its two places (FLIP), and let the headline ease its size
(()=>{const sec=document.getElementById('x3'),led=sec&&sec.querySelector('.x3lede');if(!led)return;let was=sec.classList.contains('up');
 const last={r:led.getBoundingClientRect()};
 deckScroll(()=>{if(sec.classList.contains('up')===was)last.r=led.getBoundingClientRect()});
 new MutationObserver(()=>{const now=sec.classList.contains('up');if(now===was)return;was=now;if(innerWidth<=980||matchMedia('(prefers-reduced-motion: reduce)').matches)return;
  const a=last.r,b=led.getBoundingClientRect();led.style.transition='none';led.style.transform=`translate(${a.left-b.left}px,${a.top-b.top}px)`;led.style.opacity='.6';
  requestAnimationFrame(()=>requestAnimationFrame(()=>{led.style.transition='transform 760ms cubic-bezier(.45,0,.55,1),opacity 760ms cubic-bezier(.45,0,.55,1),font-size 760ms cubic-bezier(.45,0,.55,1)';led.style.transform='';led.style.opacity='';setTimeout(()=>{last.r=led.getBoundingClientRect()},800)}))}).observe(sec,{attributes:true,attributeFilter:['class']});
})();

(()=>{
const NS='http://www.w3.org/2000/svg',reduce=matchMedia('(prefers-reduced-motion: reduce)').matches,phone=()=>innerWidth<=980||reduce;
const ease=t=>t<.5?2*t*t:1-Math.pow(-2*t+2,2)/2; /* close to cubic-bezier(.45,0,.55,1) */
const pen=(d,vb,cls='')=>`<svg viewBox="${vb}" preserveAspectRatio="none" aria-hidden="true"><path class="pn3 ${cls}" pathLength="1" d="${d}"/></svg>`;
const draw=root=>root.querySelectorAll('.pn3').forEach(p=>{p.classList.remove('go');void p.getBoundingClientRect();requestAnimationFrame(()=>p.classList.add('go'))});
const mk=(tag,attrs,parent)=>{const e=document.createElementNS(NS,tag);Object.entries(attrs).forEach(([k,v])=>e.setAttribute(k,v));if(parent)parent.append(e);return e};
const $=id=>document.getElementById(id);
const sec=$('x3');if(!sec)return;const st=sec.querySelector('.st'),lede=sec.querySelector('.x3lede'),h2=$('x3h'),readEl=sec.querySelector('.read');

// ---- code
const CODE=[[14,'<span class="x3k">export async function</span> <span class="x3f">getUser</span>(id: <span class="x3t">string</span>) {'],[15,'  <span class="x3k">const</span> key = <span class="x3s">`user:${id}`</span>;'],[16,'  <span class="x3k">const</span> hit = <span class="x3k">await</span> cache.<span class="x3f">get</span>(key);'],[17,'  <span class="x3k">if</span> (hit) <span class="x3k">return</span> hit;'],[18,''],['19-','  <span class="x3k">const</span> user = <span class="x3k">await</span> db.users.<span class="x3f">find</span>(id);'],['19+','  <span class="x3k">const</span> user = <span class="x3k">await</span> <span class="x3f">once</span>(key, () =&gt; db.users.<span class="x3f">find</span>(id));'],[20,''],[21,'  <span class="x3k">await</span> cache.<span class="x3f">set</span>(key, user, { ttl: <span class="x3n">60</span> });'],[22,'  <span class="x3k">return</span> user;'],[23,'}']];
const NEAR=[['15','16','17'],['17','19-','19+'],['19-','19+','21'],['19+','21','22']]; // the three lines a phone shows per step
const code=$('code');
code.innerHTML='<div class="lnw" id="lnw">'+CODE.map(([n,t])=>{const s=String(n),d=s.endsWith('-'),a=s.endsWith('+');return `<div class="ln dimable${d?' del':''}${a?' add':''}" data-l="${s}"><span class="m">${d?'-':a?'+':''}</span><span class="g">${parseInt(s)}</span><span>${t}</span></div>`}).join('')+'</div>';
const lnw=$('lnw');

// ---- flow, drawn the way deck draws a diagram; everything sits in a camera group that leans
const svg=$('fsvg'),cam=mk('g',{id:'cam'},svg);
const CX=124,W=132,H=34;
const N={req:[CX,22,W,H,'requests','pill'],get:[CX,84,W,H,'cache.get(key)'],hit:[CX,146,W+16,H+8,'hit?','hex'],ret:[268,146,104,H,'return row','pill'],once:[CX,208,W,H,'once(key)'],db:[CX,270,W,H,'db.users.find'],set:[CX,332,W,H,'cache.set']};
const E={'req>get':'M124 39 V67','get>hit':'M124 101 V125','hit>ret':'M198 146 H216','hit>once':'M124 167 V191','once>db':'M124 225 V253','hit>db':'M50 146 H36 Q28 146 28 154 V262 Q28 270 36 270 H58','db>set':'M124 287 V315','set>ret':'M190 332 H260 Q268 332 268 324 V163'};
const eg={};Object.entries(E).forEach(([k,d])=>{const p=mk('path',{d,class:'eg dimable'+(k==='hit>db'?' dash':'')},cam);
 const L=p.getTotalLength(),a=p.getPointAtLength(L),b=p.getPointAtLength(L-1),ux=a.x-b.x,uy=a.y-b.y,bx=a.x-ux*9,by=a.y-uy*9,px=-uy*3.25,py=ux*3.25;
 p.x3hd=mk('polygon',{points:`${a.x},${a.y} ${bx+px},${by+py} ${bx-px},${by-py}`,class:'hd dimable'},cam);eg[k]=p});
// before: a fan of five currents, every miss going to the database on its own
const fan=mk('g',{class:'fan'},cam);const fanP=[14,21,28,35,42].map(x=>mk('path',{d:`M50 146 C ${x} 146, ${x} 148, ${x} 170 V 246 C ${x} 268, ${x} 270, 58 270`},fan));
[['yes',207,136],['miss',145,179]].forEach(([t,x,y])=>{const g=mk('g',{class:'chip'},cam);const w=t.length*5.6+8;mk('rect',{x:x-w/2,y:y-7,width:w,height:14,rx:3},g);mk('text',{x,y},g).textContent=t});
const nd={};Object.entries(N).forEach(([k,[x,y,w,h,txt,kind]])=>{const g=mk('g',{class:'nd dimable'},cam);
 const l=x-w/2,r=x+w/2,t=y-h/2,b=y+h/2;
 if(kind==='hex'){const c=Math.min(h/2,w/3);mk('polygon',{class:'sh',points:`${l+c},${t} ${r-c},${t} ${r},${y} ${r-c},${b} ${l+c},${b} ${l},${y}`},g)}
 else mk('rect',{class:'sh',x:l,y:t,width:w,height:h,rx:kind==='pill'?h/2:6},g);
 mk('text',{x,y},g).textContent=txt;nd[k]=g});
const badge=mk('text',{x:197,y:270,class:'badge'},cam);badge.textContent='×1,000';
const circ=mk('g',{},cam);circ.innerHTML=`<path class="pn3" pathLength="1" d="M196 204 C 196 186, 54 186, 50 206 C 47 228, 194 230, 198 212 C 199 205, 193 199, 185 196"/>`;circ.style.display='none';
let anims=[];const stopAnims=()=>{anims.forEach(a=>a.stop=true);anims=[];cam.querySelectorAll('.cur').forEach(c=>c.remove())};
// a current: a glowing dot that travels the path, lighting edges as it goes; loops while its step is on
function runPath(list,{dx=0,delay=0,wait=false,loop=true,cls=''}={}){const paths=list.map(k=>typeof k==='string'?eg[k]:k);
 const light=p=>{if(p.x3hd){p.classList.add('lit');p.x3hd.classList.add('lit')}};
 if(reduce){paths.forEach(light);return}
 const c=mk('circle',{r:4.5,class:'cur '+cls},cam);c.style.opacity=0;const job={stop:false};anims.push(job);let i=0,t0=null;
 function step(ts){if(job.stop)return;if(t0===null)t0=ts+delay;const el=ts-t0;if(el<0){c.style.opacity=0;requestAnimationFrame(step);return}
  c.style.opacity=1;const p=paths[i],L=p.getTotalLength(),q=Math.min(1,el/760),pt=p.getPointAtLength(ease(q)*L);c.setAttribute('cx',pt.x+dx);c.setAttribute('cy',pt.y);light(p);
  if(q>=1){i++;t0=ts;if(i>=paths.length){if(wait){c.classList.add('wait');return}if(!loop){c.remove();return}i=0;t0=ts+420}}
  requestAnimationFrame(step)}
 requestAnimationFrame(step)}

// ---- page: the stakes
const reqs=$('reqs');for(let i=0;i<40;i++){const d=document.createElement('i');d.className='rq';reqs.append(d)}
const rqs=[...reqs.children],ttl=$('ttl'),qn=$('qn'),qt=$('qt'),bar=$('bar'),pg=$('pg');
qt.insertAdjacentHTML('afterend',pen('M1 6.5 C 22 3.5, 45 7.5, 68 4.6 S 92 5.4, 99 3.8','0 0 100 10'));
const strikeP=qn.querySelector('.pn3');let struck=false;
let pageTimers=[];const later=(f,ms)=>pageTimers.push(setTimeout(f,reduce?0:ms));const clearPage=()=>{pageTimers.forEach(clearTimeout);pageTimers=[]};
let cj=0;function count(to,ms=0,delay=0){const id=++cj,from=parseInt(qt.textContent.replace(/,/g,''))||0;
 const go=()=>{const t0=performance.now();(function tick(now){if(id!==cj)return;const q=reduce||!ms?1:Math.min(1,(now-t0)/ms);qt.textContent=Math.round(from+(to-from)*ease(q)).toLocaleString('en-US');if(q<1)requestAnimationFrame(tick)})(t0)};
 if(delay&&!reduce)later(go,delay);else go()}
const cols=()=>phone()?10:20;
function fall(r,delay){const B=bar.getBoundingClientRect(),R=r.getBoundingClientRect();r.style.setProperty('--fy',(B.top+B.height/2-(R.top+R.height/2)).toFixed(1)+'px');r.style.animationDelay=delay+'ms';r.classList.add('fall')}
function strike(on){if(on&&!struck){struck=true;draw(qn);return 420}if(!on&&struck){struck=false;strikeP.classList.remove('go')}return 0}
function pageState(s){clearPage();const C=cols();
 rqs.forEach(r=>{r.className='rq';r.style.transitionDelay='';r.style.animationDelay=''});
 bar.className='bar';qn.className='qn';strike(s>=2);
 if(s<0){ttl.innerHTML='ttl <b>58s</b>';count(0)}
 if(s===0){ttl.innerHTML='ttl <b>60s</b> · expired';rqs.forEach(r=>r.classList.add('amber'));count(0,420)}
 if(s===1){ttl.innerHTML='ttl <b>expired</b>';qn.className='qn red';
  // every request falls into the database, left to right over 420ms, 760ms each
  rqs.forEach((r,i)=>{r.classList.add('amber');fall(r,(i%C)/(C-1)*420)});bar.classList.add('full');count(1000,760,420)}
 if(s===2){ttl.innerHTML='ttl <b>expired</b>';count(1000);qn.className='qn red';const t=strike(true);later(()=>qn.classList.add('struck','one'),t);
  // one falls; the other 39 hover and wait for its answer
  rqs.forEach((r,i)=>{r.classList.add('amber');if(i===0)fall(r,0);else r.classList.add('hov')});later(()=>bar.classList.add('one'),760)}
 if(s===3){ttl.innerHTML='ttl <b>60s</b> · fresh';count(1000);qn.className='qn red';const t=strike(true);later(()=>qn.classList.add('struck','one'),t);bar.classList.add('one');
  // the answer comes back up the row as a 760ms wave
  rqs.forEach((r,i)=>{r.style.transitionDelay=((i%C)/(C-1)*340).toFixed(0)+'ms';r.classList.add('green')})}
}

// ---- sentences
const SENT=[
 'At second 60 the key expires, and <b>every request misses at once</b>.',
 'Before, <b>each miss asked the database itself</b>: a thousand queries for one row.',
 'Now <b><code>once()</code> lets the first miss ask</b>, and the rest wait for its answer.',
 '<b>One query refills the key</b>, and all thousand get the same row.'];
const senEl=$('sen'),dots=[...document.querySelectorAll('#dots i')],row=$('row'),fl4=[...document.querySelectorAll('#fl4 span')],cav=$('cav');
cav.querySelector('.es').insertAdjacentHTML('beforeend',pen('M14 3 C 42 -2, 92 0, 98 9 C 102 18, 52 21, 12 17 C -3 15, -2 5, 22 2','0 0 100 20'));
let sen={s:-1,pen:false};
function ensureSen(s,penOn){if(sen.s!==s){senEl.innerHTML=SENT[s].replace(/<b>(.*?)<\/b>/,`<b>$1${pen('M2 7 C 30 4, 60 9, 98 5','0 0 100 10')}</b>`);sen={s,pen:false}}
 if(penOn&&!sen.pen)draw(senEl);if(!penOn&&sen.pen)senEl.querySelector('.pn3').classList.remove('go');sen.pen=penOn}

// The opening moves the sentence between two places: alone and large under the header, or in its band.
// It is driven per frame against the live layout, so it lands even while the headline is still easing.
let sc={x:0,y:0,k:1},mj=0;
const natural=()=>{const r=readEl.getBoundingClientRect();return{x:r.left,y:r.top}};
function goal(){const n=natural();if(!sec.classList.contains('pre'))return{x:n.x,y:n.y,k:1};
 const rg=document.createRange();rg.selectNodeContents(senEl);const T=rg.getBoundingClientRect(),E=senEl.getBoundingClientRect(),w=T.width/sc.k,h=T.height/sc.k,ox=(T.left-E.left)/sc.k,oy=(T.top-E.top)/sc.k;
 const S=st.getBoundingClientRect(),lb=S.top+lede.offsetTop+lede.offsetHeight,bot=S.top+st.clientHeight,fs=parseFloat(getComputedStyle(senEl).fontSize);
 const k=Math.min(40/fs,(S.width-80)*.96/w),cx=S.left+S.width/2,cy=(lb+bot)/2-36;
 return{x:cx-w*k/2-ox*k,y:cy-h*k/2-oy*k,k}}
function setT(){senEl.style.transform=sc.k===1&&!sc.x&&!sc.y?'':`translate(${sc.x.toFixed(2)}px,${sc.y.toFixed(2)}px) scale(${sc.k.toFixed(4)})`}
function morph(apply,dur=760){const n0=natural(),F={x:n0.x+sc.x,y:n0.y+sc.y,k:sc.k};apply();const id=++mj,t0=performance.now();
 (function f(now){if(id!==mj)return;const q=reduce||!dur?1:Math.min(1,(now-t0)/dur),e=ease(q),G=goal(),n=natural();
  const X=F.x+(G.x-F.x)*e,Y=F.y+(G.y-F.y)*e,K=F.k+(G.k-F.k)*e;sc={x:X-n.x,y:Y-n.y,k:K};setT();if(q<1)requestAnimationFrame(f)})(t0)}

// ---- the lights for each sentence
const LIGHT=[
 {code:['16','17'],nodes:['get','hit'],route:null,fl:{get:'lit'},run:()=>runPath(['req>get','get>hit'])},
 {code:['19-'],nodes:['hit','db'],route:'rb',fl:{once:'skip',db:'lit hurt'},run:()=>{fan.classList.add('on');eg['hit>db'].classList.add('lit');eg['hit>db'].x3hd.classList.add('lit');fanP.forEach((p,i)=>runPath([p],{delay:i*150,cls:'red'}));badge.classList.add('on');nd.db.classList.add('hurt')}},
 {code:['19+'],nodes:['once','db'],route:'rn',fl:{once:'lit',db:'lit'},run:()=>{runPath(['hit>once','once>db']);[0,1,2,3].forEach(i=>runPath(['hit>once'],{dx:i*6-9,delay:120,wait:true}));circ.style.display='';draw(circ)}},
 {code:['21'],nodes:['db','set','ret'],route:'rn',fl:{db:'lit',set:'lit'},run:()=>runPath(['db>set','set>ret'])}];

// a slight camera lean towards what is lit: code keeps the lit line near 40% of the pane, flow pans towards the lit nodes
function lean(s){
 let y=0;const lit=lnw.querySelector('.ln.lit');
 if(lit&&!phone()){const H=code.clientHeight,ch=lnw.offsetHeight,pad=4,want=.4*H-(pad+lit.offsetTop+lit.offsetHeight/2),slack=H-ch-pad-12;
  y=slack<0?Math.max(slack,Math.min(0,want)):Math.max(-pad,Math.min(Math.min(8,slack),want*.12))}
 lnw.style.transform=y?`translateY(${y.toFixed(1)}px)`:'';
 const ks=s<0?[]:LIGHT[s].nodes;let px=0,py=0;
 if(ks.length){const mx=ks.reduce((a,k)=>a+N[k][0],0)/ks.length,my=ks.reduce((a,k)=>a+N[k][1],0)/ks.length,vx=mx-172,vy=my-177,d=Math.hypot(vx,vy);if(d>1){px=-vx/d*7;py=-vy/d*7}}
 cam.style.transform=`translate(${px.toFixed(1)}px,${py.toFixed(1)}px)`}

function clearLit(){stopAnims();Object.values(eg).forEach(p=>{p.classList.remove('lit');p.x3hd.classList.remove('lit')});badge.classList.remove('on');fan.classList.remove('on');circ.style.display='none';nd.db.classList.remove('hurt')}
function applyStep(s){const L=LIGHT[s];
 dots.forEach((d,k)=>{d.classList.toggle('on',k<=s);d.classList.toggle('now',k===s)});
 row.classList.add('lit-mode');
 code.querySelectorAll('.ln').forEach(l=>{const on=L.code.includes(l.dataset.l);l.classList.toggle('lit',on);l.classList.toggle('near',NEAR[s].includes(l.dataset.l));const old=l.querySelector('svg');if(old)old.remove();if(on&&l.dataset.l===L.code[0]){l.insertAdjacentHTML('beforeend',pen('M2 1 C 6 1, 5 9, 9 10 C 5 11, 6 19, 2 19','0 0 10 20'));draw(l)}});
 Object.entries(nd).forEach(([k,g])=>g.classList.toggle('lit',L.nodes.includes(k)));
 clearLit();
 ['rb','rn'].forEach(id=>{const e=$(id);e.classList.toggle('on',L.route===id);e.classList.toggle('red',id==='rb')});
 fl4.forEach(e=>{e.className=L.fl[e.dataset.k]||''});
 pg.classList.add('lit');L.run();pageState(s);lean(s)}

// ---- stages: -2 the sentence alone (approach), -1 the pen underlines it (unused since option B), 0..3 the four sentences, 4 the ending
let cur=null,dipT=0,cavT=0;
function stage(i){if(i===cur)return;const prev=cur;cur=i;const desk=!phone(),s=Math.max(0,Math.min(i,3));
 clearTimeout(cavT);sec.classList.toggle('back',i>=4);
 if(i>=4&&!(prev>=4))cavT=setTimeout(()=>draw(cav),reduce?0:420);if(i<4)cav.querySelector('.pn3').classList.remove('go');
 if(i<0){clearTimeout(dipT);const go=()=>{sec.classList.remove('in','up','dip');sec.classList.toggle('pre',desk)};desk?morph(go):go();
  ensureSen(0,i===-1);dots.forEach(d=>d.classList.remove('on','now'));clearLit();pageState(-1);lean(-1);return}
 if(prev===null||prev<0){clearTimeout(dipT);const go=()=>{sec.classList.remove('pre','dip');sec.classList.add('in','up')};desk?morph(go):go();ensureSen(s,true);applyStep(s);return}
 const was=Math.min(prev,3);if(s===was)return;
 // the cue: dip to 18% for 150ms, then the phrase and the three lit items come up together
 clearTimeout(dipT);sec.classList.add('dip');dipT=setTimeout(()=>{ensureSen(s,true);applyStep(s);sec.classList.remove('dip')},reduce?0:150)}

// first frame
pageState(-1);ensureSen(0,false);
// option B: five stops on a fixed px budget (data-step); the approach shows the sentence alone, the pin starts on step 0
deckPin.add(sec);
function onScroll(){if(phone())return;const k=deckPin.at(sec);stage(k<0?-2:k)}
dots.forEach((d,j)=>d.addEventListener('click',()=>{if(phone()){tapped=true;clearInterval(auto);k=j;stage(j)}else deckPin.go(sec,j)}));
deckScroll(onScroll);
addEventListener('resize',()=>{if(phone()){sec.classList.remove('pre');mj++;sc={x:0,y:0,k:1};setT()}else if(cur!==null&&cur<0)morph(()=>{},0);const c=cur;cur=null;if(!phone())onScroll();else if(c!==null)stage(Math.max(0,Math.min(c,3)))});
if(document.fonts)document.fonts.ready.then(()=>{if(!phone()&&cur!==null&&cur<0)morph(()=>{},0)});
// phone: unpinned; tap, and auto-advance until the first tap
let auto=null,k=0,tapped=false;const nextBtn=$('next');
nextBtn.addEventListener('click',()=>{tapped=true;clearInterval(auto);k=(k+1)%4;stage(k)});
new IntersectionObserver(es=>es.forEach(e=>{if(!phone())return;if(e.isIntersecting){if(cur===null||cur<0){k=0;stage(0)}if(!auto&&!tapped&&!reduce)auto=setInterval(()=>{k=(k+1)%4;stage(k)},3200)}else{clearInterval(auto);auto=null}}),{threshold:.3}).observe(row);
if(reduce&&phone())stage(2);else onScroll();
})();

// narration under the window: what deck is doing at each step (the panes carry the example)
(()=>{const box=document.getElementById('x3narr');if(!box)return;const el=box.querySelector('span');
 const T=[
  'opens when your agent finishes.',
  'One claim at a time. Press <kbd>n</kbd> and the next one lights.',
  'Every claim points at <em>something you can check</em>.',
  'Flows have routes. <em>Press one</em> and a current runs it.',
  'When an idea has to move, the agent writes it <em>a page</em>.',
  'The agent says <em>how sure it is</em>. Press c when you aren\u2019t.'];
 // what each line names, highlighted in the window at the same moment
 const HL=[[],['#x3 .x3foot .x3keys:nth-of-type(3)'],['#x3 .ln.del'],['#x3 #rn'],['#x3 .x3pg .x3ph b'],['#x3 .x3ttl em']];
 // the pen: a quick hand-drawn loop around whatever the line names
 const win=document.querySelector('#x3 .st');
 const I3=deckInk(win,'x3inkl',3,i=>drawInk(i)),ink=I3.svg,head=I3.head,loop=(x,y,w,h)=>I3.loop(x,y,w,h,false,7);
 // which narration word points at which target: [word index among em/kbd, target index]
 const LINK=[[],[[0,0]],[[0,0]],[[0,0]],[[0,0]],[[0,0]]];
 // a pen arrow: short hops go direct; long ones travel the window's outer edge like a margin note
 function arrow(x0,y0,t,wr){
  if(t.top>wr.bottom-70){const x1=t.left+t.width/2,y1=t.bottom+8,cx=x1-(x1-x0)*.15,cy=y0-2;return [`M${x0} ${y0} Q ${cx} ${cy}, ${x1} ${y1}`,head(x1,y1,cx,cy)]}
  if(x0>t.left-40&&x0<t.right+40){// target straight above: a gentle near-vertical stroke into its underside
   const x1=Math.max(t.left+14,Math.min(t.right-14,x0+24)),y1=t.bottom+10,cx=x0+34,cy=(y0+y1)/2;
   return [`M${x0} ${y0} Q ${cx} ${cy}, ${x1} ${y1}`,head(x1,y1,cx,cy)]}
  // one sweeping arc over the deck: land on the side of the loop that faces the word
  if(t.top>wr.top+wr.height*.5){// low target: rise from the word and curve up into the loop's underside
   const x1=t.left+t.width*.42,y1=t.bottom+14,cx=x1,cy=y0-6;
   return [`M${x0} ${y0} Q ${cx} ${cy}, ${x1} ${y1}`,head(x1,y1,cx,cy)]}
  const tcx=t.left+t.width/2,tcy=t.top+t.height/2,below=tcy<y0;
  const x1=tcx+(x0<tcx?-1:1)*Math.min(t.width/2+10,40),y1=t.bottom+12;
  const mx=(x0+x1)/2,my=(y0+y1)/2,dx=x1-x0,dy=y1-y0,L=Math.hypot(dx,dy)||1,nx=-dy/L,ny=dx/L,side=x1>x0?-1:1,bow=Math.min(160,L*.28);
  const cx=mx+nx*bow*side,cy=my+ny*bow*side;
  return [`M${x0} ${y0} Q ${cx} ${cy}, ${x1} ${y1}`,head(x1,y1,cx,cy)]}
 const mkP=(d,cls,delay)=>I3.add(d,cls,delay);
 function drawInk(i){const W=win.getBoundingClientRect();ink.setAttribute('viewBox',`0 0 ${W.width} ${W.height}`);const targets=[];
  (HL[i+1]||[]).forEach(q=>{const els=[...document.querySelectorAll(q.replace(/^union:/,''))];const rects=q.startsWith('union:')?[els.map(e=>e.getBoundingClientRect()).reduce((u,b)=>({left:Math.min(u.left,b.left),top:Math.min(u.top,b.top),right:Math.max(u.right,b.right),bottom:Math.max(u.bottom,b.bottom)}),{left:1e9,top:1e9,right:-1e9,bottom:-1e9})].map(u=>({left:u.left,top:u.top,width:u.right-u.left,height:u.bottom-u.top})):els.map(e=>e.getBoundingClientRect());rects.forEach(b=>{if(!b.width)return;
   targets.push(b);mkP(loop(b.left-W.left,b.top-W.top,b.width,b.height))})});
  // arrows: from the word in the narration, up to the loop around what it names
  const words=[...document.querySelectorAll('#x3 .x3narr span em, #x3 .x3narr span kbd')];
  (LINK[i+1]||[]).forEach(([wi,ti])=>{const w=words[wi],t=targets[ti];if(!w||!t)return;const a=w.getBoundingClientRect();
   const wb=document.querySelector('#x3 .x3win').getBoundingClientRect(),wr={left:wb.left-W.left,right:wb.right-W.left,top:wb.top-W.top,bottom:wb.bottom-W.top,width:wb.width,height:wb.height};
   const tt={left:t.left-W.left,right:t.left-W.left+t.width,top:t.top-W.top,bottom:t.top-W.top+t.height,width:t.width,height:t.height};
   arrow(a.left+a.width/2-W.left,a.top-6-W.top,tt,wr).forEach((d,k)=>{if(!k)mkP(d,'halo');mkP(d,'',k?420:0)})})}
 const hl=i=>I3.show(i,620);
 let last=-9;
 function put(i){if(i===last)return;last=i;hl(i);el.classList.add('out');setTimeout(()=>{el.innerHTML=T[i+1];el.classList.remove('out')},180)}
 const sec=document.getElementById('x3'),dots=[...document.querySelectorAll('#x3 #dots i')];
 const read=()=>{const n=dots.filter(d=>d.classList.contains('on')).length;const back=sec.classList.contains('back');put(n===0?-1:back?4:n-1)};
 new MutationObserver(read).observe(sec,{attributes:true,subtree:true,attributeFilter:['class']});read();
})();
