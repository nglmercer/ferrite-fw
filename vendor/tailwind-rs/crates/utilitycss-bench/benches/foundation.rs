//! Dependency-free benchmark for Phase 0 foundation types.

use std::hint::black_box;
use std::time::Instant;

use utilitycss_compiler::{Compiler, CompilerConfig, SourceInput};
use utilitycss_extractor::{extract_for_framework, Framework};
use utilitycss_scanner::scan;
use utilitycss_span::{SourceId, Span};
use utilitycss_stylesheet::{transform_stylesheet, StylesheetInput};
use utilitycss_swc::{extract as extract_swc, SourceKind};
use utilitycss_syntax::parse;

fn main() {
    const ITERATIONS: u32 = 100_000;

    benchmark_spans(ITERATIONS);
    benchmark_scan_and_parse(ITERATIONS);
    benchmark_static_extraction(ITERATIONS / 100);
    benchmark_swc_extraction(ITERATIONS / 10);
    benchmark_compiler(ITERATIONS / 100);
    benchmark_introspection(ITERATIONS / 100);
    benchmark_stylesheet();
}

fn benchmark_static_extraction(iterations: u32) {
    let source = (0..2_000)
        .map(|index| {
            format!(
                "<!-- comment-{index} class=\"ignored-{index}\" --> <div class=\"flex p-{index}\" data-id=\"{index}\"></div>"
            )
        })
        .collect::<String>();
    let started = Instant::now();
    let mut extracted = 0_u64;
    for _ in 0..iterations {
        extracted = extracted.wrapping_add(
            extract_for_framework(black_box(&source), Framework::Html)
                .expect("benchmark fixture fits")
                .len() as u64,
        );
    }
    println!(
        "static extraction: {iterations} iterations over 2,000 attributes/comments in {:?} (candidates={extracted})",
        started.elapsed()
    );
}

fn benchmark_swc_extraction(iterations: u32) {
    let source = r#"
        export const Card = ({ active }) => (
            <article className={clsx("flex", active && "p-4", `text-${tone}`)} />
        );
    "#;
    let started = Instant::now();
    let mut extracted = 0_u64;
    for _ in 0..iterations {
        let candidates =
            extract_swc(black_box(source), SourceKind::Jsx).expect("benchmark source is valid JSX");
        extracted = extracted.wrapping_add(candidates.len() as u64);
    }
    let elapsed = started.elapsed();
    println!("swc extraction: {iterations} iterations in {elapsed:?} (candidates={extracted})");
}

fn benchmark_spans(iterations: u32) {
    let started = Instant::now();
    let mut checksum = 0_u64;
    for index in 0..iterations {
        let span = Span::new(index, index + 12).expect("benchmark span is ordered");
        checksum = checksum.wrapping_add(u64::from(black_box(span.len())));
    }

    let elapsed = started.elapsed();
    println!("span construction: {iterations} iterations in {elapsed:?} (checksum={checksum})");
}

fn benchmark_scan_and_parse(iterations: u32) {
    let source = r#"<div class="flex gap-4 p-4 hover:bg-red-500/50 md:grid"></div>"#;
    let started = Instant::now();
    let mut scanned = 0_u64;
    let mut parsed = 0_u64;

    for _ in 0..iterations {
        let tokens = scan(black_box(source));
        scanned = scanned.wrapping_add(tokens.len() as u64);
        for token in tokens {
            if parse(black_box(token.raw())).is_ok() {
                parsed = parsed.wrapping_add(1);
            }
        }
    }

    let elapsed = started.elapsed();
    println!(
        "scan+parse: {iterations} iterations in {elapsed:?} (scanned={scanned}, parsed={parsed})"
    );
}

fn benchmark_compiler(iterations: u32) {
    let source_id = SourceId::new("src/app.html");
    let source = r#"<main class="flex gap-4 p-4 hover:bg-red-500/50 md:grid"></main>"#;

    let started = Instant::now();
    let mut css_bytes = 0_u64;
    for _ in 0..iterations {
        let mut compiler = Compiler::new(CompilerConfig::new());
        compiler
            .update_source(SourceInput::new(source_id.clone(), black_box(source)))
            .expect("benchmark source is valid");
        css_bytes = css_bytes.wrapping_add(compiler.build().css().len() as u64);
    }
    println!(
        "cold compile: {iterations} iterations in {:?} (css-bytes={css_bytes})",
        started.elapsed()
    );

    let mut compiler = Compiler::new(CompilerConfig::new());
    compiler
        .update_source(SourceInput::new(source_id.clone(), source))
        .expect("benchmark source is valid");
    black_box(compiler.build());

    let started = Instant::now();
    for index in 0..iterations {
        let content = if index % 2 == 0 { source } else { "<main class=\"flex p-8\"></main>" };
        compiler
            .update_source(SourceInput::new(source_id.clone(), black_box(content)))
            .expect("benchmark source is valid");
        black_box(compiler.build());
    }
    println!("incremental rebuild: {iterations} iterations in {:?}", started.elapsed());

    let started = Instant::now();
    for _ in 0..iterations {
        black_box(compiler.build());
    }
    println!("no-op rebuild: {iterations} iterations in {:?}", started.elapsed());
}

fn benchmark_introspection(iterations: u32) {
    let compiler = Compiler::new(CompilerConfig::new());
    let candidates = [
        "hover:bg-red-500/50!",
        "w-[calc(100%_-_2rem)]",
        "[@supports(display:grid)]:grid",
        "[content-visibility:auto]",
    ];
    let started = Instant::now();
    let mut valid = 0_u64;
    for index in 0..iterations {
        let result = compiler.explain(utilitycss_compiler::ExplainRequest::new(black_box(
            candidates[index as usize % candidates.len()],
        )));
        valid = valid
            .wrapping_add(u64::from(result.status == utilitycss_compiler::ResolutionStatus::Valid));
    }
    println!("explain+resolve: {iterations} iterations in {:?} (valid={valid})", started.elapsed());
}

fn benchmark_stylesheet() {
    let large_no_apply =
        (0..10_000).map(|index| format!(".plain-{index} {{ color: red; }}")).collect::<String>();
    let mut compiler = Compiler::new(CompilerConfig::new());
    let started = Instant::now();
    let output = transform_stylesheet(
        &mut compiler,
        StylesheetInput::new(SourceId::new("no-apply.css"), black_box(large_no_apply)),
    );
    println!(
        "stylesheet no-apply fast path: 10,000 authored rules in {:?} (css-bytes={})",
        started.elapsed(),
        output.css().len()
    );

    for count in [100_usize, 1_000, 10_000] {
        let source = (0..count)
            .map(|index| format!(".button-{index} {{ @apply flex p-4 hover:bg-red-500; }}"))
            .collect::<String>();
        let mut compiler = Compiler::new(CompilerConfig::new());
        let started = Instant::now();
        let output = transform_stylesheet(
            &mut compiler,
            StylesheetInput::new(SourceId::new("bench.css"), black_box(source)),
        );
        println!(
            "stylesheet transform: {count} @apply directives in {:?} (css-bytes={}, diagnostics={})",
            started.elapsed(),
            output.css().len(),
            output.diagnostics().len()
        );
    }

    let repeated = (0..10_000)
        .map(|index| format!(".repeat-{index} {{ @apply flex p-4; }}"))
        .collect::<String>();
    let mut compiler = Compiler::new(CompilerConfig::new());
    let started = Instant::now();
    let output = transform_stylesheet(
        &mut compiler,
        StylesheetInput::new(SourceId::new("repeated.css"), black_box(repeated)),
    );
    println!(
        "stylesheet repeated candidates: 10,000 directives in {:?} (css-bytes={})",
        started.elapsed(),
        output.css().len()
    );
}
