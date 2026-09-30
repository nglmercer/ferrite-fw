(id, source, argument, interval, remote) => {
  const key = Symbol.for('ferrite.functionWaits');
  const tasks = globalThis[key] || (globalThis[key] = new Map());
  if (tasks.has(id)) return true;
  const state = {status: 'pending', active: true, timer: null, value: undefined,
    stop() {
      this.active = false;
      if (this.timer !== null) {
        if (interval === null) cancelAnimationFrame(this.timer);
        else clearTimeout(this.timer);
      }
      this.timer = null;
      this.value = undefined;
    }};
  tasks.set(id, state);
  const fail = error => {
    if (!state.active) return;
    state.status = 'error';
    state.error = String(error?.stack || error);
    state.active = false;
  };
  const tick = () => {
    state.timer = null;
    if (!state.active) return;
    let candidate;
    try {
      const value = (0, eval)('(' + source + ')');
      candidate = typeof value === 'function' ? value(argument) : value;
    } catch (error) { fail(error); return; }
    // Playwright tests the predicate's immediate return, so a Promise is
    // truthy even when its eventual value is false. Await it for the result.
    const truthy = Boolean(candidate);
    Promise.resolve(candidate).then(value => {
      if (!state.active) return;
      if (truthy) {
        try {
          if (remote) state.value = value;
          else {
            state.json = JSON.stringify(value);
            if (state.json === undefined) throw new Error('function wait value is not JSON serializable; use wait_for_function_handle');
          }
          state.status = 'ready';
          state.active = false;
        } catch (error) { fail(error); }
      } else {
        state.timer = interval === null ? requestAnimationFrame(tick) : setTimeout(tick, interval);
      }
    }, fail);
  };
  tick();
  return true;
}
