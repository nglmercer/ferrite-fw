//! Small, runtime-independent source location types shared by compiler crates.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fmt;

use serde::Serialize;

/// Maximum source length representable by a [`Span`] offset.
pub const MAX_SOURCE_BYTES: usize = u32::MAX as usize;

/// Error returned when a source cannot be represented by the 32-bit span model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSizeError {
    length: usize,
}

impl SourceSizeError {
    /// Returns the rejected source length in bytes.
    #[must_use]
    pub const fn length(self) -> usize {
        self.length
    }
}

impl fmt::Display for SourceSizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "source is {} bytes, but source locations support at most {} bytes",
            self.length, MAX_SOURCE_BYTES
        )
    }
}

impl std::error::Error for SourceSizeError {}

/// Checks whether a source length can be represented by [`Span`] offsets.
pub fn validate_source_len(length: usize) -> Result<(), SourceSizeError> {
    if length > MAX_SOURCE_BYTES {
        Err(SourceSizeError { length })
    } else {
        Ok(())
    }
}

/// A stable identity for a source unit supplied to the compiler.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SourceId(String);

impl SourceId {
    /// Creates a source identity from a caller-owned or caller-generated string.
    ///
    /// Source IDs are opaque to the compiler. Callers should keep them stable across
    /// incremental updates; a path can be used as metadata but does not have to be the ID.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the source identity as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the owned source identity.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A half-open byte range within a source unit: `[start, end)`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Span {
    start: u32,
    end: u32,
}

impl Span {
    /// Creates a span if `start` does not come after `end`.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Option<Self> {
        if start <= end {
            Some(Self { start, end })
        } else {
            None
        }
    }

    /// Creates an empty span at a byte offset.
    #[must_use]
    pub const fn empty(at: u32) -> Self {
        Self { start: at, end: at }
    }

    /// Returns the first byte offset in the span.
    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    /// Returns the exclusive end byte offset in the span.
    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }

    /// Returns the span length in bytes.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    /// Returns whether the span contains no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Returns whether a byte offset lies inside this span.
    #[must_use]
    pub const fn contains(self, offset: u32) -> bool {
        self.start <= offset && offset < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_source_len, SourceId, Span, MAX_SOURCE_BYTES};

    #[test]
    fn source_ids_are_orderable_and_displayable() {
        let first = SourceId::new("src/a.html");
        let second = SourceId::new("src/b.html");

        assert!(first < second);
        assert_eq!(first.to_string(), "src/a.html");
    }

    #[test]
    fn spans_are_half_open() {
        let span = Span::new(4, 9).expect("valid span");

        assert_eq!(span.start(), 4);
        assert_eq!(span.end(), 9);
        assert_eq!(span.len(), 5);
        assert!(span.contains(4));
        assert!(span.contains(8));
        assert!(!span.contains(9));
        assert!(!span.is_empty());
    }

    #[test]
    fn reversed_spans_are_rejected_without_panicking() {
        assert_eq!(Span::new(9, 4), None);
    }

    #[test]
    fn source_lengths_are_checked_before_span_conversion() {
        assert!(validate_source_len(MAX_SOURCE_BYTES).is_ok());
        let error = validate_source_len(MAX_SOURCE_BYTES.saturating_add(1))
            .expect_err("oversized source is not representable");
        assert_eq!(error.length(), MAX_SOURCE_BYTES.saturating_add(1));
    }
}
