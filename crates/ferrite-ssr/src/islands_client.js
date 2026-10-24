// `registry` is generated from configured literal module imports by the host.
const active = new WeakMap();
const disposers = new WeakMap();
const schedules = new WeakMap();
function dispose(el) {
  const cancel = schedules.get(el); if (cancel) {schedules.delete(el);cancel();}
  const stop = disposers.get(el); disposers.delete(el);
  active.delete(el); scheduled.delete(el);
  if (stop) {
    const report = error => el.dispatchEvent(new CustomEvent('ferrite:island-error',{bubbles:true,detail:error}));
    try {Promise.resolve(stop()).catch(report);} catch (error) {report(error);}
  }
}
export function hydrateIsland(el) {
  if (active.has(el)) return active.get(el);
  const work = Promise.resolve().then(async () => {
    if (!el.isConnected) return;
    if (el.parentElement?.closest('[data-ferrite-island]')) throw new Error('Nested island ownership is unsupported');
    const name = el.getAttribute('data-ferrite-island');
    if (!Object.hasOwn(registry, name)) throw new Error('Unregistered island: ' + name);
    const props = JSON.parse(el.getAttribute('data-props') || '{}');
    el.setAttribute('data-ferrite-state', 'loading');
    const mod = await registry[name]();
    if (!el.isConnected || active.get(el) !== work) return;
    if (typeof mod.hydrate !== 'function') throw new Error('Island must export hydrate(element, props): ' + name);
    const stop = await mod.hydrate(el, props);
    if (!el.isConnected || active.get(el) !== work) {if (typeof stop === 'function') stop();return;}
    if (typeof stop === 'function') disposers.set(el, stop);
  const cancel = schedules.get(el); if (cancel) {schedules.delete(el);cancel();}
    el.setAttribute('data-ferrite-state', 'ready');
    el.dispatchEvent(new CustomEvent('ferrite:island-ready', {bubbles:true}));
  }).catch(error => {
    active.delete(el);
    el.setAttribute('data-ferrite-state', 'error');
    el.dispatchEvent(new CustomEvent('ferrite:island-error', {bubbles:true, detail:error}));
    throw error;
  });
  active.set(el, work);
  return work;
}
const scheduled = new WeakSet();
function schedule(el) {
  if (scheduled.has(el)) return;
  scheduled.add(el);
  el.setAttribute('data-ferrite-state','pending');
  const run = () => hydrateIsland(el).catch(() => {});
  const mode = el.getAttribute('data-ferrite-hydrate') || 'load';
  if (mode === 'manual') return;
  if (mode === 'load') { run(); return; }
  if (mode === 'idle') {const handle=(globalThis.requestIdleCallback || (fn=>setTimeout(fn,1)))(run); schedules.set(el,()=> (globalThis.cancelIdleCallback || clearTimeout)(handle));return;}
  if (mode === 'visible') {
    if (!globalThis.IntersectionObserver) { run(); return; }
    const observer = new IntersectionObserver(entries => { if (entries.some(entry=>entry.isIntersecting)) {observer.disconnect();run();} });
    schedules.set(el,()=>observer.disconnect());observer.observe(el);return;
  }
  if (mode === 'interaction') {
    const trigger = event => {
      const target = event.target;
      if (event.type === 'click') { event.preventDefault(); event.stopImmediatePropagation(); }
      hydrateIsland(el).then(() => {
        el.removeEventListener('click',trigger,true);el.removeEventListener('focusin',trigger,true);
        if (event.type === 'click' && target.isConnected) target.dispatchEvent(new MouseEvent('click',{bubbles:true,cancelable:true,button:event.button,ctrlKey:event.ctrlKey,shiftKey:event.shiftKey,altKey:event.altKey,metaKey:event.metaKey}));
      }).catch(()=>{});
    };
    schedules.set(el,()=>{el.removeEventListener('click',trigger,true);el.removeEventListener('focusin',trigger,true);});
    el.addEventListener('click',trigger,true);el.addEventListener('focusin',trigger,true);return;
  }
  el.setAttribute('data-ferrite-state','error');
  el.dispatchEvent(new CustomEvent('ferrite:island-error',{bubbles:true,detail:new Error('Unknown hydration strategy: '+mode)}));
}
function scan(root) {
  if (root.nodeType !== 1 && root.nodeType !== 9) return;
  if (root.matches?.('[data-ferrite-island]')) schedule(root);
  for (const el of root.querySelectorAll('[data-ferrite-island]')) schedule(el);
}
scan(document);
new MutationObserver(records => {
  for (const record of records) {
    for (const node of record.addedNodes) scan(node);
    for (const node of record.removedNodes) {
      if (node.nodeType !== 1) continue;
      if (!node.isConnected) dispose(node);
      for (const el of node.querySelectorAll('[data-ferrite-island]')) if (!el.isConnected) dispose(el);
    }
  }
}).observe(document.documentElement,{childList:true,subtree:true});
document.addEventListener('ferrite:hydrate',event=>{if(event.target.matches?.('[data-ferrite-island]')) hydrateIsland(event.target).catch(()=>{});});
