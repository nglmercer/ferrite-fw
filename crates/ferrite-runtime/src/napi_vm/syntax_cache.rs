//! Bounded, engine-versioned syntax results. No AST or guest state is retained.
use super::NAPI_VM_PIN;
use indexmap::IndexMap;

type SyntaxResult = std::result::Result<(), String>;
const MAX_ENTRIES: usize = 256;
const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub(super) struct SyntaxCache {
    entries: IndexMap<(&'static str, String), SyntaxResult>,
    bytes: usize,
}
impl SyntaxCache {
    pub fn check(&mut self, code: &str) -> SyntaxResult {
        self.check_with(code, NAPI_VM_PIN, parse)
    }
    fn check_with(
        &mut self,
        code: &str,
        engine: &'static str,
        parse: impl FnOnce(&str) -> SyntaxResult,
    ) -> SyntaxResult {
        // Exact source equality prevents hash collisions from bypassing parsing.
        let key = (engine, code.to_owned());
        if let Some(result) = self.entries.shift_remove(&key) {
            self.entries.insert(key, result.clone());
            return result;
        }
        let result = parse(code);
        let cost = code.len() + result.as_ref().err().map_or(0, String::len);
        if cost <= MAX_BYTES {
            while self.entries.len() >= MAX_ENTRIES || self.bytes + cost > MAX_BYTES {
                let Some(((.., code), result)) = self.entries.shift_remove_index(0) else {
                    break;
                };
                self.bytes -= code.len() + result.as_ref().err().map_or(0, String::len);
            }
            self.bytes += cost;
            self.entries.insert(key, result.clone());
        }
        result
    }
}
fn parse(code: &str) -> SyntaxResult {
    let mut lexer = napi_vm::lexer::Lexer::new(code);
    let mut parser = napi_vm::parser::Parser::new_with_spans(lexer.tokenize_with_spans());
    parser
        .parse_program()
        .map(|_| ())
        .map_err(|error| error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_source_engine_errors_and_eviction_are_respected() {
        let mut cache = SyntaxCache::default();
        assert!(cache.check("export const ok = 1;").is_ok());
        assert!(cache
            .check_with("export const ok = 1;", NAPI_VM_PIN, |_| panic!(
                "cache miss"
            ))
            .is_ok());
        let error = cache.check("export function broken( {").unwrap_err();
        assert_eq!(
            cache
                .check_with("export function broken( {", NAPI_VM_PIN, |_| panic!(
                    "error cache miss"
                ))
                .unwrap_err(),
            error
        );
        assert!(cache
            .check_with("export const ok = 1;", "other-engine", |_| Err(
                "different grammar".into()
            ))
            .is_err());
        for index in 0..MAX_ENTRIES {
            cache.check(&format!("export const n = {index};")).unwrap();
        }
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert!(cache
            .entries
            .keys()
            .all(|(_, code)| code != "export const ok = 1;"));
        cache
            .check_with(&" ".repeat(MAX_BYTES + 1), NAPI_VM_PIN, |_| Ok(()))
            .unwrap();
        assert!(cache.bytes <= MAX_BYTES);
    }
    #[test]
    #[ignore = "explicit parser preflight measurement"]
    fn parser_preflight_cold_and_cached_measurement() {
        let code: String = (0..2000)
            .map(|index| format!("export function fn{index}(n) {{ return n + {index}; }}\n"))
            .collect();
        let started = std::time::Instant::now();
        for _ in 0..20 {
            parse(std::hint::black_box(&code)).unwrap();
        }
        let cold = started.elapsed();
        let mut cache = SyntaxCache::default();
        cache.check(&code).unwrap();
        let started = std::time::Instant::now();
        for _ in 0..20 {
            cache.check(std::hint::black_box(&code)).unwrap();
        }
        eprintln!("parser preflight: bytes={}, iterations=20, cold={cold:?}, cached={:?}, engine={NAPI_VM_PIN}", code.len(), started.elapsed());
    }
}
