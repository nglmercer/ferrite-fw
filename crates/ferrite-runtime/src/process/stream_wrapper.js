import * as entry from __ENTRY__;
// Deliberately limited to the default-reader, pull-source subset needed by
// server renderers. BYOB, tee and pipe transforms fail explicitly.
if (typeof globalThis.ReadableStream === 'undefined') {
  globalThis.ReadableStream = class ReadableStream {
    constructor(source = {}, strategy = {}) {
      if (strategy.size !== undefined) throw new TypeError('custom stream size functions are unsupported');
      if (source.type !== undefined && source.type !== 'bytes') throw new TypeError('unsupported stream type');
      const state = this;
      this.source = source;
      this.queue = [];
      this.queuedBytes = 0;
      this.ended = false;
      this.failure = null;
      this.locked = false;
      this.pending = null;
      this.pulling = false;
      const mark = strategy.highWaterMark === undefined ? (source.type === 'bytes' ? 0 : 1) : strategy.highWaterMark;
      if (!Number.isFinite(mark) || mark < 0 || mark > 1048576) throw new RangeError('invalid stream highWaterMark');
      this.controller = {
        get desiredSize() { return state.failure ? null : state.ended ? 0 : mark - (source.type === 'bytes' ? state.queuedBytes : state.queue.length); },
        enqueue(value) {
          if (state.ended || state.failure) throw new TypeError('stream is closed');
          const size = typeof value === 'string' ? value.length * 4 : value.byteLength === undefined ? value.length : value.byteLength;
          if (!Number.isFinite(size) || size < 0 || size > 1048576 || state.queuedBytes + size > 1048576) {
            const error = new RangeError('stream source exceeded the 1 MiB queue bound');
            this.error(error); throw error;
          }
          if (state.pending) { const pending = state.pending; state.pending = null; pending.resolve({done:false,value}); }
          else { state.queue.push({value,size}); state.queuedBytes += size; }
        },
        close() { if (state.ended || state.failure) throw new TypeError('stream is closed'); state.ended = true; if (state.pending) { const pending = state.pending; state.pending = null; pending.resolve({done:true}); } },
        error(error) { if (state.failure) return; state.failure = error || new Error('stream failed'); state.queue = []; state.queuedBytes = 0; if (state.pending) { const pending = state.pending; state.pending = null; pending.reject(state.failure); } }
      };
      this.pump = () => {
        if (!state.pending || state.pulling || state.ended || state.failure || !source.pull) return;
        state.pulling = true;
        Promise.resolve().then(() => source.pull(state.controller)).then(() => {state.pulling=false;state.pump();}, error => {state.pulling=false;state.controller.error(error);});
      };
      this.started = Promise.resolve().then(() => source.start ? source.start(this.controller) : undefined).catch(error => this.controller.error(error));
    }
    getReader(options) {
      if (options !== undefined && options.mode !== undefined) throw new TypeError('BYOB readers are unsupported');
      if (this.locked) throw new TypeError('stream is locked');
      this.locked = true;
      const state = this;
      let released = false;
      return {
        async read() {
          if (released) throw new TypeError('reader lock released');
          if (state.failure) throw state.failure;
          if (state.queue.length) { const item = state.queue.shift(); state.queuedBytes -= item.size; return {done:false,value:item.value}; }
          if (state.ended) return {done:true};
          if (state.pending) throw new TypeError('concurrent stream reads are unsupported');
          return new Promise((resolve,reject) => {
            state.pending = {resolve,reject};
            state.started.then(() => state.pump());
          });
        },
        async cancel(reason) { if (released) throw new TypeError('reader lock released'); state.queue=[]; state.queuedBytes=0; if (!state.ended && !state.failure) state.controller.close(); if (state.source.cancel) await state.source.cancel(reason); },
        releaseLock() { if (state.pending) throw new TypeError('pending read'); released=true; state.locked=false; }
      };
    }
    tee() { throw new TypeError('stream tee is unsupported'); }
    pipeTo() { throw new TypeError('stream pipeTo is unsupported'); }
    pipeThrough() { throw new TypeError('stream pipeThrough is unsupported'); }
  };
}
let reader;
let pending = null;
let length = 0;
let offset = 0;
export async function start(...args) {
  const render = entry[__EXPORT__];
  if (typeof render !== 'function') throw new TypeError('stream renderer export must be a function');
  const result = await render(...args);
  const body = result && result.body !== undefined ? result.body : result;
  if (body && typeof body.getReader === 'function') reader = body.getReader();
  else if (body && typeof body[Symbol.asyncIterator] === 'function') { const iterator = body[Symbol.asyncIterator](); reader = {read: () => iterator.next()}; }
  else if (body && typeof body.next === 'function') reader = {read: () => body.next()};
  else throw new TypeError('stream renderer must return a ReadableStream or asynchronous iterator');
  return {html:'',status:result.status === undefined ? 200 : result.status,headers:result.headers === undefined ? [] : result.headers};
}
export async function next() {
  if (pending === null || offset >= length) {
    const item = await reader.read();
    if (item.done) return {done:true,hex:'',error:null};
    pending = item.value;
    offset = 0;
  }
  const chunk = __ferriteStreamBytes(pending, offset);
  length = chunk.length;
  offset += chunk.hex.length / 2;
  return {done:false,hex:chunk.hex,error:null};
}
