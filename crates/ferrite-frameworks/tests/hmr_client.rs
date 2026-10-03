//! Execute the actual client with a controlled transport and file-backed ESM edits.
use std::time::Duration;
#[tokio::test]
#[ignore = "requires explicitly invoked Node execution"]
async fn actual_hmr_client_calls_previous_boundary_once_per_edit() {
    let dir = tempfile::tempdir().unwrap();
    let client = dir.path().join("client.mjs");
    std::fs::write(&client, include_str!("../../ferrite-hmr/src/client.js")).unwrap();
    let wrapper = dir.path().join("probe.mjs");
    std::fs::write(&wrapper, r#"
import {writeFileSync} from 'node:fs';
export async function probe({client, app}) {
  globalThis.location = {protocol: 'http:', host: 'fixture', reload() {throw new Error('unexpected reload');}};
  globalThis.document = {getElementById() {return null;}};
  const handlers = {};
  globalThis.WebSocket = class {addEventListener(event, callback) {handlers[event] = callback;}};
  await import(client);
  globalThis.history = [];
  globalThis.disposals = [];
  const source = (version) => `export const version = ${version};
    const hot = globalThis.__ferrite_create_hot__(${JSON.stringify(app)});
    hot.data.executions = (hot.data.executions || 0) + 1;
    hot.dispose(data => globalThis.disposals.push(data.executions));
    hot.accept(next => globalThis.history.push([${version}, next.version]));`;
  writeFileSync(new URL(app), source(0));
  await import(app);
  for (let version = 1; version <= 3; version++) {
    writeFileSync(new URL(app), source(version));
    await handlers.message({data: JSON.stringify({type: 'update', updates: [{type: 'js-update', path: app, acceptedPath: app, timestamp: version}]})});
  }
  // Both messages arrive before either dynamic import completes. Without
  // serialization the slower update calls the stale callback after the newer one.
  writeFileSync(new URL(app), `export const version = Number(new URL(import.meta.url).searchParams.get('t'));
    if (version === 4) await new Promise(resolve => setTimeout(resolve, 30));
    const hot = globalThis.__ferrite_create_hot__(${JSON.stringify(app)});
    hot.data.executions = (hot.data.executions || 0) + 1;
    hot.dispose(data => globalThis.disposals.push(data.executions));
    hot.accept(next => globalThis.history.push([version, next.version]));
    if (version === 5) hot.on('fixture:failure', () => { throw new Error('expected handler failure'); });`);
  const event = (timestamp) => ({data: JSON.stringify({type: 'update', updates: [{type: 'js-update', path: app, acceptedPath: app, timestamp}]})});
  await Promise.all([handlers.message(event(4)), handlers.message(event(5))]);
  let rejected = false;
  try { await handlers.message({data: JSON.stringify({type: 'custom', event: 'fixture:failure', data: null})}); }
  catch (error) { rejected = error.message === 'expected handler failure'; }
  await handlers.message(event(6));
  return {history: globalThis.history, disposals: globalThis.disposals, rejected};
}
"#).unwrap();
    let host = ferrite_plugin::node_adapter::NodeAdapterHost::spawn_with_timeout(
        None,
        Duration::from_secs(10),
    )
    .unwrap();
    host.register_plugin(
        "hmr-probe",
        url::Url::from_file_path(wrapper).unwrap().as_str(),
    )
    .unwrap();
    let result = host.call_export("hmr-probe", "probe", serde_json::json!({"client": url::Url::from_file_path(client).unwrap().as_str(), "app": url::Url::from_file_path(dir.path().join("App.mjs")).unwrap().as_str()})).await.unwrap();
    assert_eq!(
        result["history"],
        serde_json::json!([[0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 6]])
    );
    assert_eq!(result["disposals"], serde_json::json!([1, 2, 3, 4, 5, 6]));
    assert_eq!(result["rejected"], true);
    host.shutdown();
}
