// Official Svelte compiler wrapper executed inside Ferrite's embedded JS runtime.
// The compiler input is the upstream self-contained UMD compiler bundle; this
// wrapper must not use Node builtins.
import '/svelte-compiler.mjs';

const compiler = globalThis.svelte;
if (!compiler || typeof compiler.compile !== 'function') throw new Error('embedded Svelte compiler did not initialize');

function clone(value) { return value ? JSON.parse(JSON.stringify(value)) : null; }

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
function decodeValue(text, state) {
  let result = 0;
  let shift = 0;
  for (;;) {
    if (state.index >= text.length) throw new Error('invalid source-map VLQ segment');
    const digit = B64.indexOf(text[state.index++]);
    if (digit < 0) throw new Error('invalid source-map VLQ digit');
    result += (digit & 31) << shift;
    if (!(digit & 32)) break;
    shift += 5;
  }
  const negative = result & 1;
  result >>= 1;
  return negative ? -result : result;
}
function encodeValue(value) {
  let output = '';
  let data = Math.abs(value) * 2 + (value < 0 ? 1 : 0);
  do {
    let digit = data % 32;
    data = Math.floor(data / 32);
    if (data) digit += 32;
    output += B64[digit];
  } while (data);
  return output;
}
function decodeMappings(mappings) {
  return mappings.split(';').map(line => {
    if (!line) return [];
    return line.split(',').map(text => {
      const segment = [];
      const state = { index: 0 };
      while (state.index < text.length) segment.push(decodeValue(text, state));
      return segment;
    });
  });
}
function encodeMappings(lines) {
  return lines.map(line => line.map(segment => segment.map(encodeValue).join('')).join(',')).join(';');
}

// ECMA-426 DecodeMappingsField: invalid original positions are null, with
// a reported diagnostic. Never clamp them to fabricated source coordinates.
function svelteMap(value, request, diagnostics) {
  const json = clone(value);
  if (!json) return null;
  if (typeof json.mappings !== 'string') return json;
  const lines = decodeMappings(json.mappings);
  let invalid = 0;
  for (const line of lines) {
    for (let index = 0; index < line.length; index++) {
      const segment = line[index];
      if (segment.length > 1 && (segment[2] < 0 || segment[3] < 0)) {
        line[index] = [segment[0]];
        invalid++;
      }
    }
  }
  if (invalid) {
    json.mappings = encodeMappings(lines);
    diagnostics.push({ severity: 'warning', code: 'invalid_original_source_map_position',
      message: `Svelte ${request.version} emitted ${invalid} negative original source-map positions; those positions are unmapped per ECMA-426, and valid mappings are retained`,
      filename: request.filename, start: null });
  }
  return json;
}

export async function compile(request) {
  const { filename, source, target, development } = request;
  const server = target === 'server';
  const result = request.module
    ? compiler.compileModule(source, { filename, generate: target, dev: development })
    : compiler.compile(source, { filename, generate: target, dev: development, css: 'external', hmr: development && !server });
  const diagnostics = (result.warnings || []).map(warning => ({ severity: 'warning', code: warning.code, message: warning.message, filename: warning.filename || filename, start: warning.start || null }));
  const jsMap = svelteMap(result.js.map, request, diagnostics);
  const cssMap = result.css ? svelteMap(result.css.map, request, diagnostics) : null;
  return { pieces: [{ code: result.js.code, map: jsMap }], language: 'js', css: result.css ? [{ id: 'style-0', code: result.css.code, map: cssMap, modules: null }] : [], dependencies: [], diagnostics, compilerVersion: compiler.VERSION };
}
