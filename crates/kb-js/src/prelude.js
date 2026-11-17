// Browser-side JavaScript API, installed before any page script runs.
// Native operations live on __kbn (see crates/kb-js/src/lib.rs).
(function () {
  const N = globalThis.__kbn;
  const cache = new Map();
  const listeners = new Map();

  function wrap(id) {
    if (id === null || id === undefined || id < 0) return null;
    let w = cache.get(id);
    if (!w) {
      w = new Element(id);
      cache.set(id, w);
    }
    return w;
  }

  // Resolve a URL attribute against the page address (enough for script and image sources).
  function resolveUrl(value) {
    if (value === null) return '';
    const v = String(value);
    if (/^[a-z][a-z0-9+.-]*:/i.test(v)) return v;
    const base = String(globalThis.__kbUrl || '');
    if (v.startsWith('//')) return base.split(':')[0] + ':' + v;
    const origin = base.split('/').slice(0, 3).join('/');
    if (v.startsWith('/')) return origin + v;
    return base.split('?')[0].split('#')[0].replace(/[^/]*$/, '') + v;
  }

  class Element {
    constructor(id) { this.__id = id; }
    get tagName() { return N.tag(this.__id) || ''; }
    get nodeName() { return this.tagName; }
    get id() { return N.getAttr(this.__id, 'id') ?? ''; }
    set id(v) { N.setAttr(this.__id, 'id', String(v)); }
    get className() { return N.getAttr(this.__id, 'class') ?? ''; }
    set className(v) { N.setAttr(this.__id, 'class', String(v)); }
    get textContent() { return N.getText(this.__id) ?? ''; }
    set textContent(v) { N.setText(this.__id, String(v)); }
    get innerText() { return this.textContent; }
    set innerText(v) { this.textContent = v; }
    // Reading innerHTML returns text only (serialisation is not implemented).
    get innerHTML() { return this.textContent; }
    set innerHTML(v) { N.setHtml(this.__id, String(v)); }
    get parentNode() { return wrap(N.parent(this.__id)); }
    get nodeType() { return N.nodeType(this.__id); }
    get ownerDocument() { return globalThis.document; }
    // Named access to the attributes (attributes[name] is an Attr-like object, or undefined).
    get attributes() {
      const el = this;
      return new Proxy({}, {
        get(_t, k) {
          if (typeof k !== 'string') return undefined;
          const v = el.getAttribute(k);
          return v === null ? undefined : { name: k, nodeName: k, value: v, nodeValue: v };
        },
        has(_t, k) { return typeof k === 'string' && el.getAttribute(k) !== null; },
      });
    }
    get childNodes() { return N.childNodes(this.__id).map(wrap); }
    get firstChild() { return this.childNodes[0] ?? null; }
    get lastChild() { const c = this.childNodes; return c.length ? c[c.length - 1] : null; }
    get parentElement() { return this.parentNode; }
    get children() { return N.children(this.__id).map(wrap); }
    get firstElementChild() { return this.children[0] ?? null; }
    get lastElementChild() { const c = this.children; return c[c.length - 1] ?? null; }
    get style() {
      const id = this.__id;
      return new Proxy({}, {
        set(_t, k, v) {
          N.setStyle(id, String(k).replace(/[A-Z]/g, (m) => '-' + m.toLowerCase()), String(v));
          return true;
        },
        get(_t, k) {
          if (k === 'setProperty') return (p, v) => N.setStyle(id, String(p), String(v));
          return undefined;
        },
      });
    }
    // A missing attribute is null, as in the DOM (the host reports it as undefined).
    getAttribute(n) { const v = N.getAttr(this.__id, String(n)); return v === undefined || v === null ? null : v; }
    setAttribute(n, v) { N.setAttr(this.__id, String(n), String(v)); }
    // Script and image sources come back as absolute addresses, as they do in a browser.
    get src() { return resolveUrl(this.getAttribute('src')); }
    set src(v) { this.setAttribute('src', String(v)); }
    removeAttribute(n) { N.delAttr(this.__id, String(n)); }
    hasAttribute(n) { return N.getAttr(this.__id, String(n)) !== null; }
    // A document fragment is a detached holder; its children move into the parent when it is inserted.
    appendChild(c) {
      if (c.getAttribute('data-kb-fragment') !== null) { for (const k of N.children(c.__id)) N.append(this.__id, k); return c; }
      N.append(this.__id, c.__id); return c;
    }
    insertBefore(c, r) {
      if (c.getAttribute('data-kb-fragment') !== null) { for (const k of N.children(c.__id)) N.insertBefore(this.__id, k, r ? r.__id : -1); return c; }
      N.insertBefore(this.__id, c.__id, r ? r.__id : -1); return c;
    }
    // A copy of the element (its attributes and, when deep, its content), detached from the page.
    cloneNode(deep) { const id = N.clone(this.__id, !!deep); return id < 0 ? null : wrap(id); }
    removeChild(c) { N.remove(c.__id); return c; }
    remove() { N.remove(this.__id); }
    append(...nodes) {
      for (const n of nodes) this.appendChild(typeof n === 'string' ? document.createTextNode(n) : n);
    }
    querySelector(s) { return wrap(N.qs(String(s), this.__id)); }
    querySelectorAll(s) { return N.qsa(String(s), this.__id).map(wrap); }
    addEventListener(type, fn) {
      let m = listeners.get(this.__id);
      if (!m) { m = {}; listeners.set(this.__id, m); }
      (m[type] ||= []).push(fn);
    }
    removeEventListener(type, fn) {
      const m = listeners.get(this.__id);
      if (m && m[type]) m[type] = m[type].filter((f) => f !== fn);
    }
    dispatchEvent(ev) { dispatchTo(this.__id, ev); return !ev.defaultPrevented; }
    // Layout queries: this browser does not run a layout the page can read, so these report zero
    // size and no scrolling. Code that only checks for them, or that measures and continues, works.
    getBoundingClientRect() { return { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 }; }
    getClientRects() { return []; }
    get offsetWidth() { return 0; }
    get offsetHeight() { return 0; }
    get offsetTop() { return 0; }
    get offsetLeft() { return 0; }
    get clientWidth() { return 0; }
    get clientHeight() { return 0; }
    get scrollWidth() { return 0; }
    get scrollHeight() { return 0; }
    get scrollTop() { return 0; }
    set scrollTop(_v) {}
    get scrollLeft() { return 0; }
    set scrollLeft(_v) {}
    scrollIntoView() {}
    scrollTo() {}
    scrollBy() {}
    select() {}
    setSelectionRange() {}
    get selectionStart() { return 0; }
    get selectionEnd() { return 0; }
    // Two nodes are equal when they have the same type, name, attributes and children.
    isSameNode(other) { return !!other && other.__id === this.__id; }
    isEqualNode(other) {
      if (!other || other.nodeType !== this.nodeType) return false;
      if (this.nodeType === 3) return this.textContent === other.textContent;
      if (this.tagName !== other.tagName) return false;
      for (const a of ['id', 'class', 'name', 'type', 'src', 'href', 'rel', 'content', 'property', 'charset', 'nonce', 'value', 'style']) {
        if (this.getAttribute(a) !== other.getAttribute(a)) return false;
      }
      const ca = this.childNodes, cb = other.childNodes;
      if (ca.length !== cb.length) return false;
      return ca.every((c, i) => c.isEqualNode(cb[i]));
    }
    hasAttributes() { return this.id !== '' || this.getAttribute('class') !== null; }
    compareDocumentPosition(other) { return other === this ? 0 : 4; }
    getRootNode() { let n = this; while (n.parentNode) n = n.parentNode; return globalThis.document; }
    // Siblings and tree helpers (elements only: text nodes are not part of this tree).
    get nextSibling() { const p = this.parentNode; if (!p) return null; const k = p.childNodes; const i = k.findIndex((x) => x.__id === this.__id); return i >= 0 && i + 1 < k.length ? k[i + 1] : null; }
    get previousSibling() { const p = this.parentNode; if (!p) return null; const k = p.childNodes; const i = k.findIndex((x) => x.__id === this.__id); return i > 0 ? k[i - 1] : null; }
    get nodeValue() { return this.nodeType === 3 ? this.textContent : null; }
    set nodeValue(v) { if (this.nodeType === 3) this.textContent = String(v); }
    get data() { return this.nodeType === 3 ? this.textContent : undefined; }
    get isConnected() { return globalThis.document.documentElement.contains(this); }
    hasChildNodes() { return this.childNodes.length > 0; }
    prepend(...nodes) { const first = this.firstChild; for (const n of nodes) this.insertBefore(typeof n === 'string' ? globalThis.document.createTextNode(n) : n, first); }
    replaceChild(n, old) { this.insertBefore(n, old); this.removeChild(old); return old; }
    insertAdjacentElement(where, el) { if (where === 'beforebegin') this.parentNode.insertBefore(el, this); else if (where === 'afterbegin') this.insertBefore(el, this.firstChild); else if (where === 'beforeend') this.appendChild(el); else if (where === 'afterend') this.parentNode.insertBefore(el, this.nextSibling); return el; }
    setAttributeNS(_ns, n, v) { this.setAttribute(n, v); }
    removeAttributeNS(_ns, n) { this.removeAttribute(n); }
    getElementsByClassName(c) { return N.qsa('.' + String(c).trim().split(/\s+/).join('.'), this.__id).map(wrap); }
    getElementsByTagName(t) { return N.qsa(String(t), this.__id).map(wrap); }
    matches(sel) { return N.matches(this.__id, String(sel)); }
    closest(sel) { let el = this; while (el && el.__id !== undefined) { if (el.matches(sel)) return el; el = el.parentNode; } return null; }
    contains(other) { let n = other; while (n) { if (n.__id === this.__id) return true; n = n.parentNode; } return false; }
    get nextElementSibling() { return wrap(N.sibling(this.__id, 1)); }
    get previousElementSibling() { return wrap(N.sibling(this.__id, -1)); }
    get classList() {
      const el = this;
      const list = () => el.className.split(/\s+/).filter(Boolean);
      const set = (arr) => { el.className = arr.join(' '); };
      return {
        contains: (c) => list().includes(c),
        add: (...cs) => { const l = list(); for (const c of cs) if (!l.includes(c)) l.push(c); set(l); },
        remove: (...cs) => set(list().filter((x) => !cs.includes(x))),
        toggle: (c, force) => { const has = list().includes(c); const want = force === undefined ? !has : !!force; if (want && !has) el.classList.add(c); if (!want && has) el.classList.remove(c); return want; },
        get length() { return list().length; },
        item: (i) => list()[i] ?? null,
        toString: () => el.className,
      };
    }
    get dataset() {
      const el = this;
      return new Proxy({}, {
        get(_t, k) { return el.getAttribute('data-' + String(k).replace(/[A-Z]/g, (m) => '-' + m.toLowerCase())) ?? undefined; },
        set(_t, k, v) { el.setAttribute('data-' + String(k).replace(/[A-Z]/g, (m) => '-' + m.toLowerCase()), String(v)); return true; },
      });
    }
    get hidden() { return this.hasAttribute('hidden'); }
    set hidden(v) { if (v) this.setAttribute('hidden', ''); else this.removeAttribute('hidden'); }
    insertAdjacentHTML(pos, html) {
      const tmp = document.createElement('div');
      tmp.innerHTML = String(html);
      const nodes = tmp.children;
      for (const n of nodes.slice()) {
        if (pos === 'beforeend') this.appendChild(n);
        else if (pos === 'afterbegin') this.insertBefore(n, this.firstElementChild);
        else if (pos === 'beforebegin' && this.parentNode) this.parentNode.insertBefore(n, this);
        else if (pos === 'afterend' && this.parentNode) this.parentNode.insertBefore(n, this.nextElementSibling);
      }
    }
    click() { dispatch(this.__id, 'click'); }
    focus() {}
    blur() {}
  }

  globalThis.Element = Element;
  globalThis.HTMLElement = Element;

  const document = {
    // A Document: the node type and name that libraries check (jQuery and Sizzle need nodeType 9).
    nodeType: 9,
    nodeName: '#document',
    ownerDocument: null,
    compatMode: 'CSS1Compat',
    // The <script> being run; set by the host while a script executes.
    currentScript: null,
    // The page address and its parts, as pages read them.
    get URL() { return String(globalThis.__kbUrl || ''); },
    get documentURI() { return this.URL; },
    get domain() { return String(globalThis.location.hostname || ''); },
    referrer: '',
    get activeElement() { return this.body; },
    hasFocus() { return true; },
    get defaultView() { return globalThis; },
    get body() { return wrap(N.qs('body', -1)); },
    get head() { return wrap(N.qs('head', -1)); },
    get documentElement() { return wrap(N.qs('html', -1)); },
    get title() { const t = N.qs('title', -1); return t === null ? '' : (N.getText(t) ?? ''); },
    set title(v) { const t = N.qs('title', -1); if (t !== null) N.setText(t, String(v)); },
    get readyState() { return 'complete'; },
    getElementById(id) { return wrap(N.byId(String(id))); },
    querySelector(s) { return wrap(N.qs(String(s), -1)); },
    querySelectorAll(s) { return N.qsa(String(s), -1).map(wrap); },
    getElementsByTagName(t) { return N.qsa(String(t), -1).map(wrap); },
    getElementsByName(n) { return N.qsa(`[name="${String(n)}"]`, -1).map(wrap); },
    getElementsByClassName(c) { return N.qsa('.' + String(c).trim().split(/\s+/).join('.'), -1).map(wrap); },
    createElement(tag) { return wrap(N.create(String(tag))); },
    createTextNode(t) { return wrap(N.text(String(t))); },
    createElementNS(_ns, tag) { return wrap(N.create(String(tag))); },
    createComment(_t) { return wrap(N.create('template')); },
    importNode(node, deep) { return node.cloneNode(!!deep); },
    createDocumentFragment() { const f = wrap(N.create('template')); f.setAttribute('data-kb-fragment', ''); return f; },
    addEventListener(type, fn) { wrap(N.qs('html', -1)).addEventListener(type, fn); },
  };
  globalThis.document = document;

  // ---- events -------------------------------------------------------------
  // Returns 0: no listener on the path, 1: listeners ran, 2: a listener called preventDefault.
  function dispatch(id, type) {
    return dispatchTo(id, {
      type,
      target: null,
      currentTarget: null,
      defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; },
      stopPropagation() {},
    });
  }
  // An event listener is a function, or an object with a handleEvent method (as the DOM allows).
  function invokeListener(fn, thisArg, ev) {
    if (typeof fn === 'function') return fn.call(thisArg, ev);
    if (fn && typeof fn.handleEvent === 'function') return fn.handleEvent(ev);
    return undefined;
  }
  // Deliver `ev` to the listeners of node `id` and its ancestors (bubbling order).
  function dispatchTo(id, ev) {
    const path = [];
    for (let cur = id; cur !== null && cur >= 0; cur = N.parent(cur)) path.push(cur);
    ev.target = wrap(id);
    let ran = false;
    for (const n of path) {
      const m = listeners.get(n);
      if (!m || !m[ev.type]) continue;
      ran = true;
      ev.currentTarget = wrap(n);
      for (const fn of m[ev.type]) {
        try { invokeListener(fn, ev.currentTarget, ev); } catch (e) { console.error(e && e.stack ? String(e.stack).split('\n').slice(0, 3).join(' | ') : String(e)); }
      }
    }
    if (!ran) return 0;
    return ev.defaultPrevented ? 2 : 1;
  }

  // ---- timers (run after page scripts, within a budget) --------------------
  const timers = [];
  let seq = 0;
  globalThis.setTimeout = (fn, ms, ...args) => {
    const t = { id: ++seq, fn, args, due: Number(ms) || 0 };
    timers.push(t);
    return t.id;
  };
  // setInterval: pages are rendered on demand, not animated, so an interval does not keep a timer running.
  // It fires INTERVAL_RUNS times while the page settles (a page that polls for something sees several
  // ticks), and clearInterval stops it. The bound keeps a page from holding the device busy.
  const INTERVAL_RUNS = 3;
  const cancelled = new Set();
  globalThis.setInterval = (fn, ms, ...args) => {
    const t = { id: ++seq, fn, args, due: Number(ms) || 0, every: Math.max(Number(ms) || 0, 1), runs: INTERVAL_RUNS };
    timers.push(t);
    return t.id;
  };
  globalThis.clearTimeout = (id) => {
    cancelled.add(id);
    const i = timers.findIndex((t) => t.id === id);
    if (i >= 0) timers.splice(i, 1);
  };
  globalThis.clearInterval = globalThis.clearTimeout;
  globalThis.requestAnimationFrame = (fn) => globalThis.setTimeout(fn, 16);

  // ---- network (synchronous under the hood, Promise-shaped) ------------------
  globalThis.fetch = (url, opts) => new Promise((resolve, reject) => {
    const o = opts || {};
    const method = String(o.method || 'GET');
    const flat = [];
    const hdrs = o.headers || {};
    for (const k of Object.keys(hdrs)) flat.push(String(k), String(hdrs[k]));
    let payload;
    if (o.body !== undefined && o.body !== null) {
      payload = typeof o.body === 'string' ? o.body : (typeof o.body === 'object' ? JSON.stringify(o.body) : String(o.body));
    }
    // Relative addresses resolve against the page, as they do in a browser.
    const r = N.httpRequest(method, resolveUrl(String(url)), flat, payload); // [status, finalUrl, body] or null
    if (r === null || r === undefined) return reject(new TypeError('fetch failed: ' + url));
    const status = Number(r[0]);
    const text = r[2];
    const contentType = r[3] || '';
    resolve({
      ok: status >= 200 && status < 300,
      status,
      statusText: '',
      url: r[1],
      headers: {
        get: (name) => (String(name).toLowerCase() === 'content-type' ? contentType : null),
        has: (name) => String(name).toLowerCase() === 'content-type' && contentType !== '',
      },
      text: () => Promise.resolve(text),
      json: () => Promise.resolve().then(() => JSON.parse(text)),
    });
  });

  // ---- storage, console, window -------------------------------------------
  // localStorage is kept per site in the device's data folder, so it survives a restart.
  const storageOrigin = (() => { const m = /^(https?:\/\/[^/?#]*)/.exec(globalThis.__kbUrl || ''); return m ? m[1] : 'internal'; })();
  let saved = {};
  try { saved = JSON.parse(N.storageLoad(storageOrigin) || '{}'); } catch (e) { saved = {}; }
  const store = new Map(Object.entries(saved));
  const saveStore = () => {
    if (!N.storageSave(storageOrigin, JSON.stringify(Object.fromEntries(store)))) {
      store.clear();
      throw new Error('QuotaExceededError: localStorage is full');
    }
  };
  globalThis.localStorage = {
    getItem: (k) => (store.has(String(k)) ? store.get(String(k)) : null),
    setItem: (k, v) => { store.set(String(k), String(v)); saveStore(); },
    removeItem: (k) => { store.delete(String(k)); saveStore(); },
    clear: () => { store.clear(); saveStore(); },
    key: (i) => Array.from(store.keys())[i] ?? null,
    get length() { return store.size; },
  };
  const log = (...a) => N.log(a.map(String).join(' '));
  // An Error prints with its first stack frames, so a logged failure shows where it happened.
  const describe = (a) => (a && a.stack ? String(a.stack).split('\n').slice(0, 3).join(' | ') : String(a));
  globalThis.console = { log, info: log, debug: log, warn: log, error: (...a) => N.log('error: ' + a.map(describe).join(' ')) };
  globalThis.window = globalThis;
  globalThis.self = globalThis;
  globalThis.scrollX = 0; globalThis.scrollY = 0; globalThis.pageXOffset = 0; globalThis.pageYOffset = 0;
  globalThis.scrollTo = () => {}; globalThis.scrollBy = () => {};
  globalThis.navigator = { userAgent: 'Mozilla/5.0 (Linux; Kindle; U) PrimitiveBrowser/0.1', language: 'en', onLine: true };
  const href = globalThis.__kbUrl;
  let parsed = { hostname: '', protocol: '', pathname: '/' };
  const m = /^(https?:)\/\/([^/?#]*)([^?#]*)/.exec(href);
  if (m) parsed = { hostname: m[2].split(':')[0], protocol: m[1], pathname: m[3] || '/' };
  // Script navigation: setting location.href, calling location.replace() or assign(), or assigning
  // window.location asks the browser to load the address once the script returns. While the browser
  // itself updates the location (copyLocation), the href setter only records the new value.
  let currentHref = href;
  let silentLocation = false;
  const requestNavigation = (u) => N.navigate(resolveUrl(String(u)));
  const locationObject = Object.assign({ origin: href.split('/').slice(0, 3).join('/') }, parsed);
  Object.defineProperty(locationObject, 'href', {
    enumerable: true, configurable: true,
    get() { return currentHref; },
    set(v) { if (silentLocation) currentHref = String(v); else requestNavigation(v); },
  });
  locationObject.assign = requestNavigation;
  locationObject.replace = requestNavigation;
  locationObject.reload = () => requestNavigation(currentHref);
  Object.defineProperty(globalThis, 'location', {
    enumerable: true, configurable: true,
    get() { return locationObject; },
    set(v) { requestNavigation(v); },
  });
  // A page's own window is its top and parent window, so window.parent.location works.
  globalThis.parent = globalThis;
  globalThis.top = globalThis;

  // ---- standard browser APIs that pages expect -----------------------------
  // Each is implemented enough for common code paths; anything heavier is stubbed.
  class EventTargetLite {
    constructor() { this.__l = new Map(); }
    addEventListener(type, fn, opts) {
      if (!fn) return;
      const list = this.__l.get(type) || [];
      if (!list.some((x) => x.fn === fn)) list.push({ fn, once: !!(opts && opts.once) });
      this.__l.set(type, list);
    }
    removeEventListener(type, fn) {
      const list = this.__l.get(type);
      if (list) this.__l.set(type, list.filter((x) => x.fn !== fn));
    }
    dispatchEvent(ev) {
      const list = this.__l.get(ev.type) || [];
      for (const x of list.slice()) {
        try { invokeListener(x.fn, this, ev); } catch (e) { console.error(e && e.stack ? String(e.stack).split('\n').slice(0, 3).join(' | ') : String(e)); }
        if (x.once) this.removeEventListener(ev.type, x.fn);
      }
      return !ev.defaultPrevented;
    }
  }
  globalThis.EventTarget = EventTargetLite;
  class Event {
    constructor(type, init) { this.type = String(type); this.bubbles = !!(init && init.bubbles); this.defaultPrevented = false; this.target = null; }
    preventDefault() { this.defaultPrevented = true; }
    stopPropagation() {}
    stopImmediatePropagation() {}
  }
  class CustomEvent extends Event {
    constructor(type, init) { super(type, init); this.detail = init && init.detail !== undefined ? init.detail : null; }
  }
  globalThis.Event = Event;
  globalThis.CustomEvent = CustomEvent;
  // window and document act as event targets, so load/DOMContentLoaded listeners work.
  const winEvents = new EventTargetLite();
  for (const k of ['addEventListener', 'removeEventListener', 'dispatchEvent']) {
    globalThis[k] = winEvents[k].bind(winEvents);
    document[k] = winEvents[k].bind(winEvents);
  }
  // Going back or forward to an entry that pushState created on this page: the page hears popstate with the
  // state it pushed, and the document is not loaded again.
  globalThis.__kbPopState = (json) => {
    let state = null;
    try { state = JSON.parse(json); } catch (e) { state = null; }
    globalThis.history.state = state;
    const ev = new Event('popstate');
    ev.state = state;
    winEvents.dispatchEvent(ev);
  };
  globalThis.__fireLoadEvents = () => {
    const ev = (t) => { const e = new Event(t); e.target = document; winEvents.dispatchEvent(e); };
    ev('DOMContentLoaded');
    ev('load');
  };

  class AbortSignal extends EventTargetLite {
    constructor() { super(); this.aborted = false; this.reason = undefined; this.onabort = null; }
    throwIfAborted() { if (this.aborted) throw this.reason; }
  }
  class AbortController {
    constructor() { this.signal = new AbortSignal(); }
    abort(reason) {
      if (this.signal.aborted) return;
      this.signal.aborted = true;
      this.signal.reason = reason !== undefined ? reason : new Error('AbortError');
      const e = new Event('abort');
      if (typeof this.signal.onabort === 'function') this.signal.onabort(e);
      this.signal.dispatchEvent(e);
    }
  }
  globalThis.AbortController = AbortController;
  globalThis.AbortSignal = AbortSignal;

  // crypto: values from Math.random (not cryptographically strong; no secrets should rely on it).
  const rand = (n) => { const a = new Uint8Array(n); for (let i = 0; i < n; i++) a[i] = Math.floor(Math.random() * 256); return a; };
  globalThis.crypto = {
    getRandomValues(arr) { const r = rand(arr.byteLength); new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength).set(r); return arr; },
    randomUUID() {
      const b = rand(16); b[6] = (b[6] & 0x0f) | 0x40; b[8] = (b[8] & 0x3f) | 0x80;
      const h = Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
      return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
    },
    subtle: {},
  };

  // Typed arrays and text codecs (ASCII/UTF-8 subset).
  if (typeof globalThis.TextEncoder === 'undefined') {
    globalThis.TextEncoder = class { encode(str = '') { const out = []; for (const ch of String(str)) { let c = ch.codePointAt(0); if (c < 0x80) out.push(c); else if (c < 0x800) out.push(0xc0 | (c >> 6), 0x80 | (c & 63)); else if (c < 0x10000) out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63)); else out.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 63), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63)); } return new Uint8Array(out); } };
  }
  if (typeof globalThis.TextDecoder === 'undefined') {
    globalThis.TextDecoder = class { decode(buf) { let s = ''; const b = new Uint8Array(buf || []); for (let i = 0; i < b.length;) { const c = b[i]; let cp, n; if (c < 0x80) { cp = c; n = 1; } else if (c < 0xe0) { cp = ((c & 31) << 6) | (b[i + 1] & 63); n = 2; } else if (c < 0xf0) { cp = ((c & 15) << 12) | ((b[i + 1] & 63) << 6) | (b[i + 2] & 63); n = 3; } else { cp = ((c & 7) << 18) | ((b[i + 1] & 63) << 12) | ((b[i + 2] & 63) << 6) | (b[i + 3] & 63); n = 4; } s += String.fromCodePoint(cp); i += n; } return s; } };
  }

  // URL and URLSearchParams.
  class URLSearchParams {
    constructor(init) {
      this.__p = [];
      if (typeof init === 'string') {
        const s = init.startsWith('?') ? init.slice(1) : init;
        for (const part of s.split('&')) {
          if (!part) continue;
          const [k, v = ''] = part.split('=');
          this.__p.push([decodeURIComponent(k.replace(/\+/g, ' ')), decodeURIComponent(v.replace(/\+/g, ' '))]);
        }
      } else if (init) {
        for (const [k, v] of Object.entries(init)) this.__p.push([k, String(v)]);
      }
    }
    get(k) { const e = this.__p.find((x) => x[0] === k); return e ? e[1] : null; }
    getAll(k) { return this.__p.filter((x) => x[0] === k).map((x) => x[1]); }
    has(k) { return this.__p.some((x) => x[0] === k); }
    set(k, v) { const i = this.__p.findIndex((x) => x[0] === k); if (i >= 0) { this.__p[i][1] = String(v); this.__p = this.__p.filter((x, j) => x[0] !== k || j === i); } else this.append(k, v); }
    append(k, v) { this.__p.push([String(k), String(v)]); }
    delete(k) { this.__p = this.__p.filter((x) => x[0] !== k); }
    toString() { return this.__p.map(([k, v]) => encodeURIComponent(k) + '=' + encodeURIComponent(v)).join('&'); }
    forEach(fn) { for (const [k, v] of this.__p) fn(v, k, this); }
    [Symbol.iterator]() { return this.__p[Symbol.iterator](); }
  }
  class URLLite {
    constructor(url, base) {
      let s = String(url);
      if (base !== undefined && !/^[a-z][a-z0-9+.-]*:/i.test(s)) {
        const b = new URLLite(base);
        if (s.startsWith('//')) s = b.protocol + s;
        else if (s.startsWith('/')) s = b.origin + s;
        else if (s.startsWith('?')) s = b.origin + b.pathname + s;
        else if (s.startsWith('#')) s = b.origin + b.pathname + b.search + s;
        else s = b.origin + b.pathname.replace(/[^/]*$/, '') + s;
      }
      const m = /^([a-z][a-z0-9+.-]*:)\/\/([^/?#]*)([^?#]*)(\?[^#]*)?(#.*)?$/i.exec(s);
      if (!m) throw new TypeError('Invalid URL: ' + url);
      this.protocol = m[1].toLowerCase();
      this.host = m[2];
      this.hostname = m[2].split(':')[0];
      this.port = m[2].includes(':') ? m[2].split(':')[1] : '';
      this.pathname = m[3] || '/';
      this.search = m[4] || '';
      this.hash = m[5] || '';
      this.origin = this.protocol + '//' + this.host;
      this.searchParams = new URLSearchParams(this.search);
    }
    get href() { return this.origin + this.pathname + (this.searchParams.toString() ? '?' + this.searchParams.toString() : '') + this.hash; }
    toString() { return this.href; }
    toJSON() { return this.href; }
  }
  globalThis.URL = URLLite;
  // Object.assign copies own properties only; href is a getter on URLLite, so set it explicitly.
  const copyLocation = (u) => {
    silentLocation = true;
    try { Object.assign(globalThis.location, u, { href: u.href }); } finally { silentLocation = false; }
  };
  try { copyLocation(new URLLite(href)); } catch (_) {}
  globalThis.URLSearchParams = URLSearchParams;

  // Timing and scheduling.
  const t0 = Date.now();
  globalThis.performance = {
    now: () => Date.now() - t0, timeOrigin: t0,
    mark() {}, measure() {}, clearMarks() {}, clearMeasures() {}, clearResourceTimings() {},
    getEntries: () => [], getEntriesByName: () => [], getEntriesByType: () => [],
    toJSON() { return { timeOrigin: t0 }; },
  };
  globalThis.queueMicrotask = (fn) => Promise.resolve().then(fn);
  globalThis.requestIdleCallback = (fn) => setTimeout(() => fn({ didTimeout: false, timeRemaining: () => 10 }), 1);
  globalThis.cancelIdleCallback = (id) => clearTimeout(id);
  globalThis.cancelAnimationFrame = (id) => clearTimeout(id);
  globalThis.structuredClone = (v) => JSON.parse(JSON.stringify(v));
  globalThis.btoa = (s) => { const T = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'; let out = ''; const str = String(s); for (let i = 0; i < str.length; i += 3) { const a = str.charCodeAt(i), b = str.charCodeAt(i + 1), c = str.charCodeAt(i + 2); const n = (a << 16) | ((b || 0) << 8) | (c || 0); out += T[(n >> 18) & 63] + T[(n >> 12) & 63] + (i + 1 < str.length ? T[(n >> 6) & 63] : '=') + (i + 2 < str.length ? T[n & 63] : '='); } return out; };
  globalThis.atob = (s) => { const T = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'; const str = String(s).replace(/=+$/, ''); let out = ''; let buf = 0, bits = 0; for (const ch of str) { buf = (buf << 6) | T.indexOf(ch); bits += 6; if (bits >= 8) { bits -= 8; out += String.fromCharCode((buf >> bits) & 255); } } return out; };

  // Streams: enough of ReadableStream for code that only checks for it or reads whole bodies.
  class ReadableStream {
    constructor(src) { this.__src = src; this.locked = false; }
    getReader() { return { read: () => Promise.resolve({ done: true, value: undefined }), releaseLock() {}, cancel: () => Promise.resolve() }; }
    cancel() { return Promise.resolve(); }
  }
  globalThis.ReadableStream = ReadableStream;
  globalThis.WritableStream = class { constructor() {} getWriter() { return { write() { return Promise.resolve(); }, close() { return Promise.resolve(); }, releaseLock() {} }; } };

  // IntersectionObserver: pages use it to reveal content as it scrolls into view. Pages
  // are laid out in full here, so every observed element counts as visible (reported once,
  // asynchronously, like a real observer's first callback).
  class IntersectionObserverLite {
    constructor(cb, opts) { this.cb = cb; this.opts = opts || {}; this.targets = []; }
    observe(el) {
      if (!el) return;
      this.targets.push(el);
      setTimeout(() => {
        const rect = { x: 0, y: 0, width: 0, height: 0, top: 0, left: 0, right: 0, bottom: 0 };
        const entries = [{ target: el, isIntersecting: true, intersectionRatio: 1, boundingClientRect: rect, intersectionRect: rect, rootBounds: null, time: 0 }];
        try { this.cb(entries, this); } catch (e) { console.error(e && e.stack ? String(e.stack).split('\n').slice(0, 3).join(' | ') : String(e)); }
      }, 0);
    }
    unobserve(el) { this.targets = this.targets.filter((t) => t !== el); }
    disconnect() { this.targets = []; }
    takeRecords() { return []; }
  }
  globalThis.IntersectionObserver = IntersectionObserverLite;

  // XMLHttpRequest as a thin wrapper over fetch: the request is sent by send(), and readystatechange, load,
  // error and loadend fire in the order browsers use.
  class XMLHttpRequestLite {
    constructor() {
      this.readyState = 0; this.status = 0; this.statusText = ''; this.responseText = ''; this.response = '';
      this.responseURL = ''; this.responseType = ''; this.onreadystatechange = null; this.onload = null;
      this.onerror = null; this.onloadend = null;
      this._headers = {}; this._listeners = {}; this._respHeaders = null; this._aborted = false;
    }
    open(method, url) { this._method = String(method || 'GET'); this._url = String(url); this._aborted = false; this._setState(1); }
    setRequestHeader(name, value) { this._headers[String(name)] = String(value); }
    getResponseHeader(name) { return this._respHeaders ? (this._respHeaders[String(name).toLowerCase()] ?? null) : null; }
    getAllResponseHeaders() { return ''; }
    addEventListener(type, fn) { (this._listeners[type] = this._listeners[type] || []).push(fn); }
    removeEventListener(type, fn) { this._listeners[type] = (this._listeners[type] || []).filter((f) => f !== fn); }
    abort() { this._aborted = true; this.status = 0; this._setState(this.readyState === 0 ? 0 : 4); }
    _emit(type) {
      const ev = { type, target: this };
      const handler = this['on' + type];
      if (typeof handler === 'function') handler.call(this, ev);
      for (const fn of this._listeners[type] || []) fn.call(this, ev);
    }
    _setState(state) { this.readyState = state; this._emit('readystatechange'); }
    send(body) {
      if (this.readyState !== 1) throw new Error('InvalidStateError: open() must be called first');
      const init = { method: this._method, headers: this._headers };
      if (body !== undefined && body !== null) init.body = body;
      fetch(this._url, init).then((res) => res.text().then((text) => {
        if (this._aborted) return;
        this.status = res.status; this.statusText = res.statusText || ''; this.responseURL = res.url;
        this.responseText = text; this.response = text;
        this._respHeaders = { 'content-type': res.headers.get('content-type') || '' };
        this._setState(2); this._setState(3); this._setState(4);
        this._emit('load'); this._emit('loadend');
      }), () => {
        if (this._aborted) return;
        this.status = 0; this._setState(4); this._emit('error'); this._emit('loadend');
      });
    }
  }
  XMLHttpRequestLite.UNSENT = 0; XMLHttpRequestLite.OPENED = 1; XMLHttpRequestLite.HEADERS_RECEIVED = 2;
  XMLHttpRequestLite.LOADING = 3; XMLHttpRequestLite.DONE = 4;
  globalThis.XMLHttpRequest = XMLHttpRequestLite;

  // MutationObserver (basic): attribute and child-list changes made through Element.prototype methods are
  // recorded, and each observer's callback runs once with its records, in a microtask. Text and innerHTML
  // writes are not recorded.
  const observers = [];
  let deliveryQueued = false;
  const isWithin = (node, root) => { for (let n = node; n; n = n.parentNode) if (n === root) return true; return false; };
  const deliver = () => {
    deliveryQueued = false;
    for (const o of observers.slice()) {
      if (!o._records.length) continue;
      const records = o._records;
      o._records = [];
      try { o._cb.call(o, records, o); } catch (e) { N.log('error: MutationObserver callback: ' + e); }
    }
  };
  const recordMutation = (rec) => {
    for (const o of observers) {
      const hit = o._targets.find((t) => {
        if (rec.type === 'attributes' && !t.attributes) return false;
        if (rec.type === 'childList' && !t.childList) return false;
        if (rec.type === 'attributes' && t.attributeFilter && !t.attributeFilter.includes(rec.attributeName)) return false;
        return t.target === rec.target || (t.subtree && isWithin(rec.target, t.target));
      });
      if (!hit) continue;
      o._records.push(rec);
      if (!deliveryQueued) { deliveryQueued = true; Promise.resolve().then(deliver); }
    }
  };
  const elementProto = Element.prototype;
  const setAttributeNative = elementProto.setAttribute;
  elementProto.setAttribute = function (name, value) {
    const oldValue = this.getAttribute(name);
    setAttributeNative.call(this, name, value);
    recordMutation({ type: 'attributes', target: this, attributeName: String(name), oldValue });
  };
  const appendChildNative = elementProto.appendChild;
  elementProto.appendChild = function (child) {
    const result = appendChildNative.call(this, child);
    recordMutation({ type: 'childList', target: this, addedNodes: [child], removedNodes: [] });
    return result;
  };
  const removeChildNative = elementProto.removeChild;
  elementProto.removeChild = function (child) {
    const result = removeChildNative.call(this, child);
    recordMutation({ type: 'childList', target: this, addedNodes: [], removedNodes: [child] });
    return result;
  };
  class MutationObserverLite {
    constructor(callback) { this._cb = callback; this._targets = []; this._records = []; }
    observe(target, options) {
      const o = options || {};
      const attributes = !!o.attributes || o.attributeFilter !== undefined;
      this._targets = this._targets.filter((t) => t.target !== target);
      this._targets.push({ target, subtree: !!o.subtree, childList: !!o.childList, attributes, attributeFilter: o.attributeFilter });
      if (!observers.includes(this)) observers.push(this);
    }
    disconnect() { this._targets = []; this._records = []; const i = observers.indexOf(this); if (i >= 0) observers.splice(i, 1); }
    takeRecords() { const r = this._records; this._records = []; return r; }
  }
  globalThis.MutationObserver = MutationObserverLite;

  // Observers that never fire: pages fall back to their static content.
  class NoopObserver { constructor(cb) { this.cb = cb; } observe() {} unobserve() {} disconnect() {} takeRecords() { return []; } }
  globalThis.ResizeObserver = NoopObserver;
  globalThis.PerformanceObserver = NoopObserver;

  // Element classes for feature checks and `instanceof` (for example Next.js checks HTMLScriptElement).
  // An element is an instance when its tag matches, so `el instanceof HTMLScriptElement` works.
  const elementClass = (name, matches) => {
    const C = { [name]: class extends Element {} }[name];
    Object.defineProperty(C, Symbol.hasInstance, { value: (o) => o instanceof Element && matches(String(o.tagName).toUpperCase()) });
    return C;
  };
  if (!globalThis.Element) globalThis.Element = Element;
  if (!globalThis.Node) globalThis.Node = Element;
  if (!globalThis.HTMLElement) globalThis.HTMLElement = elementClass('HTMLElement', () => true);
  for (const [name, tag] of [
    ['HTMLScriptElement', 'SCRIPT'], ['HTMLImageElement', 'IMG'], ['HTMLAnchorElement', 'A'],
    ['HTMLDivElement', 'DIV'], ['HTMLSpanElement', 'SPAN'], ['HTMLInputElement', 'INPUT'],
    ['HTMLFormElement', 'FORM'], ['HTMLButtonElement', 'BUTTON'], ['HTMLLinkElement', 'LINK'],
    ['HTMLIFrameElement', 'IFRAME'], ['HTMLTextAreaElement', 'TEXTAREA'], ['HTMLSelectElement', 'SELECT'],
    ['HTMLStyleElement', 'STYLE'], ['HTMLTemplateElement', 'TEMPLATE'], ['HTMLCanvasElement', 'CANVAS'],
    ['HTMLVideoElement', 'VIDEO'], ['HTMLAudioElement', 'AUDIO'],
    ['SVGElement', 'SVG'],
  ]) {
    if (!globalThis[name]) globalThis[name] = elementClass(name, (t) => t === tag);
  }

  // Character data nodes: Text and its base class, matched by node type (3 is text).
  const nodeClass = (name, matches) => {
    const C = { [name]: class extends Element {} }[name];
    Object.defineProperty(C, Symbol.hasInstance, { value: (o) => o instanceof Element && matches(o.nodeType) });
    return C;
  };
  if (!globalThis.CharacterData) globalThis.CharacterData = nodeClass('CharacterData', (t) => t === 3 || t === 8);
  if (!globalThis.Text) globalThis.Text = nodeClass('Text', (t) => t === 3);
  if (!globalThis.Comment) globalThis.Comment = nodeClass('Comment', (t) => t === 8);
  if (!globalThis.DocumentType) globalThis.DocumentType = nodeClass('DocumentType', (t) => t === 10);

  // IndexedDB is not available on this device. The constructors exist so feature checks and
  // references do not throw; opening a database fails asynchronously, the way a closed one does.
  for (const name of ['IDBDatabase', 'IDBIndex', 'IDBObjectStore', 'IDBTransaction', 'IDBRequest', 'IDBCursor', 'IDBKeyRange']) {
    if (!globalThis[name]) globalThis[name] = class {};
  }
  if (!globalThis.indexedDB) {
    const refuse = () => {
      const req = { result: undefined, error: null, readyState: 'pending', onsuccess: null, onerror: null };
      setTimeout(() => {
        req.readyState = 'done';
        req.error = new Error('IndexedDB is not available on this device');
        if (typeof req.onerror === 'function') req.onerror({ target: req });
      }, 0);
      return req;
    };
    globalThis.indexedDB = { open: refuse, deleteDatabase: refuse, cmp: () => 0 };
  }

  // Layout queries used for feature checks.
  const cssWidth = globalThis.__kbCssWidth || 412;
  globalThis.innerWidth = cssWidth;
  globalThis.innerHeight = globalThis.__kbCssHeight || 800;
  globalThis.outerWidth = cssWidth;
  globalThis.devicePixelRatio = globalThis.__kbDevicePixelRatio || 1;
  // Media queries: comma-separated lists (any part may match), "and" between features, "not" and "only",
  // and the features a Kindle page can answer: size, orientation, colour scheme, and pointer/hover.
  const mediaPx = (v) => (/(em|rem)$/.test(v) ? parseFloat(v) * 16 : parseFloat(v));
  const mediaFeature = (f) => {
    const m = /^\s*([a-z-]+)\s*(?::\s*(.+?))?\s*$/.exec(f);
    if (!m) return false;
    const name = m[1];
    const value = m[2] === undefined ? undefined : m[2].toLowerCase();
    const h = globalThis.innerHeight;
    switch (name) {
      case 'min-width': return cssWidth >= mediaPx(value);
      case 'max-width': return cssWidth <= mediaPx(value);
      case 'width': return cssWidth === mediaPx(value);
      case 'min-height': return h >= mediaPx(value);
      case 'max-height': return h <= mediaPx(value);
      case 'height': return h === mediaPx(value);
      case 'orientation': return value === (cssWidth >= h ? 'landscape' : 'portrait');
      case 'prefers-color-scheme': return value === 'light';
      case 'prefers-reduced-motion': return value === 'reduce' ? false : true;
      case 'hover': case 'any-hover': return value === 'none';
      case 'pointer': case 'any-pointer': return value === 'none';
      case 'display-mode': return value === 'browser';
      default: return false;
    }
  };
  const mediaMatches = (q) => q.split(',').some((part) => {
    let s = part.trim().toLowerCase();
    let negate = false;
    if (s.startsWith('not ')) { negate = true; s = s.slice(4).trim(); }
    if (s.startsWith('only ')) s = s.slice(5).trim();
    let ok = true;
    for (const t of s.split(/\s+and\s+/)) {
      const token = t.trim();
      if (!token || token === 'all' || token === 'screen') continue;
      if (token.startsWith('(') && token.endsWith(')')) { if (!mediaFeature(token.slice(1, -1))) { ok = false; break; } continue; }
      ok = false;
      break;
    }
    return negate ? !ok : ok;
  });
  globalThis.matchMedia = (q) => ({ matches: mediaMatches(String(q)), media: String(q), onchange: null, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} });
  globalThis.getComputedStyle = (el) => new Proxy({}, { get: (_t, k) => (typeof k === 'string' ? '' : undefined) });
  globalThis.Image = class { constructor(w, h) { this.width = w || 0; this.height = h || 0; this.src = ''; this.onload = null; } };

  // History (single document: navigation stays in the browser).
  // pushState and replaceState change the address shown (not the page): the browser takes the new address after the script.
  const applyHistoryUrl = (url, replace, state) => {
    if (url === undefined || url === null) return;
    const abs = resolveUrl(String(url));
    let json = 'null';
    try { json = JSON.stringify(state === undefined ? null : state) || 'null'; } catch (e) { json = 'null'; }
    N.setUrl(abs, replace, json);
    copyLocation(new URLLite(abs));
  };
  globalThis.history = {
    length: 1,
    state: null,
    pushState(st, title, url) { this.state = st; applyHistoryUrl(url, false, st); },
    replaceState(st, title, url) { this.state = st; applyHistoryUrl(url, true, st); },
    back() {}, forward() {}, go() {},
  };

  // Cookies: read/write through the browser's jar on the next request (approximate).
  let cookieStore = '';
  Object.defineProperty(document, 'cookie', {
    get() { return cookieStore; },
    set(v) { const kv = String(v).split(';')[0]; if (kv) cookieStore = (cookieStore ? cookieStore + '; ' : '') + kv; },
  });

  // sessionStorage mirrors localStorage in this single-document browser.
  globalThis.sessionStorage = globalThis.localStorage;

  // ---- entry points used by the host ---------------------------------------
  globalThis.__kb = {
    setLocation: (url) => copyLocation(new URLLite(url)),
    setCurrentScript: (id) => { document.currentScript = id < 0 ? null : wrap(id); },
    click: (id) => dispatch(id, 'click'),
    event: (id, type) => dispatch(id, String(type)),
    popState: (json) => globalThis.__kbPopState(String(json)),
    runTimers: () => {
      let n = 0;
      while (timers.length && n < 500) {
        timers.sort((a, b) => a.due - b.due || a.id - b.id);
        const t = timers.shift();
        n++;
        try { t.fn(...t.args); } catch (e) { console.error(e && e.stack ? String(e.stack).split('\n').slice(0, 3).join(' | ') : String(e)); }
        // An interval that has runs left and was not cleared is scheduled again, one period later.
        if (t.runs !== undefined && --t.runs > 0 && !cancelled.has(t.id)) {
          t.due += t.every;
          timers.push(t);
        }
      }
      return n;
    },
  };
})();
