// Shared per document by native ESM; concurrent imports reuse in-flight CSS.
const pending = new Map();
function stylesheet(url) {
  if (pending.has(url)) return pending.get(url);
  const promise = new Promise((resolve, reject) => {
    let link = Array.from(document.querySelectorAll('link[rel="stylesheet"]')).find(link => link.href === url);
    if (link && link.sheet) { resolve(); return; }
    const owned = !link;
    if (!link) { link = document.createElement('link'); link.rel = 'stylesheet'; link.href = url; }
    function clear() { link.removeEventListener('load', loaded); link.removeEventListener('error', failed); }
    function loaded() { clear(); resolve(); }
    function failed() { clear(); if (owned) link.remove(); reject(new Error('Failed to load stylesheet: ' + url)); }
    link.addEventListener('load', loaded);
    link.addEventListener('error', failed);
    if (owned) document.head.appendChild(link);
  });
  pending.set(url, promise);
  promise.catch(() => { if (pending.get(url) === promise) pending.delete(url); });
  return promise;
}
export function preload(load, paths, importer) {
  return Promise.all(paths.map(path => stylesheet(new URL(path, importer).href))).then(load);
}
