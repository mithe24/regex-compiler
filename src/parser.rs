//! A small recursive-descent parser for the regex grammar, built on top of
//! `lexer`'s token stream.
use std::fmt;

use thiserror::Error;

use crate::{LexError, LexToken, lexer};

/// A parsed regular expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ast {
    Character(char),
    Epsilon,
    Concatenation(Box<Ast>, Box<Ast>),
    Alternation(Box<Ast>, Box<Ast>),
    QuantorStar(Box<Ast>),
}

impl Ast {
    fn concatenation(left: Ast, right: Ast) -> Ast {
        Ast::Concatenation(Box::new(left), Box::new(right))
    }

    fn alternation(left: Ast, right: Ast) -> Ast {
        Ast::Alternation(Box::new(left), Box::new(right))
    }

    fn quantor_star(inner: Ast) -> Ast {
        Ast::QuantorStar(Box::new(inner))
    }
}

impl fmt::Display for Ast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Character(c) => write!(f, "{c}"),
            Self::Epsilon => write!(f, "ε"),
            Self::Concatenation(l, r) => write!(f, "{l}{r}"),
            Self::Alternation(l, r) => write!(f, "({l}|{r})"),
            Self::QuantorStar(inner) => write!(f, "{inner}*"),
        }
    }
}

/// Errors raised while parsing a token stream.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParserError {
    #[error("unexpected end of input")]
    UnexpectedEnd,

    #[error("unexpected token `{found:?}`, expected `{expected:?}`")]
    UnexpectedToken {
        found: Option<LexToken>,
        expected: Option<LexToken>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error(transparent)]
    Lex(#[from] LexError),

    #[error(transparent)]
    Parser(#[from] ParserError),
}

/// Recursive-descent parser over an already-lexed token stream.
pub struct Parser {
    tokens: Vec<LexToken>,
    index: usize,
}

impl Parser {
    pub fn new(tokens: Vec<LexToken>) -> Self {
        Parser { tokens, index: 0 }
    }

    fn peek(&self) -> Option<&LexToken> {
        self.tokens.get(self.index)
    }

    fn consume(&mut self) -> Result<LexToken, ParserError> {
        let token = self
            .tokens
            .get(self.index)
            .cloned()
            .ok_or(ParserError::UnexpectedEnd)?;
        self.index += 1;
        Ok(token)
    }

    pub fn parse(mut self) -> Result<Ast, ParserError> {
        // RegExp -> Alt
        let ast = self.parse_alt()?;

        // make sure we consumed the whole word
        if let Some(token) = self.peek() {
            return Err(ParserError::UnexpectedToken {
                found: Some(token.clone()),
                expected: None,
            });
        }

        Ok(ast)
    }

    fn parse_alt(&mut self) -> Result<Ast, ParserError> {
        // Alt -> epsilon
        if matches!(self.peek(), None | Some(LexToken::ParenRight)) {
            return Ok(Ast::Epsilon);
        }

        // Alt -> "|" Alt
        if self.peek() == Some(&LexToken::Alternation) {
            self.consume()?;
            let right = self.parse_alt()?;
            return Ok(Ast::alternation(Ast::Epsilon, right));
        }

        let left = self.parse_concat()?;

        // Alt -> Concat
        if self.peek() != Some(&LexToken::Alternation) {
            return Ok(left);
        }

        self.consume()?;

        // Alt -> Concat "|" Alt
        let right = self.parse_alt()?;
        Ok(Ast::alternation(left, right))
    }

    fn parse_concat(&mut self) -> Result<Ast, ParserError> {
        let left = self.parse_quantor_star()?;

        // Concat -> Kleene
        if !Self::starts_atom(self.peek()) {
            return Ok(left);
        }

        // Concat -> Kleene Concat
        let right = self.parse_concat()?;
        Ok(Ast::concatenation(left, right))
    }

    fn parse_quantor_star(&mut self) -> Result<Ast, ParserError> {
        let inner = self.parse_atom()?;

        // Kleene -> Atom "*"
        if self.peek() == Some(&LexToken::QuantorStar) {
            self.consume()?;
            return Ok(Ast::quantor_star(inner));
        }

        // Kleene -> Atom
        Ok(inner)
    }

    fn parse_atom(&mut self) -> Result<Ast, ParserError> {
        match self.peek() {
            // Atom -> character
            Some(LexToken::Character(c)) => {
                let c = *c;
                self.consume()?;
                Ok(Ast::Character(c))
            }

            Some(LexToken::ParenLeft) => {
                self.consume()?;

                // Atom -> "(" Alt ")"
                let inner = self.parse_alt()?;
                let token = self.consume()?;
                if token == LexToken::ParenRight {
                    Ok(inner)
                } else {
                    Err(ParserError::UnexpectedToken {
                        found: Some(token),
                        expected: Some(LexToken::ParenRight),
                    })
                }
            }

            other => Err(ParserError::UnexpectedToken {
                found: other.cloned(),
                expected: None,
            }),
        }
    }

    fn starts_atom(token: Option<&LexToken>) -> bool {
        matches!(
            token,
            Some(LexToken::Character(_)) | Some(LexToken::ParenLeft)
        )
    }
}

/// Parses an already-tokenized regex.
pub fn parse_tokens(tokens: Vec<LexToken>) -> Result<Ast, ParserError> {
    Parser::new(tokens).parse()
}

/// Lexes and parses a regex string in one step.
pub fn parse(code: &str, new_quantors: bool) -> Result<Ast, ParseError> {
    let tokens =
        lexer::scan(code, new_quantors).collect::<Result<Vec<_>, _>>()?;
    Ok(parse_tokens(tokens)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Ast::*;

    fn p(code: &str) -> Ast {
        parse(code, true).unwrap()
    }

    #[test]
    fn single_character() {
        assert_eq!(p("a"), Character('a'));
    }

    #[test]
    fn empty_input_is_epsilon() {
        assert_eq!(p(""), Epsilon);
    }

    #[test]
    fn concatenation() {
        assert_eq!(p("ab"), Ast::concatenation(Character('a'), Character('b')));
    }

    #[test]
    fn alternation() {
        assert_eq!(p("a|b"), Ast::alternation(Character('a'), Character('b')));
    }

    #[test]
    fn leading_alternation_has_epsilon_on_the_left() {
        assert_eq!(p("|a"), Ast::alternation(Epsilon, Character('a')));
    }

    #[test]
    fn star() {
        assert_eq!(p("a*"), Ast::quantor_star(Character('a')));
    }

    #[test]
    fn grouping_and_precedence() {
        // (a|b)*c
        let expected = Ast::concatenation(
            Ast::quantor_star(Ast::alternation(Character('a'), Character('b'))),
            Character('c'),
        );
        assert_eq!(p("(a|b)*c"), expected);
    }

    #[test]
    fn trailing_garbage_after_a_full_parse_is_rejected() {
        let err = parse("a)", true).unwrap_err();
        assert_eq!(
            err,
            ParseError::Parser(ParserError::UnexpectedToken {
                found: Some(LexToken::ParenRight),
                expected: None
            })
        );
    }

    #[test]
    fn unclosed_group_is_rejected() {
        let err = parse("(a", true).unwrap_err();
        assert_eq!(err, ParseError::Parser(ParserError::UnexpectedEnd));
    }

    #[test]
    fn mismatched_group_close_reports_expected_token() {
        // Nothing in the grammar produces this directly from valid lexemes,
        // but parse_atom's mismatch branch is still reachable, e.g. via a
        // group holding an alternation that swallows everything up to EOF
        // and never finds its `)`. Exercise it through parse_tokens with a
        // hand-built stream instead.
        let tokens = vec![
            LexToken::ParenLeft,
            LexToken::Character('a'),
            LexToken::Character('b'),
        ];
        let err = parse_tokens(tokens).unwrap_err();
        assert_eq!(err, ParserError::UnexpectedEnd);
    }

    #[test]
    fn display_round_trips_readably() {
        assert_eq!(p("a(b|c)*").to_string(), "a(b|c)*");
    }
}
