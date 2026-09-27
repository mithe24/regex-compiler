use std::collections::VecDeque;
use std::fmt;
use std::str::Chars;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexToken {
    ParenLeft,
    ParenRight,
    QuantorStar,
    Alternation,
    QuantorPlus,
    QuantorOptional,
    CurlyLeft,
    CurlyRight,
    Comma,
    Number(String),
    Character(char),
}

impl fmt::Display for LexToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParenLeft => write!(f, "("),
            Self::ParenRight => write!(f, ")"),
            Self::QuantorStar => write!(f, "*"),
            Self::Alternation => write!(f, "|"),
            Self::QuantorPlus => write!(f, "+"),
            Self::QuantorOptional => write!(f, "?"),
            Self::CurlyLeft => write!(f, "{{"),
            Self::CurlyRight => write!(f, "}}"),
            Self::Comma => write!(f, ","),
            Self::Number(digits) => write!(f, "{digits}"),
            Self::Character(c) => write!(f, "{c}"),
        }
    }
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexError {
    #[error("incomplete escape sequence at end of input")]
    IncompleteEscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Initial,
    Escape,
    Number,
}

/// Scans `code` into a stream of [`LexToken`]s.
///
/// `new_quantors` toggles the extended syntax (`+`, `?`, `{m,n}`); with it
/// off, `+`, `?`, `{`, `}`, `,` and digits are all just literal characters.
pub fn scan(code: &str, new_quantors: bool) -> Lexer<'_> {
    Lexer::new(code, new_quantors)
}

/// Lazily lexes a regex string. See [`scan`].
pub struct Lexer<'a> {
    chars: Chars<'a>,
    state: State,
    buffer: String,
    new_quantors: bool,
    queue: VecDeque<LexToken>,
    finished: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(code: &'a str, new_quantors: bool) -> Self {
        Lexer {
            chars: code.chars(),
            state: State::Initial,
            buffer: String::new(),
            new_quantors,
            queue: VecDeque::new(),
            finished: false,
        }
    }

    /// Tokenizes a single character while in `State::Initial`. Returns the
    /// token to emit, if any; otherwise it has moved `self.state` forward
    /// (into `Escape` or `Number`) with nothing to yield yet.
    fn step_char(&mut self, c: char) -> Option<LexToken> {
        match c {
            '\\' => {
                self.state = State::Escape;
                None
            }
            '(' => Some(LexToken::ParenLeft),
            ')' => Some(LexToken::ParenRight),
            '*' => Some(LexToken::QuantorStar),
            '|' => Some(LexToken::Alternation),

            '+' if self.new_quantors => Some(LexToken::QuantorPlus),
            '?' if self.new_quantors => Some(LexToken::QuantorOptional),
            '{' if self.new_quantors => Some(LexToken::CurlyLeft),
            '}' if self.new_quantors => Some(LexToken::CurlyRight),
            ',' if self.new_quantors => Some(LexToken::Comma),
            digit if self.new_quantors && digit.is_numeric() => {
                self.buffer.push(digit);
                self.state = State::Number;
                None
            }

            other => Some(LexToken::Character(other)),
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Result<LexToken, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(token) = self.queue.pop_front() {
            return Some(Ok(token));
        }

        loop {
            let Some(c) = self.chars.next() else {
                if self.finished {
                    return None;
                }
                self.finished = true;
                return match self.state {
                    State::Escape => Some(Err(LexError::IncompleteEscape)),
                    State::Number => Some(Ok(LexToken::Number(
                        std::mem::take(&mut self.buffer),
                    ))),
                    State::Initial => None,
                };
            };

            if self.state == State::Escape {
                self.state = State::Initial;
                return Some(Ok(LexToken::Character(c)));
            }

            if self.state == State::Number {
                if c.is_numeric() {
                    self.buffer.push(c);
                    continue;
                }

                let number = std::mem::take(&mut self.buffer);
                self.state = State::Initial;
                if let Some(token) = self.step_char(c) {
                    self.queue.push_back(token);
                }
                return Some(Ok(LexToken::Number(number)));
            }

            if let Some(token) = self.step_char(c) {
                return Some(Ok(token));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(code: &str, new_quantors: bool) -> Result<Vec<LexToken>, LexError> {
        scan(code, new_quantors).collect()
    }

    #[test]
    fn basic_tokens() {
        use LexToken::*;
        assert_eq!(
            lex("a(b|c)*", true).unwrap(),
            vec![
                Character('a'),
                ParenLeft,
                Character('b'),
                Alternation,
                Character('c'),
                ParenRight,
                QuantorStar,
            ]
        );
    }

    #[test]
    fn escape_sequence() {
        use LexToken::*;
        assert_eq!(lex(r"\(", true).unwrap(), vec![Character('(')]);
    }

    #[test]
    fn incomplete_escape_is_an_error() {
        assert_eq!(lex("a\\", true), Err(LexError::IncompleteEscape));
    }

    #[test]
    fn new_quantors_off_treats_them_as_literals() {
        use LexToken::*;
        assert_eq!(
            lex("a+?{1,2}", false).unwrap(),
            vec![
                Character('a'),
                Character('+'),
                Character('?'),
                Character('{'),
                Character('1'),
                Character(','),
                Character('2'),
                Character('}'),
            ]
        );
    }

    #[test]
    fn new_quantors_on_parses_plus_and_optional() {
        use LexToken::*;
        assert_eq!(
            lex("a+b?", true).unwrap(),
            vec![Character('a'), QuantorPlus, Character('b'), QuantorOptional]
        );
    }

    #[test]
    fn curly_bound_with_number_followed_by_symbol() {
        use LexToken::*;
        assert_eq!(
            lex("a{2,3}", true).unwrap(),
            vec![
                Character('a'),
                CurlyLeft,
                Number("2".to_string()),
                Comma,
                Number("3".to_string()),
                CurlyRight,
            ]
        );
    }

    #[test]
    fn number_at_end_of_input_is_flushed() {
        use LexToken::*;
        assert_eq!(
            lex("a{12", true).unwrap(),
            vec![Character('a'), CurlyLeft, Number("12".to_string())]
        );
    }

    #[test]
    fn multi_digit_number_followed_by_letter() {
        use LexToken::*;
        assert_eq!(
            lex("1a2", true).unwrap(),
            vec![
                Number("1".to_string()),
                Character('a'),
                Number("2".to_string())
            ]
        );
    }
}
