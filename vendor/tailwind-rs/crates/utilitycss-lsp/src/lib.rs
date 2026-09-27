//! IDE services for utilitycss using the standard Language Server Protocol.
//!
//! The server keeps editor lifecycle state at the adapter boundary, delegates extraction and
//! semantics to the Rust compiler crates, and exposes diagnostics, completion, and hover data.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{collections::BTreeMap, path::Path, sync::Mutex};

use tower_lsp::{
    jsonrpc::Result,
    lsp_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, Diagnostic as LspDiagnostic, DiagnosticSeverity,
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        Documentation, Hover, HoverContents, HoverParams, InitializeParams, InitializeResult,
        InitializedParams, MarkedString, MessageType, Position, Range, ServerCapabilities,
        TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    },
    Client, LanguageServer,
};
use utilitycss_compiler::{CandidateInput, Compiler, SourceInput};
use utilitycss_diagnostics::Severity;
use utilitycss_extractor::{extract_for_framework, Framework};
use utilitycss_scanner::ExtractionMode;
use utilitycss_span::{SourceId, Span};
use utilitycss_swc::{extract as extract_swc, SourceKind as SwcSourceKind};

/// The server backend shared by all LSP requests.
#[derive(Debug)]
pub struct Backend {
    client: Client,
    state: Mutex<ServerState>,
}

#[derive(Debug, Default)]
struct ServerState {
    compiler: Compiler,
    documents: BTreeMap<Url, String>,
}

impl Backend {
    /// Creates a backend connected to an LSP client.
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self { client, state: Mutex::new(ServerState::default()) }
    }

    async fn update_document(&self, uri: Url, content: String) {
        let diagnostics = {
            let Ok(mut state) = self.state.lock() else {
                self.client
                    .log_message(MessageType::ERROR, "utilitycss LSP state is poisoned")
                    .await;
                return;
            };
            let diagnostics = compile_document(&mut state, &uri, &content);
            state.documents.insert(uri.clone(), content);
            diagnostics
        };
        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions::default()),
                hover_provider: Some(tower_lsp::lsp_types::HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "utilitycss language server initialized").await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.update_document(params.text_document.uri, params.text_document.text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        self.update_document(params.text_document.uri, change.text).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Ok(mut state) = self.state.lock() {
            state.documents.remove(&uri);
            state.compiler.remove_source(&SourceId::new(uri.to_string()));
        }
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let items = self
            .state
            .lock()
            .ok()
            .map(|state| {
                state
                    .compiler
                    .completions("")
                    .into_iter()
                    .map(|item| CompletionItem {
                        label: item.label,
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(item.detail),
                        documentation: Some(Documentation::String(item.documentation)),
                        ..Default::default()
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let word = self
            .state
            .lock()
            .ok()
            .and_then(|state| state.documents.get(&uri).cloned())
            .and_then(|source| word_at_position(&source, position));
        let Some(word) = word else {
            return Ok(None);
        };
        let Some(info) = self.state.lock().ok().and_then(|state| state.compiler.hover(&word))
        else {
            return Ok(None);
        };
        let mut content = format!("{}\n\n`{}`", info.description, info.candidate);
        if let Some(css) = info.css {
            content.push_str(&format!("\n\n```css\n{css}\n```"));
        }
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(content)),
            range: None,
        }))
    }
}

fn compile_document(state: &mut ServerState, uri: &Url, content: &str) -> Vec<LspDiagnostic> {
    let source_id = SourceId::new(uri.to_string());
    let source = SourceInput::new(source_id.clone(), content.to_owned()).with_path(uri.path());
    let candidates = extract_candidates(uri, content);
    match candidates {
        Ok(candidates) => {
            if let Err(error) = state.compiler.update_source_with_candidates(source, candidates) {
                state.compiler.remove_source(&source_id);
                return vec![error_diagnostic(content, error.to_string(), None)];
            }
        }
        Err(error) => {
            state.compiler.remove_source(&source_id);
            let span = error.offset().and_then(|offset| Span::new(offset, offset));
            return vec![error_diagnostic(content, error.to_string(), span)];
        }
    }

    state
        .compiler
        .build()
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.source().is_some_and(|source| source == &source_id))
        .map(|diagnostic| {
            let span = diagnostic.span();
            LspDiagnostic {
                range: span
                    .map(|span| lsp_range(content, span))
                    .unwrap_or_else(|| lsp_range(content, Span::empty(0))),
                severity: Some(match diagnostic.severity() {
                    Severity::Error => DiagnosticSeverity::ERROR,
                    Severity::Warning => DiagnosticSeverity::WARNING,
                    Severity::Note => DiagnosticSeverity::INFORMATION,
                    Severity::Help => DiagnosticSeverity::HINT,
                }),
                code: Some(tower_lsp::lsp_types::NumberOrString::String(
                    diagnostic.code().to_string(),
                )),
                message: lsp_diagnostic_message(diagnostic),
                ..Default::default()
            }
        })
        .collect()
}

fn lsp_diagnostic_message(diagnostic: &utilitycss_diagnostics::Diagnostic) -> String {
    let mut message = diagnostic.message().to_owned();
    if let Some(help) = diagnostic.help() {
        message.push_str(&format!(" ({help})"));
    }
    if let Some(explanation) = diagnostic.explanation() {
        message.push_str(&format!(" — {explanation}"));
    }
    if !diagnostic.suggestions().is_empty() {
        let suggestions = diagnostic
            .suggestions()
            .iter()
            .map(|suggestion| suggestion.replacement.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        message.push_str(&format!(" Suggestions: {suggestions}"));
    }
    message
}

/// Extraction failures from either the SWC or framework extractor.
#[derive(Debug)]
enum LspExtractionError {
    Swc(utilitycss_swc::ExtractionError),
    Framework(utilitycss_extractor::ExtractionError),
}

impl LspExtractionError {
    fn offset(&self) -> Option<u32> {
        match self {
            Self::Swc(error) => error.offset(),
            Self::Framework(_) => None,
        }
    }
}

impl std::fmt::Display for LspExtractionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Swc(error) => write!(formatter, "{error}"),
            Self::Framework(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<utilitycss_swc::ExtractionError> for LspExtractionError {
    fn from(error: utilitycss_swc::ExtractionError) -> Self {
        Self::Swc(error)
    }
}

impl From<utilitycss_extractor::ExtractionError> for LspExtractionError {
    fn from(error: utilitycss_extractor::ExtractionError) -> Self {
        Self::Framework(error)
    }
}

fn extract_candidates(
    uri: &Url,
    content: &str,
) -> std::result::Result<Vec<CandidateInput>, LspExtractionError> {
    match file_kind(uri.path()) {
        FileKind::JavaScript(kind) => Ok(extract_swc(content, kind)?
            .into_iter()
            .map(|candidate| {
                CandidateInput::new(candidate.raw(), candidate.span())
                    .with_extraction_mode(ExtractionMode::Ast)
            })
            .collect()),
        FileKind::Framework(framework) => Ok(extract_for_framework(content, framework)?
            .into_iter()
            .map(|candidate| {
                CandidateInput::new(candidate.raw(), candidate.span())
                    .with_extraction_mode(ExtractionMode::Static)
            })
            .collect()),
    }
}

enum FileKind {
    JavaScript(SwcSourceKind),
    Framework(Framework),
}

fn file_kind(path: &str) -> FileKind {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("js") | Some("mjs") | Some("cjs") => FileKind::JavaScript(SwcSourceKind::JavaScript),
        Some("jsx") | Some("mjsx") | Some("cjsx") => FileKind::JavaScript(SwcSourceKind::Jsx),
        Some("ts") | Some("mts") | Some("cts") => FileKind::JavaScript(SwcSourceKind::TypeScript),
        Some("tsx") | Some("mtsx") | Some("ctsx") => FileKind::JavaScript(SwcSourceKind::Tsx),
        Some("vue") => FileKind::Framework(Framework::Vue),
        Some("svelte") => FileKind::Framework(Framework::Svelte),
        Some("astro") => FileKind::Framework(Framework::Astro),
        _ => FileKind::Framework(Framework::Html),
    }
}

fn error_diagnostic(source: &str, message: String, span: Option<Span>) -> LspDiagnostic {
    LspDiagnostic {
        range: span
            .map(|span| lsp_range(source, span))
            .unwrap_or_else(|| lsp_range(source, Span::empty(0))),
        severity: Some(DiagnosticSeverity::ERROR),
        message,
        ..Default::default()
    }
}

fn lsp_range(source: &str, span: Span) -> Range {
    Range { start: position_at(source, span.start()), end: position_at(source, span.end()) }
}

fn position_at(source: &str, offset: u32) -> Position {
    let offset = usize::try_from(offset).unwrap_or(source.len()).min(source.len());
    let prefix = source.get(..offset).unwrap_or_default();
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let character = prefix[line_start..].encode_utf16().count() as u32;
    Position { line, character }
}

fn word_at_position(source: &str, position: Position) -> Option<String> {
    let offset = offset_at(source, position)?;
    let bytes = source.as_bytes();
    let mut start = offset.min(bytes.len());
    let mut end = start;
    while start > 0
        && !bytes[start - 1].is_ascii_whitespace()
        && !matches!(bytes[start - 1], b'"' | b'\'' | b'`' | b'<' | b'>')
    {
        start -= 1;
    }
    while end < bytes.len()
        && !bytes[end].is_ascii_whitespace()
        && !matches!(bytes[end], b'"' | b'\'' | b'`' | b'<' | b'>')
    {
        end += 1;
    }
    source.get(start..end).filter(|word| !word.is_empty()).map(str::to_owned)
}

fn offset_at(source: &str, position: Position) -> Option<usize> {
    let mut line = 0_u32;
    let mut offset = 0_usize;
    for segment in source.split_inclusive('\n') {
        if line == position.line {
            let mut utf16 = 0_u32;
            for (index, character) in segment.char_indices() {
                if utf16 >= position.character {
                    return Some(offset + index);
                }
                utf16 = utf16.saturating_add(character.len_utf16() as u32);
            }
            return Some(offset + segment.trim_end_matches('\n').len());
        }
        line += 1;
        offset += segment.len();
    }
    if line == position.line {
        Some(offset)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compile_document, file_kind, lsp_range, offset_at, position_at, word_at_position, FileKind,
        ServerState,
    };
    use tower_lsp::lsp_types::{NumberOrString, Position, Url};
    use utilitycss_span::Span;

    #[test]
    fn detects_source_kinds_from_editor_paths() {
        assert!(matches!(file_kind("src/App.tsx"), FileKind::JavaScript(_)));
        assert!(matches!(file_kind("src/App.vue"), FileKind::Framework(_)));
        assert!(matches!(file_kind("src/App.html"), FileKind::Framework(_)));
    }

    #[test]
    fn converts_utf8_byte_spans_to_utf16_lsp_positions() {
        let source = "é\n<div class=\"p-4\">";
        let start = source.find("p-4").expect("candidate exists") as u32;
        let range = lsp_range(source, Span::new(start, start + 3).expect("span is valid"));

        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 12);
        assert_eq!(position_at(source, start), range.start);
    }

    #[test]
    fn maps_lsp_positions_and_extracts_hover_words() {
        let source = "<div class=\"p-4\">";
        let position = Position { line: 0, character: 13 };

        assert_eq!(offset_at(source, position), Some(13));
        assert_eq!(word_at_position(source, position).as_deref(), Some("p-4"));
        assert!(Url::parse("file:///src/App.tsx").is_ok());
    }

    #[test]
    fn compiles_editor_documents_with_host_specific_extraction() {
        let uri = Url::parse("file:///src/Button.tsx").expect("URI is valid");
        let mut state = ServerState::default();
        let diagnostics = compile_document(
            &mut state,
            &uri,
            r#"export const Button = () => <button className="flex p-4" />;"#,
        );

        assert!(diagnostics.is_empty());
        let css = state.compiler.build().css().to_owned();
        assert!(css.contains(".flex{"));
        assert!(css.contains(".p-4{"));
    }

    #[test]
    fn reports_source_spans_for_invalid_editor_candidates() {
        let uri = Url::parse("file:///src/App.html").expect("URI is valid");
        let mut state = ServerState::default();
        let diagnostics = compile_document(&mut state, &uri, r#"<div class="p-[]"></div>"#);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_ref().map(|code| match code {
                NumberOrString::String(code) => code.clone(),
                NumberOrString::Number(code) => code.to_string(),
            }),
            Some("syntax.empty-arbitrary-value".to_owned())
        );
        assert_eq!(diagnostics[0].range.start.line, 0);
    }
}
