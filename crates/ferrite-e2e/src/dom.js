(() => {
  const normalize = value => String(value ?? '').replace(/\s+/g, ' ').trim();
  function query(root, selector) {
    const result = [...root.querySelectorAll(selector)];
    for (const element of root.querySelectorAll('*')) {
      if (element.shadowRoot) result.push(...query(element.shadowRoot, selector));
    }
    return [...new Set(result)];
  }
  function ariaHidden(element) {
    for (let node = element; node; node = node.parentElement || node.getRootNode()?.host) {
      const style = getComputedStyle(node);
      if (node.hidden || node.getAttribute('aria-hidden') === 'true' || style.display === 'none' || style.visibility === 'hidden' || style.visibility === 'collapse') return true;
    }
    return false;
  }
  function hidden(element) {
    const box = element.getBoundingClientRect();
    return ariaHidden(element) || box.width <= 0 || box.height <= 0;
  }
  const disabled = element => element.matches(':disabled') || !!element.closest('[aria-disabled="true"]');
  function name(element, visited = new Set()) {
    if (!element || visited.has(element)) return '';
    visited.add(element);
    const ids = element.getAttribute('aria-labelledby');
    if (ids) {
      const values = ids.split(/\s+/).map(id => element.getRootNode().getElementById?.(id) || document.getElementById(id)).filter(Boolean);
      if (values.length) return normalize(values.map(node => name(node, visited)).join(' '));
    }
    if (element.hasAttribute('aria-label')) return normalize(element.getAttribute('aria-label'));
    if (element.labels?.length) return normalize([...element.labels].map(label => label.textContent).join(' '));
    if (element.matches('img,area,input[type="image"]')) return normalize(element.getAttribute('alt'));
    if (element.matches('input[type="button"],input[type="submit"],input[type="reset"]')) return normalize(element.value || ({submit: 'Submit', reset: 'Reset'}[element.type] || ''));
    const text = normalize(element.textContent || element.shadowRoot?.textContent);
    return text || normalize(element.getAttribute('title'));
  }
  function description(element) {
    const ids = element.getAttribute('aria-describedby');
    if (ids) return normalize(ids.split(/\s+/).map(id => (element.getRootNode().getElementById?.(id) || document.getElementById(id))?.textContent || '').join(' '));
    return normalize(element.getAttribute('aria-description') || element.getAttribute('title'));
  }
  function role(element) {
    const explicit = element.getAttribute('role')?.split(/\s+/)[0];
    if (explicit) return explicit;
    const tag = element.tagName.toLowerCase();
    if (tag === 'a' || tag === 'area') return element.hasAttribute('href') ? 'link' : '';
    if (tag === 'input') {
      const type = element.type;
      return ({button:'button',submit:'button',reset:'button',image:'button',checkbox:'checkbox',radio:'radio',range:'slider',number:'spinbutton',search:'searchbox',hidden:'',password:''})[type] ?? (element.hasAttribute('list') ? 'combobox' : 'textbox');
    }
    if (tag === 'select') return element.multiple || element.size > 1 ? 'listbox' : 'combobox';
    if (/^h[1-6]$/.test(tag)) return 'heading';
    return ({button:'button',textarea:'textbox',img:'img',ul:'list',ol:'list',li:'listitem',table:'table',tr:'row',td:'cell',th:'columnheader',option:'option',progress:'progressbar',meter:'meter',dialog:'dialog',nav:'navigation',main:'main',article:'article',aside:'complementary',summary:'button',hr:'separator',output:'status'})[tag] || '';
  }
  function matches(value, wanted, exact = false, regex = null) {
    value = normalize(value);
    if (regex != null) return new RegExp(regex, 'u').test(value);
    wanted = normalize(wanted);
    return exact ? value === wanted : value.toLowerCase().includes(wanted.toLowerCase());
  }
  function box(element) {
    const bounds = element.getBoundingClientRect();
    let x = bounds.x, y = bounds.y, current = window;
    try {
      while (current.frameElement) {
        const frame = current.frameElement, offset = frame.getBoundingClientRect();
        x += offset.x + frame.clientLeft; y += offset.y + frame.clientTop;
        current = current.parent;
      }
    } catch (_) { /* Cross-origin coordinates require driver-side frame offsets. */ }
    return { x, y, width: bounds.width, height: bounds.height };
  }
  function receives(element) {
    const bounds = element.getBoundingClientRect();
    const x = Math.max(0, Math.min(innerWidth - 1, bounds.x + bounds.width / 2));
    const y = Math.max(0, Math.min(innerHeight - 1, bounds.y + bounds.height / 2));
    let hit = document.elementFromPoint(x, y);
    while (hit?.shadowRoot) {
      const child = hit.shadowRoot.elementFromPoint(x, y);
      if (!child || child === hit) break;
      hit = child;
    }
    return !!hit && (element === hit || element.contains(hit));
  }
  function aria(root) {
    if (root instanceof Element && ariaHidden(root)) return [];
    const children = [...(root.children || []), ...(root.shadowRoot?.children || [])].flatMap(aria);
    const elementRole = root instanceof Element ? role(root) : '';
    if (!elementRole || elementRole === 'none' || elementRole === 'presentation') return children;
    const node = { role: elementRole, name: name(root) };
    if (children.length) node.children = children;
    if (disabled(root)) node.disabled = true;
    for (const key of ['checked', 'expanded', 'pressed', 'selected']) {
      const value = root.getAttribute('aria-' + key);
      if (value != null) node[key] = value === 'true' ? true : value === 'false' ? false : value;
    }
    if (root.matches('input[type=checkbox],input[type=radio]')) node.checked = root.checked;
    if (/^h[1-6]$/.test(root.tagName.toLowerCase())) node.level = +root.tagName[1];
    return [node];
  }
  function render(nodes, depth = 0) {
    return nodes.flatMap(node => {
      let line = '  '.repeat(depth) + '- ' + node.role + (node.name ? ' ' + JSON.stringify(node.name) : '');
      for (const key of ['level', 'checked', 'expanded', 'pressed', 'selected', 'disabled']) if (key in node) line += ' [' + key + (node[key] === true ? '' : '=' + node[key]) + ']';
      return [line, ...(node.children ? render(node.children, depth + 1) : [])];
    });
  }
  return { normalize, query, hidden, ariaHidden, disabled, name, description, role, matches, box, receives, aria, render };
})()
