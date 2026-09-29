(() => {
  if (window.__ferriteClock) return true;
  const native = { date: Date, setTimeout, clearTimeout, setInterval, clearInterval,
    raf: requestAnimationFrame, caf: cancelAnimationFrame,
    ric: window.requestIdleCallback, cic: window.cancelIdleCallback,
    perf: performance.now.bind(performance) };
  let now = native.date.now(), elapsed = 0, fixed = null, seq = 1, paused = true;
  const timers = new Map();
  const asFn = cb => typeof cb === 'function' ? cb : () => Function(String(cb))();
  function FakeDate(...args) {
    if (!new.target) return new native.date(fixed ?? now).toString();
    return args.length ? new native.date(...args) : new native.date(fixed ?? now);
  }
  FakeDate.now = () => Math.floor(fixed ?? now);
  FakeDate.parse = native.date.parse; FakeDate.UTC = native.date.UTC;
  FakeDate.prototype = native.date.prototype;
  window.Date = FakeDate;
  const schedule = (cb, ms, args, repeat) => {
    const id = seq++; const delay = Math.max(0, Number(ms) || 0);
    timers.set(id, { time: elapsed + delay, cb: asFn(cb), args, repeat: repeat ? Math.max(1, delay) : 0 }); return id;
  };
  window.setTimeout = (cb, ms = 0, ...args) => schedule(cb, ms, args, false);
  window.setInterval = (cb, ms = 0, ...args) => schedule(cb, ms, args, true);
  window.clearTimeout = window.clearInterval = id => timers.delete(id);
  window.requestAnimationFrame = cb => schedule(() => cb(elapsed), 16, [], false);
  window.cancelAnimationFrame = id => timers.delete(id);
  window.requestIdleCallback = (cb, options = {}) => schedule(() => cb({ didTimeout: false, timeRemaining: () => 50 }), Math.min(options.timeout ?? 1, 1), [], false);
  window.cancelIdleCallback = id => timers.delete(id);
  performance.now = () => elapsed;
  let wall = native.perf(), running = false;
  async function tick(ms, jump = false) {
    const end = elapsed + Math.max(0, Number(ms) || 0); let fired = 0;
    if (jump) { now += end - elapsed; elapsed = end; }
    for (;;) {
      let best = 0, bestTime = Infinity;
      for (const [id, timer] of timers) if (timer.time <= end && timer.time < bestTime) { best = id; bestTime = timer.time; }
      if (!best) break;
      if (++fired > 10000) throw new Error('clock tick exceeded 10000 timers (infinite timer loop?)');
      const timer = timers.get(best); timers.delete(best);
      if (!jump) { now += timer.time - elapsed; elapsed = timer.time; }
      if (timer.repeat) timers.set(best, { ...timer, time: elapsed + timer.repeat });
      timer.cb(...timer.args);
      await Promise.resolve(); // Promise callbacks can schedule the next timer.
    }
    now += end - elapsed; elapsed = end; wall = native.perf(); return Math.floor(now);
  }
  const heartbeat = native.setInterval.call(window, async () => {
    if (paused || running) { wall = native.perf(); return; }
    running = true;
    try { await tick(native.perf() - wall); } finally { running = false; }
  }, 10);
  window.__ferriteClock = {
    setFixed(ms) { fixed = Number(ms); return Math.floor(now); },
    setSystem(ms) { now = Number(ms); fixed = null; return Math.floor(now); },
    now() { return Math.floor(fixed ?? now); },
    pause() { paused = true; return Math.floor(now); },
    resume() { wall = native.perf(); paused = false; return Math.floor(now); },
    isPaused() { return paused; }, tick,
    async pauseAt(ms) { paused = true; return await tick(Math.max(0, Number(ms) - now), true); },
    fastForward(ms) { return tick(ms, true); },
    uninstall() {
      native.clearInterval.call(window, heartbeat);
      window.Date = native.date; window.setTimeout = native.setTimeout; window.clearTimeout = native.clearTimeout;
      window.setInterval = native.setInterval; window.clearInterval = native.clearInterval;
      window.requestAnimationFrame = native.raf; window.cancelAnimationFrame = native.caf;
      window.requestIdleCallback = native.ric; window.cancelIdleCallback = native.cic;
      performance.now = native.perf; delete window.__ferriteClock; return true;
    }
  };
  return true;
})()
