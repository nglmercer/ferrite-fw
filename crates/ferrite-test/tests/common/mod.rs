//! Shared vite-compat test helpers.

use ferrite::server::DevServer;
use ferrite_test::TempProject;

#[allow(dead_code)]
pub fn dev_config(project: &TempProject) -> ferrite::ResolvedConfig {
    project.resolve_config()
}

#[allow(dead_code)]
pub async fn dev_server(project: &TempProject) -> DevServer {
    DevServer::new_without_watcher(dev_config(project), vec![])
        .await
        .expect("server")
}
