//! Native command-line adapter for the utilitycss compiler.

#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

use clap::{Args, CommandFactory, Parser, Subcommand};
use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use utilitycss_compiler::{
    CompatibilityProfile, Compiler, CompilerConfig, ExplainRequest, SourceInput,
};
use utilitycss_config::ConfigFile;
use utilitycss_css_ir::CssSerializationMode;
use utilitycss_extractor::{extract_for_framework, Framework};
use utilitycss_scanner::ExtractionMode;
use utilitycss_span::{validate_source_len, SourceId};
use utilitycss_stylesheet::{transform_stylesheet, StylesheetInput};
use utilitycss_swc::{extract as extract_swc, SourceKind as SwcSourceKind};
use walkdir::WalkDir;

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        eprintln!("utilitycss: {error}");
        std::process::exit(1);
    }
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io { path: PathBuf, source: io::Error },
    Config(utilitycss_config::ConfigError),
    Compiler(utilitycss_compiler::CompilerError),
    Extraction(String),
    Diagnostics,
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => formatter.write_str(message),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Config(error) => error.fmt(formatter),
            Self::Compiler(error) => error.fmt(formatter),
            Self::Extraction(message) => write!(formatter, "source extraction failed: {message}"),
            Self::Diagnostics => formatter.write_str("compilation failed with diagnostics"),
        }
    }
}

impl Error for CliError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Options {
    inputs: Vec<PathBuf>,
    stylesheet_inputs: Vec<PathBuf>,
    output: Option<PathBuf>,
    mode: Option<CssSerializationMode>,
    config: Option<PathBuf>,
    stats: bool,
    watch: bool,
    once: bool,
    interval: Duration,
}

#[derive(Clone, Debug, Parser)]
#[command(name = "utilitycss", about = "Build deterministic utility CSS from source files.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Build once and write CSS.
    Build(CommonArgs),
    /// Watch inputs and rebuild when files change.
    Watch(WatchArgs),
    /// Explain one candidate using the active semantic registry.
    Explain(CandidateArgs),
    /// Validate one candidate using the active semantic registry.
    Validate(CandidateArgs),
    /// Generate machine-readable compiler capabilities and reference artifacts.
    Capabilities(CapabilitiesArgs),
}

#[derive(Clone, Debug, Args)]
struct CandidateArgs {
    /// Candidate text to inspect.
    candidate: String,
    /// Read declarative JSON or CSS configuration.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Compatibility profile used for reporting.
    #[arg(long, default_value = "native")]
    compatibility: String,
    /// Emit the complete machine-readable result as JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug, Args)]
struct CapabilitiesArgs {
    /// Read declarative JSON or CSS configuration.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Write all generated artifacts into a directory.
    #[arg(long, value_name = "DIR")]
    output_dir: Option<PathBuf>,
    /// Print the expanded LLM reference when no output directory is supplied.
    #[arg(long)]
    full: bool,
}

#[derive(Clone, Debug, Args)]
struct CommonArgs {
    /// Add an input file or directory.
    #[arg(short = 'i', long = "input", value_name = "PATH")]
    input_flags: Vec<PathBuf>,
    /// Add an authored CSS stylesheet to transform with @apply.
    #[arg(long = "stylesheet", value_name = "PATH")]
    stylesheet_flags: Vec<PathBuf>,
    /// Write CSS to a file instead of stdout.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Read declarative JSON configuration.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Use readable CSS formatting.
    #[arg(long, conflicts_with = "minify")]
    pretty: bool,
    /// Use compact CSS formatting.
    #[arg(long, conflicts_with = "pretty")]
    minify: bool,
    /// Print compiler counters to stderr.
    #[arg(long)]
    stats: bool,
    /// Positional input files or directories.
    #[arg(value_name = "INPUT")]
    positional_inputs: Vec<PathBuf>,
}

#[derive(Clone, Debug, Args)]
struct WatchArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Run one watch iteration and exit.
    #[arg(long)]
    once: bool,
    /// Polling interval in milliseconds.
    #[arg(long, default_value_t = 250, value_name = "N")]
    interval_ms: u64,
}

fn run<I>(arguments: I) -> Result<(), CliError>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut arguments = arguments.into_iter().map(Into::into).collect::<Vec<_>>();
    if arguments.first().is_some_and(|argument| argument == "help" || argument == "--help") {
        print_help();
        return Ok(());
    }
    if matches!(
        arguments.first().map(String::as_str),
        Some("explain" | "validate" | "capabilities")
    ) {
        let cli = Cli::try_parse_from(std::iter::once("utilitycss".to_owned()).chain(arguments))
            .map_err(|error| usage(error.to_string()))?;
        return run_introspection(cli.command);
    }
    let watch = arguments.first().is_some_and(|argument| argument == "watch");
    if watch || arguments.first().is_some_and(|argument| argument == "build") {
        arguments.remove(0);
    }
    let options = parse_options(arguments, watch)?;
    if options.watch {
        run_watch(options)
    } else {
        run_build(&options)
    }
}

fn run_introspection(command: Command) -> Result<(), CliError> {
    match command {
        Command::Explain(args) => {
            let compiler = make_introspection_compiler(args.config.as_deref())?;
            let compatibility = parse_compatibility(&args.compatibility);
            let result = compiler
                .explain(ExplainRequest::new(&args.candidate).with_compatibility(compatibility));
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result)
                        .map_err(|error| usage(error.to_string()))?
                );
            } else {
                println!("candidate: {}", result.candidate);
                println!("status: {:?}", result.status);
                println!("normalized: {}", result.normalized);
                if let Some(css) = result.css {
                    println!("css: {css}");
                }
                for diagnostic in result.diagnostics {
                    println!(
                        "{} [{}]: {}",
                        diagnostic.severity, diagnostic.code, diagnostic.message
                    );
                    if let Some(help) = diagnostic.help {
                        println!("  help: {help}");
                    }
                    if let Some(explanation) = diagnostic.explanation {
                        println!("  explanation: {explanation}");
                    }
                    for suggestion in diagnostic.suggestions {
                        println!("  suggestion: {} ({})", suggestion.candidate, suggestion.reason);
                    }
                }
                for provenance in result.provenance {
                    println!(
                        "provenance: {}:{}{}",
                        provenance.kind,
                        provenance.key,
                        provenance.detail.map_or_else(String::new, |detail| format!(" ({detail})"))
                    );
                }
            }
            Ok(())
        }
        Command::Validate(args) => {
            let compiler = make_introspection_compiler(args.config.as_deref())?;
            let compatibility = parse_compatibility(&args.compatibility);
            let result = compiler
                .validate(ExplainRequest::new(&args.candidate).with_compatibility(compatibility));
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result)
                        .map_err(|error| usage(error.to_string()))?
                );
            } else {
                println!("{}: {:?}", if result.valid { "valid" } else { "invalid" }, result.status);
                for diagnostic in result.diagnostics {
                    println!(
                        "{} [{}]: {}",
                        diagnostic.severity, diagnostic.code, diagnostic.message
                    );
                    if let Some(explanation) = diagnostic.explanation {
                        println!("  explanation: {explanation}");
                    }
                    for suggestion in diagnostic.suggestions {
                        println!("  suggestion: {} ({})", suggestion.candidate, suggestion.reason);
                    }
                }
            }
            if result.valid {
                Ok(())
            } else {
                Err(CliError::Diagnostics)
            }
        }
        Command::Capabilities(args) => {
            let compiler = make_introspection_compiler(args.config.as_deref())?;
            let artifacts = compiler.generated_artifacts();
            if let Some(directory) = args.output_dir {
                fs::create_dir_all(&directory)
                    .map_err(|source| CliError::Io { path: directory.clone(), source })?;
                write_artifact(&directory, "capabilities.json", &artifacts.capabilities_json)?;
                write_artifact(&directory, "capabilities.schema.json", &artifacts.schema_json)?;
                write_artifact(&directory, "REFERENCE.md", &artifacts.reference_markdown)?;
                write_artifact(&directory, "llms.txt", &artifacts.llms_txt)?;
                write_artifact(&directory, "llms-full.txt", &artifacts.llms_full_txt)?;
                write_artifact(
                    &directory,
                    "compatibility-report.json",
                    &artifacts.compatibility_report_json,
                )?;
            } else if args.full {
                print!("{}", artifacts.llms_full_txt);
            } else {
                println!("{}", artifacts.capabilities_json);
            }
            Ok(())
        }
        Command::Build(_) | Command::Watch(_) => {
            Err(usage("introspection command expected: explain, validate, or capabilities"))
        }
    }
}

fn make_introspection_compiler(config: Option<&Path>) -> Result<Compiler, CliError> {
    let file = config
        .map_or_else(|| Ok(ConfigFile::default()), utilitycss_config::load)
        .map_err(CliError::Config)?;
    Ok(Compiler::new(
        CompilerConfig::new()
            .with_theme(file.theme().clone())
            .with_utility_registry(file.utilities().clone())
            .with_variant_registry(file.variants().clone())
            .with_preset_name(file.preset().name())
            .with_browser_target(file.browser_target())
            .with_serialization_mode(file.serialization_mode()),
    ))
}

fn parse_compatibility(value: &str) -> CompatibilityProfile {
    match value {
        "native" => CompatibilityProfile::Native,
        "tailwind-v4-like" | "tailwind-v4-subset" => CompatibilityProfile::TailwindV4Like,
        "tailwind-v3-like" | "tailwind-v3-subset" => CompatibilityProfile::TailwindV3Like,
        value => CompatibilityProfile::Custom(value.to_owned()),
    }
}

fn write_artifact(directory: &Path, name: &str, content: &str) -> Result<(), CliError> {
    let path = directory.join(name);
    fs::write(&path, content.as_bytes()).map_err(|source| CliError::Io { path, source })
}

fn parse_options(arguments: Vec<String>, watch: bool) -> Result<Options, CliError> {
    let command = if watch { "watch" } else { "build" };
    let arguments = std::iter::once("utilitycss".to_owned())
        .chain(std::iter::once(command.to_owned()))
        .chain(arguments)
        .collect::<Vec<_>>();
    let cli = Cli::try_parse_from(arguments).map_err(|error| usage(error.to_string()))?;
    let (common, once, interval) = match cli.command {
        Command::Build(common) => (common, false, Duration::from_millis(250)),
        Command::Watch(watch) => {
            (watch.common, watch.once, Duration::from_millis(watch.interval_ms.max(1)))
        }
        Command::Explain(_) | Command::Validate(_) | Command::Capabilities(_) => {
            return Err(usage("build or watch command expected"));
        }
    };
    let mut inputs = common.positional_inputs;
    inputs.extend(common.input_flags);
    if inputs.is_empty() && common.stylesheet_flags.is_empty() {
        return Err(usage("at least one source input or --stylesheet path is required"));
    }
    let mode = if common.pretty {
        Some(CssSerializationMode::Pretty)
    } else if common.minify {
        Some(CssSerializationMode::Minified)
    } else {
        None
    };
    Ok(Options {
        inputs,
        stylesheet_inputs: common.stylesheet_flags,
        output: common.output,
        mode,
        config: common.config,
        stats: common.stats,
        watch,
        once,
        interval,
    })
}

fn run_build(options: &Options) -> Result<(), CliError> {
    let mut compiler = make_compiler(options)?;
    let files = collect_files(&options.inputs, options.output.as_deref())?;
    for path in files {
        if !is_stylesheet(&path) {
            update_file(&mut compiler, &path)?;
        }
    }
    emit(&mut compiler, options)
}

/// Returns whether a watch failure is source-level (print the failure and keep
/// watching) rather than environmental (stop the watcher).
///
/// Compiler and extraction failures describe the content of one source, as do
/// undecodable bytes; the watcher prints them and waits for a fix. I/O,
/// configuration, and watcher failures describe the environment and stop the
/// watcher so the broken invocation stays visible to the caller.
fn is_recoverable_watch_error(error: &CliError) -> bool {
    match error {
        CliError::Compiler(_) | CliError::Extraction(_) | CliError::Diagnostics => true,
        CliError::Io { source, .. } => source.kind() == io::ErrorKind::InvalidData,
        CliError::Usage(_) | CliError::Config(_) => false,
    }
}

/// Applies one batch of watch events without letting source-level failures
/// stop the watcher. Returns whether output was rewritten.
fn handle_watch_batch(
    compiler: &mut Compiler,
    events: &[Event],
    options: &Options,
    known: &mut BTreeMap<PathBuf, ()>,
) -> Result<bool, CliError> {
    for event in events {
        if config_changed(event, options.config.as_deref()) {
            match reload_config(compiler, options) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!("utilitycss: configuration reload failed: {error}");
                }
            }
        }
        match apply_watch_event(compiler, event, options.output.as_deref(), known) {
            Ok(()) => {}
            Err(error) if is_recoverable_watch_error(&error) => {
                eprintln!("utilitycss: {error}");
            }
            Err(error) => return Err(error),
        }
    }
    emit_watch(compiler, options)
}

fn run_watch(options: Options) -> Result<(), CliError> {
    let mut compiler = make_compiler(&options)?;
    let mut known = BTreeMap::<PathBuf, ()>::new();
    refresh_sources(&mut compiler, &options.inputs, options.output.as_deref(), &mut known)?;
    if options.once {
        return emit(&mut compiler, &options);
    }
    emit_watch(&mut compiler, &options)?;

    let (sender, receiver) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = sender.send(event);
        },
        NotifyConfig::default().with_poll_interval(options.interval),
    )
    .map_err(|error| CliError::Usage(format!("could not start file watcher: {error}")))?;
    let mut watch_paths =
        options.inputs.iter().chain(options.stylesheet_inputs.iter()).cloned().collect::<Vec<_>>();
    if let Some(config) = options.config.as_ref() {
        let config_key = stable_path(config);
        let already_watched = watch_paths.iter().any(|input| {
            let input_key = stable_path(input);
            input_key == config_key || (input.is_dir() && config_key.starts_with(&input_key))
        });
        if !already_watched {
            watch_paths.push(config.clone());
        }
    }
    for input in &watch_paths {
        let mode =
            if input.is_dir() { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        watcher.watch(input, mode).map_err(|error| CliError::Io {
            path: input.clone(),
            source: io::Error::other(error.to_string()),
        })?;
    }
    loop {
        match receiver.recv_timeout(options.interval) {
            Ok(Ok(event)) => {
                let mut events = vec![event];
                while let Ok(result) = receiver.try_recv() {
                    match result {
                        Ok(event) => events.push(event),
                        Err(error) => {
                            return Err(CliError::Usage(format!("file watcher error: {error}")))
                        }
                    }
                }
                handle_watch_batch(&mut compiler, &events, &options, &mut known)?;
            }
            Ok(Err(error)) => return Err(CliError::Usage(format!("file watcher error: {error}"))),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(CliError::Usage("file watcher stopped unexpectedly".to_owned()))
            }
        }
    }
}

fn refresh_sources(
    compiler: &mut Compiler,
    inputs: &[PathBuf],
    output: Option<&Path>,
    known: &mut BTreeMap<PathBuf, ()>,
) -> Result<(), CliError> {
    let files = collect_files(inputs, output)?;
    let mut current = BTreeMap::new();
    for path in &files {
        if is_stylesheet(path) {
            continue;
        }
        match update_file(compiler, path) {
            Ok(()) => {
                current.insert(path.clone(), ());
            }
            Err(error) if is_recoverable_watch_error(&error) => {
                eprintln!("utilitycss: {error}");
            }
            Err(error) => return Err(error),
        }
    }
    let removed =
        known.keys().filter(|path| !current.contains_key(*path)).cloned().collect::<Vec<_>>();
    for path in removed {
        compiler.remove_source(&SourceId::new(source_id(&path)));
    }
    *known = current;
    Ok(())
}

fn apply_watch_event(
    compiler: &mut Compiler,
    event: &Event,
    output: Option<&Path>,
    known: &mut BTreeMap<PathBuf, ()>,
) -> Result<(), CliError> {
    if event.paths.is_empty() {
        return Ok(());
    }

    for path in &event.paths {
        let key = stable_path(path);
        let output_key = output.map(stable_path);
        let is_output = output_key.as_ref().is_some_and(|output| output == &key);
        let is_file = path.is_file();
        let is_supported = is_supported_source(path);

        if is_stylesheet(path) {
            continue;
        }

        if is_output || !is_supported || !is_file {
            let removed = known
                .keys()
                .filter(|known_path| {
                    known_path.as_path() == key.as_path() || known_path.starts_with(&key)
                })
                .cloned()
                .collect::<Vec<_>>();
            for removed_path in removed {
                known.remove(&removed_path);
                compiler.remove_source(&SourceId::new(source_id(&removed_path)));
            }
            continue;
        }

        update_file(compiler, &key)?;
        known.insert(key, ());
    }

    Ok(())
}

fn make_compiler(options: &Options) -> Result<Compiler, CliError> {
    Ok(Compiler::new(load_compiler_config(options)?))
}

fn load_compiler_config(options: &Options) -> Result<CompilerConfig, CliError> {
    let file = options
        .config
        .as_ref()
        .map_or_else(|| Ok(ConfigFile::default()), utilitycss_config::load)
        .map_err(CliError::Config)?;
    let mode = options.mode.unwrap_or(file.serialization_mode());
    Ok(CompilerConfig::new()
        .with_theme(file.theme().clone())
        .with_utility_registry(file.utilities().clone())
        .with_variant_registry(file.variants().clone())
        .with_preset_name(file.preset().name())
        .with_browser_target(file.browser_target())
        .with_serialization_mode(mode))
}

fn reload_config(compiler: &mut Compiler, options: &Options) -> Result<(), CliError> {
    compiler.replace_config(load_compiler_config(options)?);
    Ok(())
}

fn config_changed(event: &Event, config: Option<&Path>) -> bool {
    let Some(config) = config else {
        return false;
    };
    let config = stable_path(config);
    event.paths.iter().any(|path| stable_path(path) == config)
}

fn update_file(compiler: &mut Compiler, path: &Path) -> Result<(), CliError> {
    let content = fs::read_to_string(path)
        .map_err(|source| CliError::Io { path: path.to_owned(), source })?;
    let stable_source_id = source_id(path);
    let source_id = SourceId::new(stable_source_id.clone());
    let candidates = extract_candidates(path, &content)?;
    compiler
        .update_source_with_candidates(
            SourceInput::new(source_id, content).with_path(stable_source_id),
            candidates,
        )
        .map_err(CliError::Compiler)
}

fn extract_candidates(
    path: &Path,
    content: &str,
) -> Result<Vec<utilitycss_compiler::CandidateInput>, CliError> {
    validate_source_len(content.len()).map_err(|error| CliError::Extraction(error.to_string()))?;
    let extension =
        path.extension().and_then(|extension| extension.to_str()).map(str::to_ascii_lowercase);
    let candidates = match extension.as_deref() {
        Some("js") | Some("mjs") | Some("cjs") => extract_swc(content, SwcSourceKind::JavaScript)
            .map_err(|error| CliError::Extraction(error.to_string()))?
            .into_iter()
            .map(|candidate| (candidate.raw(), candidate.span()))
            .collect::<Vec<_>>(),
        Some("jsx") | Some("mjsx") | Some("cjsx") => extract_swc(content, SwcSourceKind::Jsx)
            .map_err(|error| CliError::Extraction(error.to_string()))?
            .into_iter()
            .map(|candidate| (candidate.raw(), candidate.span()))
            .collect::<Vec<_>>(),
        Some("ts") | Some("mts") | Some("cts") => extract_swc(content, SwcSourceKind::TypeScript)
            .map_err(|error| CliError::Extraction(error.to_string()))?
            .into_iter()
            .map(|candidate| (candidate.raw(), candidate.span()))
            .collect::<Vec<_>>(),
        Some("tsx") | Some("mtsx") | Some("ctsx") => extract_swc(content, SwcSourceKind::Tsx)
            .map_err(|error| CliError::Extraction(error.to_string()))?
            .into_iter()
            .map(|candidate| (candidate.raw(), candidate.span()))
            .collect::<Vec<_>>(),
        Some("vue") => extract_for_framework(content, Framework::Vue)
            .map_err(|error| CliError::Extraction(error.to_string()))?
            .into_iter()
            .map(|candidate| (candidate.raw(), candidate.span()))
            .collect::<Vec<_>>(),
        Some("svelte") => extract_for_framework(content, Framework::Svelte)
            .map_err(|error| CliError::Extraction(error.to_string()))?
            .into_iter()
            .map(|candidate| (candidate.raw(), candidate.span()))
            .collect::<Vec<_>>(),
        Some("astro") => extract_for_framework(content, Framework::Astro)
            .map_err(|error| CliError::Extraction(error.to_string()))?
            .into_iter()
            .map(|candidate| (candidate.raw(), candidate.span()))
            .collect::<Vec<_>>(),
        _ => extract_for_framework(content, Framework::Html)
            .map_err(|error| CliError::Extraction(error.to_string()))?
            .into_iter()
            .map(|candidate| (candidate.raw(), candidate.span()))
            .collect::<Vec<_>>(),
    };
    let mode = match extension.as_deref() {
        Some("js") | Some("mjs") | Some("cjs") | Some("jsx") | Some("mjsx") | Some("cjsx")
        | Some("ts") | Some("mts") | Some("cts") | Some("tsx") | Some("mtsx") | Some("ctsx") => {
            ExtractionMode::Ast
        }
        _ => ExtractionMode::Static,
    };
    Ok(candidates
        .into_iter()
        .map(|(raw, span)| {
            utilitycss_compiler::CandidateInput::new(raw, span).with_extraction_mode(mode)
        })
        .collect())
}

fn emit(compiler: &mut Compiler, options: &Options) -> Result<(), CliError> {
    emit_inner(compiler, options, false).map(|_| ())
}

/// Emits during watch: diagnostics are printed while the last-known-good
/// output is left intact, and the watcher keeps running. Returns whether
/// output was rewritten.
fn emit_watch(compiler: &mut Compiler, options: &Options) -> Result<bool, CliError> {
    emit_inner(compiler, options, true)
}

fn emit_inner(compiler: &mut Compiler, options: &Options, watch: bool) -> Result<bool, CliError> {
    let result = compiler.build();
    let stylesheet = transform_authored_stylesheets(compiler, options)?;
    let mode = compiler.config().serialization_mode();
    let separator = if mode == CssSerializationMode::Pretty { "\n" } else { "" };
    let css = if stylesheet.css.is_empty() {
        result.css().to_owned()
    } else if result.css().is_empty() {
        stylesheet.css.clone()
    } else {
        format!("{}{separator}{}", stylesheet.css, result.css())
    };
    let failed = !result.diagnostics().is_empty() || !stylesheet.diagnostics.is_empty();
    let keep_last_good = watch && failed;
    if !keep_last_good {
        if let Some(path) = &options.output {
            fs::write(path, css.as_bytes())
                .map_err(|source| CliError::Io { path: path.clone(), source })?;
        } else {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(css.as_bytes())
                .and_then(|_| stdout.write_all(b"\n"))
                .map_err(|source| CliError::Io { path: PathBuf::from("<stdout>"), source })?;
        }
    }
    for diagnostic in result.diagnostics().iter().chain(stylesheet.diagnostics.iter()) {
        let source = diagnostic.source().map_or_else(|| "<source>".to_owned(), ToString::to_string);
        let span = diagnostic
            .span()
            .map_or_else(String::new, |span| format!(":{}..{}", span.start(), span.end()));
        eprintln!("{}{}: {} [{}]", source, span, diagnostic.message(), diagnostic.code());
        if let Some(help) = diagnostic.help() {
            eprintln!("  help: {help}");
        }
    }
    if options.stats {
        let stats = result.stats();
        eprintln!(
            "stats: sources_scanned={} bytes_scanned={} candidates_found={} unique_candidates={} candidates_parsed={} cache_hits={} rules_generated={} rules_removed={}",
            stats.sources_scanned(),
            stats.bytes_scanned(),
            stats.candidates_found(),
            stats.unique_candidates(),
            stats.candidates_parsed(),
            stats.cache_hits(),
            stats.rules_generated(),
            stats.rules_removed(),
        );
    }
    if failed && !watch {
        return Err(CliError::Diagnostics);
    }
    Ok(!keep_last_good)
}

struct AuthoredStylesheetOutput {
    css: String,
    diagnostics: Vec<utilitycss_diagnostics::Diagnostic>,
}

fn transform_authored_stylesheets(
    compiler: &mut Compiler,
    options: &Options,
) -> Result<AuthoredStylesheetOutput, CliError> {
    let mut paths = collect_files(&options.inputs, options.output.as_deref())?;
    paths.extend(collect_files(&options.stylesheet_inputs, options.output.as_deref())?);
    paths.sort();
    paths.dedup();

    let mode = compiler.config().serialization_mode();
    let separator = if mode == CssSerializationMode::Pretty { "\n" } else { "" };
    let mut css = String::new();
    let mut diagnostics = Vec::new();
    for path in paths.into_iter().filter(|path| is_stylesheet(path)) {
        let content = fs::read_to_string(&path)
            .map_err(|source| CliError::Io { path: path.clone(), source })?;
        let source_id = source_id(&path);
        let output = transform_stylesheet(
            compiler,
            StylesheetInput::new(SourceId::new(source_id.clone()), content).with_path(source_id),
        );
        if !css.is_empty() {
            css.push_str(separator);
        }
        css.push_str(output.css());
        diagnostics.extend(output.diagnostics().iter().cloned());
    }
    Ok(AuthoredStylesheetOutput { css, diagnostics })
}

fn collect_files(inputs: &[PathBuf], output: Option<&Path>) -> Result<Vec<PathBuf>, CliError> {
    let mut files = BTreeMap::new();
    for input in inputs {
        collect_path(input, output, &mut files)?;
    }
    Ok(files.into_keys().collect())
}

fn collect_path(
    path: &Path,
    output: Option<&Path>,
    files: &mut BTreeMap<PathBuf, ()>,
) -> Result<(), CliError> {
    let output = output.map(stable_path);
    let mut entries = WalkDir::new(path).follow_links(false).into_iter();
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(|error| CliError::Io {
            path: error.path().map_or_else(|| path.to_owned(), Path::to_owned),
            source: io::Error::other(error.to_string()),
        })?;
        if entry.file_type().is_dir() && is_ignored_directory(entry.path()) {
            entries.skip_current_dir();
            continue;
        }
        if entry.file_type().is_file()
            && is_supported_source(entry.path())
            && output.as_ref().is_none_or(|output| stable_path(entry.path()) != *output)
        {
            files.insert(stable_path(entry.path()), ());
        }
    }
    Ok(())
}

fn is_supported_source(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "html"
            | "htm"
            | "js"
            | "mjs"
            | "cjs"
            | "jsx"
            | "mjsx"
            | "cjsx"
            | "ts"
            | "mts"
            | "cts"
            | "tsx"
            | "mtsx"
            | "ctsx"
            | "vue"
            | "svelte"
            | "astro"
            | "css"
    )
}

fn is_stylesheet(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("css"))
}

fn is_ignored_directory(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.to_ascii_lowercase().as_str(),
            ".git"
                | "node_modules"
                | "target"
                | "dist"
                | "build"
                | "coverage"
                | ".next"
                | ".nuxt"
                | ".svelte-kit"
        )
    })
}

fn stable_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_owned()
        } else {
            env::current_dir().map_or_else(|_| path.to_owned(), |current| current.join(path))
        }
    })
}

fn source_id(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn usage(message: impl Into<String>) -> CliError {
    CliError::Usage(message.into())
}

fn print_help() {
    println!("{}", Cli::command().render_help());
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs, io,
        path::PathBuf,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::{
        collect_files, config_changed, emit_watch, handle_watch_batch, is_recoverable_watch_error,
        make_compiler, parse_options, refresh_sources, reload_config, run, stable_path,
        transform_authored_stylesheets, CliError, Compiler, CompilerConfig, CssSerializationMode,
        Event, Options,
    };
    use notify::EventKind;
    use utilitycss_compiler::{CompilerError, SourceInput};
    use utilitycss_config::ConfigError;
    use utilitycss_span::SourceId;

    #[test]
    fn parses_build_options_and_positional_inputs() {
        let options = parse_options(vec!["--pretty".into(), "src".into(), "--stats".into()], false)
            .expect("options are valid");

        assert_eq!(options.inputs, vec![PathBuf::from("src")]);
        assert_eq!(options.mode, Some(CssSerializationMode::Pretty));
        assert!(options.stats);
        assert!(!options.watch);
    }

    #[test]
    fn help_is_available_without_a_workspace() {
        run(["help".to_owned()]).expect("help does not require inputs");
    }

    #[test]
    fn collection_skips_binary_and_generated_directories_and_output_files() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("utilitycss-cli-{suffix}"));
        let output = root.join("src/generated.html");
        fs::create_dir_all(root.join("src")).expect("test source directory is created");
        fs::create_dir_all(root.join("node_modules/pkg")).expect("ignored directory is created");
        fs::create_dir_all(root.join("target")).expect("ignored directory is created");
        fs::write(root.join("src/input.html"), "<div class=\"p-4\"></div>")
            .expect("source is written");
        fs::write(&output, "p-4{padding:1rem}").expect("generated output is written");
        fs::write(root.join("src/image.bin"), [0_u8, 159, 146, 150]).expect("binary is written");
        fs::write(root.join("node_modules/pkg/ignored.html"), "p-8")
            .expect("ignored source is written");
        fs::write(root.join("target/ignored.html"), "p-8").expect("ignored source is written");

        let files =
            collect_files(std::slice::from_ref(&root), Some(&output)).expect("collection succeeds");
        assert_eq!(files, vec![stable_path(&root.join("src/input.html"))]);

        fs::remove_dir_all(root).expect("test directory is removed");
    }

    #[test]
    fn transforms_css_inputs_separately_from_candidate_sources() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("utilitycss-cli-stylesheet-{suffix}"));
        fs::create_dir_all(&root).expect("test directory is created");
        let stylesheet = root.join("app.css");
        fs::write(&stylesheet, ".button { @apply flex p-4; }").expect("stylesheet is written");
        let options = Options {
            inputs: Vec::new(),
            stylesheet_inputs: vec![stylesheet],
            output: None,
            mode: Some(CssSerializationMode::Minified),
            config: None,
            stats: false,
            watch: false,
            once: false,
            interval: std::time::Duration::from_millis(250),
        };
        let mut compiler = Compiler::new(CompilerConfig::new());

        let output = transform_authored_stylesheets(&mut compiler, &options)
            .expect("stylesheet transformation succeeds");

        assert_eq!(output.css, ".button{display:flex;padding:1rem;}");
        assert!(output.diagnostics.is_empty());
        assert_eq!(compiler.source_count(), 0);
        fs::remove_dir_all(root).expect("test directory is removed");
    }

    #[test]
    fn config_watch_reload_replaces_theme_without_restarting_the_compiler() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("utilitycss-cli-config-watch-{suffix}"));
        fs::create_dir_all(&root).expect("test directory is created");
        let config_path = root.join("utilitycss.json");
        fs::write(&config_path, r#"{"theme":{"colors":{"brand":"red"}}}"#)
            .expect("initial config is written");
        let options = Options {
            inputs: Vec::new(),
            stylesheet_inputs: Vec::new(),
            output: None,
            mode: None,
            config: Some(config_path.clone()),
            stats: false,
            watch: true,
            once: false,
            interval: Duration::from_millis(250),
        };
        let mut compiler = make_compiler(&options).expect("initial config loads");
        compiler
            .update_source(SourceInput::new(SourceId::new("inline"), "bg-brand"))
            .expect("source is valid");
        assert!(compiler.build().css().contains("background-color:red"));

        fs::write(&config_path, r#"{"theme":{"colors":{"brand":"blue"}}}"#)
            .expect("updated config is written");
        let mut event = Event::new(EventKind::Any);
        event.paths.push(config_path.clone());
        assert!(config_changed(&event, options.config.as_deref()));
        reload_config(&mut compiler, &options).expect("updated config loads");

        assert!(compiler.build().css().contains("background-color:blue"));
        fs::remove_dir_all(root).expect("test directory is removed");
    }

    fn watch_test_options(root: &std::path::Path, output: PathBuf) -> Options {
        Options {
            inputs: vec![root.to_owned()],
            stylesheet_inputs: Vec::new(),
            output: Some(output),
            mode: Some(CssSerializationMode::Minified),
            config: None,
            stats: false,
            watch: true,
            once: false,
            interval: Duration::from_millis(250),
        }
    }

    fn watch_test_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("utilitycss-cli-{name}-{suffix}"));
        fs::create_dir_all(&root).expect("test directory is created");
        root
    }

    #[test]
    fn htm_sources_are_collected_like_html() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("utilitycss-cli-htm-{suffix}"));
        fs::create_dir_all(&root).expect("test directory is created");
        for name in ["page.html", "lower.htm", "upper.HTM", "notes.txt"] {
            fs::write(root.join(name), "<div></div>").expect("fixture is written");
        }

        let files = collect_files(std::slice::from_ref(&root), None).expect("inputs collect");
        let names = files
            .iter()
            .map(|path| path.file_name().expect("file has a name").to_owned())
            .collect::<Vec<_>>();
        for expected in ["page.html", "lower.htm", "upper.HTM"] {
            assert!(names.iter().any(|name| name == expected), "collects {expected}: {names:?}");
        }
        assert!(!names.iter().any(|name| name == "notes.txt"), "skips txt: {names:?}");

        fs::remove_dir_all(root).expect("test directory is removed");
    }

    #[test]
    fn watch_error_classification_matches_recovery_policy() {
        assert!(is_recoverable_watch_error(&CliError::Compiler(CompilerError::EmptySourceId)));
        assert!(is_recoverable_watch_error(&CliError::Extraction("too large".to_owned())));
        assert!(is_recoverable_watch_error(&CliError::Diagnostics));
        assert!(is_recoverable_watch_error(&CliError::Io {
            path: PathBuf::from("input.html"),
            source: io::Error::new(io::ErrorKind::InvalidData, "undecodable bytes"),
        }));

        assert!(!is_recoverable_watch_error(&CliError::Usage("broken".to_owned())));
        assert!(!is_recoverable_watch_error(&CliError::Config(ConfigError::InvalidJson(
            "broken".to_owned()
        ))));
        for kind in [io::ErrorKind::NotFound, io::ErrorKind::PermissionDenied] {
            assert!(!is_recoverable_watch_error(&CliError::Io {
                path: PathBuf::from("input.html"),
                source: io::Error::new(kind, "environmental failure"),
            }));
        }
    }

    #[test]
    fn watch_batch_survives_invalid_edits_and_keeps_last_good_output() {
        let root = watch_test_root("watch-invalid");
        let input = root.join("input.html");
        let output = root.join("output.css");
        fs::write(&input, "<div class=\"p-4\"></div>").expect("seed input is written");
        let options = watch_test_options(&root, output.clone());
        let mut compiler = make_compiler(&options).expect("default config loads");
        let mut known = BTreeMap::new();
        refresh_sources(&mut compiler, &options.inputs, options.output.as_deref(), &mut known)
            .expect("initial load succeeds");
        assert!(emit_watch(&mut compiler, &options).expect("initial emit succeeds"));
        let last_good = fs::read_to_string(&output).expect("output is written");
        assert!(last_good.contains("padding:1rem"), "initial css: {last_good}");

        fs::write(&input, "<div class=\"p-[]\"></div>").expect("invalid edit is written");
        let mut event = Event::new(EventKind::Any);
        event.paths.push(input.clone());
        assert!(
            !handle_watch_batch(&mut compiler, &[event], &options, &mut known)
                .expect("invalid edits do not stop the watcher"),
            "diagnostics must not rewrite output"
        );
        assert_eq!(
            fs::read_to_string(&output).expect("output is readable"),
            last_good,
            "last-known-good output stays intact"
        );

        fs::write(&input, "<div class=\"p-8\"></div>").expect("fixed edit is written");
        let mut event = Event::new(EventKind::Any);
        event.paths.push(input.clone());
        assert!(
            handle_watch_batch(&mut compiler, &[event], &options, &mut known)
                .expect("fixed edits rebuild"),
            "fixed edits rewrite output"
        );
        let rebuilt = fs::read_to_string(&output).expect("output is rewritten");
        assert!(rebuilt.contains("padding:2rem"), "rebuilt css: {rebuilt}");

        fs::remove_dir_all(root).expect("test directory is removed");
    }

    #[test]
    fn watch_batch_reports_undecodable_sources_without_stopping() {
        let root = watch_test_root("watch-binary");
        let input = root.join("input.html");
        let output = root.join("output.css");
        fs::write(&input, "<div class=\"p-4\"></div>").expect("seed input is written");
        let options = watch_test_options(&root, output.clone());
        let mut compiler = make_compiler(&options).expect("default config loads");
        let mut known = BTreeMap::new();
        refresh_sources(&mut compiler, &options.inputs, options.output.as_deref(), &mut known)
            .expect("initial load succeeds");
        emit_watch(&mut compiler, &options).expect("initial emit succeeds");
        let last_good = fs::read_to_string(&output).expect("output is written");

        fs::write(&input, [0xff_u8, 0xfe, 0x00, 0x28]).expect("binary edit is written");
        let mut event = Event::new(EventKind::Any);
        event.paths.push(input.clone());
        handle_watch_batch(&mut compiler, &[event], &options, &mut known)
            .expect("undecodable edits do not stop the watcher");
        assert_eq!(
            fs::read_to_string(&output).expect("output is readable"),
            last_good,
            "last-known-good output stays intact"
        );

        fs::remove_dir_all(root).expect("test directory is removed");
    }

    #[test]
    fn watch_batch_propagates_environmental_failures() {
        let root = watch_test_root("watch-io-error");
        let options = watch_test_options(&root, root.join("missing-directory").join("output.css"));
        let mut compiler = make_compiler(&options).expect("default config loads");
        let mut known = BTreeMap::new();

        let error = handle_watch_batch(&mut compiler, &[], &options, &mut known)
            .expect_err("missing output directory stops the watcher");
        assert!(matches!(error, CliError::Io { .. }));

        fs::remove_dir_all(root).expect("test directory is removed");
    }
}
