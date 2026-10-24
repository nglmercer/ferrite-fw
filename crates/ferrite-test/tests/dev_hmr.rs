//! Watched SSR application hydration, HMR and syntax recovery in real browsers.
#![cfg(feature = "napi-vm")]
use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
use std::time::Duration;
async fn matrix(kind: BrowserKind) {
    for framework in ["react", "svelte", "vue"] {
        for language in ["js", "ts"] {
            let mut files = ferrite::frameworks::scaffold::files(
                ferrite::frameworks::scaffold::select(framework, language, "ssr").unwrap(),
                "dev-hmr",
            )
            .unwrap();
            let extension = if framework == "react" {
                if language == "ts" {
                    "tsx"
                } else {
                    "jsx"
                }
            } else {
                language
            };
            nested_components(&mut files, framework, language, extension);
            *files.get_mut("index.html").unwrap()=files["index.html"].replace("<script type=","<script>globalThis.__ssrButton=document.querySelector('#counter');</script><script type=");
            files
                .get_mut(&format!("src/main.{extension}"))
                .unwrap()
                .push_str("\nglobalThis.__hydrated=true;\n");
            let inputs: Vec<_> = files
                .iter()
                .map(|(path, source)| (path.as_str(), source.as_str()))
                .collect();
            let project = ferrite_test::TempProject::new(&inputs);
            let npm = project.root.join(".ferrite/npm");
            let installer = ferrite::npm::Installer::new(
                ferrite::npm::RegistryClient::new(
                    ferrite::npm::DEFAULT_REGISTRY,
                    npm.join("metadata"),
                )
                .unwrap(),
                npm,
            );
            let mut lock = ferrite::npm::Lockfile::default();
            installer
                .install_project(&project.root, &mut lock, false)
                .await
                .unwrap();
            lock.write(&project.root.join("ferrite.lock")).unwrap();
            ferrite::npm::project_editor_dependencies(&project.root, &lock).unwrap();
            let (config, plugins) = ferrite::Config {
                root: Some(project.root.clone()),
                ..Default::default()
            }
            .resolve()
            .await
            .unwrap();
            let server = ferrite::DevServer::new(config.clone(), plugins)
                .await
                .unwrap();
            let shell = server.transform_index_html("/index.html").await.unwrap();
            server
                .set_ssr_adapter(
                    ferrite::create_dev_ssr_adapter(&server, &config, &shell)
                        .await
                        .unwrap(),
                )
                .await;
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/", listener.local_addr().unwrap());
            let router = server.router();
            struct Serving(tokio::task::JoinHandle<()>);
            impl Drop for Serving {
                fn drop(&mut self) {
                    self.0.abort();
                }
            }
            let _serving = Serving(tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            }));
            let executable = match kind {
                BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
                BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
            }
            .expect("both real browser executables are required");
            let browser = Browser::launch(
                LaunchOptions::default()
                    .browser(kind)
                    .executable(executable),
            )
            .await
            .unwrap();
            let page = browser.new_page().await.unwrap();
            page.goto(&url).await.unwrap();
            page.wait_for_function("globalThis.__hydrated===true", Duration::from_secs(15))
                .await
                .unwrap_or_else(|error| {
                    panic!(
                        "{framework}/{language}/{kind:?}: {error}; {:?}; {:?}",
                        page.page_errors(),
                        page.console_messages()
                    )
                });
            assert!(page
                .evaluate::<bool>("globalThis.__ssrButton===document.querySelector('#counter')")
                .await
                .unwrap());
            assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                "document.querySelector('#counter').textContent==='count: 1'",
                Duration::from_secs(5),
            )
            .await
            .unwrap();
            assert_eq!(
                page.locator("#parent-value")
                    .text_content()
                    .await
                    .unwrap()
                    .unwrap(),
                "parent: 1"
            );
            page.locator("#nested-counter").click().await.unwrap();
            page.wait_for_function(
                "document.querySelector('#nested-counter').textContent==='nested: 1'",
                Duration::from_secs(5),
            )
            .await
            .unwrap();
            page.evaluate::<bool>("globalThis.__hmrMarker=true")
                .await
                .unwrap();
            let component = if framework != "react" {
                format!("src/App.{framework}")
            } else {
                format!("src/App.{extension}")
            };
            let original = &files[&component];
            let updated = original.replace("Hello Ferrite +", "Updated");
            std::fs::write(project.root.join(&component), &updated).unwrap();
            page.wait_for_function(
                "document.querySelector('h1')?.textContent.startsWith('Updated')",
                Duration::from_secs(15),
            )
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{framework}/{language}/{kind:?}: {error}; {:?}; {:?}",
                    page.page_errors(),
                    page.console_messages()
                )
            });
            assert!(
                page.evaluate::<bool>("globalThis.__hmrMarker===true")
                    .await
                    .unwrap(),
                "component edit reloaded the document"
            );
            let expected = if framework != "svelte" {
                "count: 1"
            } else {
                "count: 0"
            };
            assert_eq!(
                page.locator("#counter")
                    .text_content()
                    .await
                    .unwrap()
                    .unwrap(),
                expected,
                "{framework}/{language}: state/reset contract"
            );
            let style = if framework != "react" {
                component.clone()
            } else {
                "src/style.css".into()
            };
            let current = std::fs::read_to_string(project.root.join(&style)).unwrap();
            assert!(current.contains("rgb(128, 0, 0)"));
            std::fs::write(
                project.root.join(&style),
                current.replace("rgb(128, 0, 0)", "rgb(0, 0, 128)"),
            )
            .unwrap();
            page.wait_for_function(
                "getComputedStyle(document.querySelector('#counter')).color==='rgb(0, 0, 128)'",
                Duration::from_secs(15),
            )
            .await
            .unwrap();
            assert!(page
                .evaluate::<bool>("globalThis.__hmrMarker===true")
                .await
                .unwrap());
            let child = if framework == "react" {
                format!("src/Child.{extension}")
            } else {
                format!("src/Child.{framework}")
            };
            let current = std::fs::read_to_string(project.root.join(&child)).unwrap();
            std::fs::write(
                project.root.join(&child),
                current.replace("nested:", "updated nested:"),
            )
            .unwrap();
            page.wait_for_function("document.querySelector('#nested-counter')?.textContent.startsWith('updated nested:')", Duration::from_secs(15)).await.unwrap();
            assert!(
                page.evaluate::<bool>("globalThis.__hmrMarker===true")
                    .await
                    .unwrap(),
                "nested component edit reloaded document"
            );
            if framework == "vue" {
                let current = std::fs::read_to_string(project.root.join(&component)).unwrap();
                assert!(current.contains("(0)"));
                std::fs::write(
                    project.root.join(&component),
                    current.replacen("(0)", "(5)", 1),
                )
                .unwrap();
                page.wait_for_function(
                    "document.querySelector('#counter')?.textContent==='count: 5'",
                    Duration::from_secs(15),
                )
                .await
                .unwrap();
                assert!(
                    page.evaluate::<bool>("globalThis.__hmrMarker===true")
                        .await
                        .unwrap(),
                    "Vue script edit reloaded document"
                );
            }
            let recovered = std::fs::read_to_string(project.root.join(&component)).unwrap();
            std::fs::write(
                project.root.join(&component),
                if framework != "react" {
                    "<script>const broken=;</script>"
                } else {
                    "export default function App( {"
                },
            )
            .unwrap();
            page.wait_for_function(
                "!!document.querySelector('#ferrite-error-overlay')",
                Duration::from_secs(15),
            )
            .await
            .unwrap();
            std::fs::write(project.root.join(&component), recovered).unwrap();
            page.wait_for_function("!document.querySelector('#ferrite-error-overlay') && document.querySelector('h1')?.textContent.startsWith('Updated')",Duration::from_secs(15)).await.unwrap();
            assert!(page
                .evaluate::<bool>("globalThis.__hmrMarker===true")
                .await
                .unwrap());
            assert!(page
                .evaluate::<String>("fetch('/').then(response=>response.text())")
                .await
                .unwrap()
                .contains("Updated"));
            browser.close().await.unwrap();
        }
    }
}
#[tokio::test]
#[ignore = "registry packages, Node compiler host, real Chromium"]
async fn chromium_framework_ssr_nested_hmr_and_recovery() {
    matrix(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "registry packages, Node compiler host, real Firefox"]
async fn firefox_framework_ssr_nested_hmr_and_recovery() {
    matrix(BrowserKind::Firefox).await;
}

fn nested_components(
    files: &mut std::collections::BTreeMap<String, String>,
    framework: &str,
    language: &str,
    extension: &str,
) {
    let typed = language == "ts";
    let component = if framework == "react" {
        format!("src/App.{extension}")
    } else {
        format!("src/App.{framework}")
    };
    let app = files.get_mut(&component).unwrap();
    match framework {
        "react" => {
            *app = format!(
                "import Child from './Child.{extension}';\n{}",
                app.replace("return <>", "return <><Child parent={count}/>")
            );
            files.insert(format!("src/Child.{extension}"),format!("import {{useState,useMemo}} from 'react'; export default function Child(props{}) {{ const [count,setCount]=useState(0);const items=useMemo(()=>[1,2,3].map(n=>n+props.parent),[props.parent]); return <section><span id=\"parent-value\">parent: {{props.parent}}</span><button id=\"nested-counter\" onClick={{()=>setCount(count+1)}}>nested: {{count}}</button><ul>{{items.map(n=><li key={{n}}>{{n}}</li>)}}</ul></section>; }}",if typed {": {parent:number}"} else {""}));
        }
        "vue" => {
            *app = app
                .replace(
                    "import { ref }",
                    "import Child from './Child.vue';\nimport { ref }",
                )
                .replace("</template>", "<Child :parent=\"count\" /></template>");
            files.insert("src/Child.vue".into(),format!("<script setup{}>import {{ref,computed}} from 'vue'; const props=defineProps({{parent:Number}});const count=ref(0);const items=computed(()=>[1,2,3].map(n=>n+props.parent));</script><template><section><span id=\"parent-value\">parent: {{{{props.parent}}}}</span><button id=\"nested-counter\" @click=\"count++\">nested: {{{{count}}}}</button><ul><li v-for=\"n in items\" :key=\"n\">{{{{n}}}}</li></ul></section></template>",if typed {" lang=\"ts\""} else {""}));
        }
        "svelte" => {
            *app = app
                .replacen(
                    "\nlet count",
                    "\nimport Child from './Child.svelte';\nlet count",
                    1,
                )
                .replacen("</script>", "</script><Child parent={count}/>", 1);
            files.insert("src/Child.svelte".into(),format!("<script{}>let {{parent}} = $props();let count=$state(0);let items=$derived([1,2,3].map(n=>n+parent));</script><section><span id=\"parent-value\">parent: {{parent}}</span><button id=\"nested-counter\" onclick={{()=>count++}}>nested: {{count}}</button><ul>{{#each items as n (n)}}<li>{{n}}</li>{{/each}}</ul></section>",if typed {" lang=\"ts\""} else {""}));
        }
        _ => unreachable!(),
    }
}
