fn inject(shell: &str, url: &str) -> String {
    if shell.contains("data-ferrite-islands-bootstrap") {
        return shell.into();
    }
    let tag =
        format!("<script type=\"module\" data-ferrite-islands-bootstrap src=\"{url}\"></script>");
    if shell.contains("</body>") {
        shell.replacen("</body>", &format!("{tag}</body>"), 1)
    } else {
        format!("{shell}{tag}")
    }
}
pub(crate) fn inject_dev_bootstrap(shell: &str, config: &ferrite_config::ResolvedConfig) -> String {
    if config.ssr.islands.is_empty() {
        return shell.into();
    }
    inject(
        shell,
        &format!(
            "{}@ferrite/islands-client",
            crate::loader::with_trailing_slash(&config.base)
        ),
    )
}
pub(crate) fn inject_build_bootstrap(
    shell: &str,
    config: &ferrite_config::ResolvedConfig,
) -> String {
    if config.ssr.islands.is_empty() {
        return shell.into();
    }
    inject(shell, "/@ferrite/islands-client")
}
