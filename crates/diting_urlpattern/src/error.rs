// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::fmt;

// diting: error Display texts are liburlpattern's exact wording — the
// measured Chrome truth pins them byte-for-byte ("Missing parameter name
// at index 1." etc). Positions are char indices; Chrome counts UTF-16
// units, which only diverges for astral characters in the pattern.

/// A error occurring during URL pattern construction, or matching.
#[derive(Debug)]
pub enum Error {
    BaseUrlRequired,
    BaseUrlWithInit,
    /// a fully-worded detail message (tokenizer/parser/canonicalizer)
    Message(String),
    Tokenizer(TokenizerError, usize),
    Parser(ParserError),
    Url(url::ParseError),
    RegExp(()),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BaseUrlRequired => {
                f.write_str("a relative input without a base URL is not valid")
            }
            Error::BaseUrlWithInit => {
                f.write_str("specifying both an init object, and a separate base URL is not valid")
            }
            Error::Message(m) => f.write_str(m),
            Error::Tokenizer(err, pos) => write!(f, "{}", err.at(*pos)),
            Error::Parser(err) => write!(f, "{err}"),
            Error::Url(err) => err.fmt(f),
            Error::RegExp(_) => f.write_str("regexp error"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub enum TokenizerError {
    IncompleteEscapeCode,
    InvalidName,
    NonAscii(char, usize),
    RegexLeadingQuestion,
    UnnamedGroup(usize),
    UnbalancedRegex,
    MissingRegex,
}

impl TokenizerError {
    fn at(&self, pos: usize) -> String {
        match self {
            Self::IncompleteEscapeCode => {
                format!("Trailing escape character at index {pos}.")
            }
            Self::InvalidName => format!("Missing parameter name at index {pos}."),
            Self::NonAscii(c, at) => format!(
                "Invalid non-ASCII character 0x{:x} at index {at}.",
                *c as u32
            ),
            Self::RegexLeadingQuestion => {
                format!("Regex cannot start with '?' at index {pos}")
            }
            Self::UnnamedGroup(at) => {
                format!("Unnamed capturing groups are not allowed at index {at}.")
            }
            Self::UnbalancedRegex => format!("Unbalanced regex at index {pos}."),
            Self::MissingRegex => format!("Missing regex at index {pos}."),
        }
    }
}

impl fmt::Display for TokenizerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.at(0))
    }
}

impl std::error::Error for TokenizerError {}

#[derive(Debug)]
pub enum ParserError {
    ExpectedToken(TokenType, TokenType, String, usize),
    DuplicateName(String, usize),
}

fn token_name(ty: &crate::tokenizer::TokenType) -> &'static str {
    use crate::tokenizer::TokenType::*;
    match ty {
        Open => "'{'",
        Close => "'}'",
        End => "end of pattern",
        Char => "character",
        EscapedChar => "escaped character",
        Name => "parameter name",
        Regexp => "regex",
        Asterisk => "'*'",
        OtherModifier => "modifier",
        InvalidChar => "invalid character",
    }
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedToken(expected_ty, found_ty, found_val, found_index) => {
                write!(
                    f,
                    "Unexpected {} '{}' at index {}, expected {}.",
                    token_name(found_ty),
                    found_val,
                    found_index,
                    token_name(expected_ty),
                )
            }
            Self::DuplicateName(name, at) => {
                write!(f, "Duplicate group name '{name}' at index {at}.")
            }
        }
    }
}

impl std::error::Error for ParserError {}

use crate::tokenizer::TokenType;
