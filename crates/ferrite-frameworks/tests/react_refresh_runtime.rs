//! Execute generated footer against the official runtime; not renderer/state conformance.
use ferrite_frameworks::react::{preamble_code, refresh_footer};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires explicit Node compiler profile and actual registry package"]
async fn official_refresh_runtime_validates_export_boundaries() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    let npm = root.join(".ferrite/npm");
    let registry =
        ferrite_npm::RegistryClient::new(ferrite_npm::DEFAULT_REGISTRY, npm.join("metadata"))
            .unwrap();
    let installer = ferrite_npm::Installer::new(registry, npm);
    let manifest = ferrite_npm::JsPackageJson {
        dependencies: [("react-refresh".into(), "0.17.0".into())].into(),
        ..Default::default()
    };
    let mut lock = ferrite_npm::Lockfile::default();
    installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    let runtime = root
        .join(".ferrite/npm/packages")
        .join(lock.find("react-refresh").unwrap().id())
        .join("runtime.js");
    let runtime_url = url::Url::from_file_path(runtime).unwrap().to_string();
    let preamble = root.join("preamble.mjs");
    std::fs::write(
        &preamble,
        preamble_code().replace(
            "\"react-refresh/runtime\"",
            &serde_json::to_string(&runtime_url).unwrap(),
        ),
    )
    .unwrap();
    let app = root.join("App.mjs");
    let app_url = url::Url::from_file_path(&app).unwrap().to_string();
    let footer = refresh_footer(&app_url, &[("App".into(), "boundary App".into())])
        .replace(
            "\"/@react-refresh\"",
            &serde_json::to_string(url::Url::from_file_path(preamble).unwrap().as_str()).unwrap(),
        )
        .replace(
            "\"react-refresh/runtime\"",
            &serde_json::to_string(&runtime_url).unwrap(),
        );
    std::fs::write(
        &app,
        format!("export function App() {{ return null; }}\nexport const constant = 7;\n{footer}"),
    )
    .unwrap();
    let wrapper = root.join("probe.mjs");
    std::fs::write(&wrapper, r#"
export async function probe({app, runtime}) {
  globalThis.window = globalThis;
  let callback;
  let invalidations = 0;
  globalThis.__ferrite_create_hot__ = () => ({accept(fn) { callback = fn; }, invalidate() { invalidations++; }});
  const previous = await import(app);
  if (typeof callback !== 'function') throw new Error('footer did not accept');
  const Runtime = (await import(runtime)).default;
  const family = Runtime.getFamilyByType(previous.App);
  if (!family || family.current !== previous.App) throw new Error('component was not registered');
  function NextApp() { return null; }
  globalThis.$RefreshReg$(NextApp, 'boundary App');
  callback({App: NextApp, constant: 7});
  if (family.current !== NextApp) throw new Error('runtime refresh did not advance family');
  if (invalidations !== 0) throw new Error('unchanged mixed exports invalidated');
  const cases = [
    {App: NextApp},
    {App: NextApp, constant: 7, extra: 1},
    {App: NextApp, constant: 8},
    {App: 42, constant: 7},
    {},
    Object.defineProperty({constant: 7}, 'App', {enumerable: true, get() { throw new Error('getter'); }}),
  ];
  for (const next of cases) {
    const before = invalidations;
    callback(next);
    if (invalidations !== before + 1) throw new Error('unsafe export boundary accepted');
  }
  callback(null);
  return {invalidations, app: typeof previous.App, preamble: globalThis.__ferrite_react_preamble_installed__};
}
"#).unwrap();
    let host = ferrite_plugin::node_adapter::NodeAdapterHost::spawn_with_timeout(
        None,
        Duration::from_secs(10),
    )
    .unwrap();
    host.register_plugin(
        "refresh-probe",
        url::Url::from_file_path(wrapper).unwrap().as_str(),
    )
    .unwrap();
    let result = host
        .call_export(
            "refresh-probe",
            "probe",
            serde_json::json!({"app": app_url, "runtime": runtime_url}),
        )
        .await
        .unwrap();
    assert_eq!(result["invalidations"], 6);
    assert_eq!(result["app"], "function");
    assert_eq!(result["preamble"], true);
    host.shutdown();
}
