use std::fmt;

use crate::*;

// OPT:
// https://alic.dev/blog/fast-lexing
// https://nothings.org/computer/lexing.html
// https://xnacly.me/posts/2025/fast-lexer-strategies/

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    ParenL = b'(',
    ParenR = b')',
    SquareL = b'[',
    SquareR = b']',
    BraceL = b'{',
    BraceR = b'}',
    Comma = b',',
    Dot = b'.',
    Colon = b':',
    Semicolon = b';',
    Equal = b'=',
    Bang = b'!',
    Question = b'?',
    Dollar = b'$',
    Hash = b'#',
    At = b'@',
    Amper = b'&',
    Bar = b'|',
    Caret = b'^',
    Plus = b'+',
    Minus = b'-',
    Star = b'*',
    Slash = b'/',
    Perc = b'%',
    Less = b'<',
    Great = b'>',

    Invalid = 128,
    Eof,

    Ident,
    IntNumber,
    FloatNumber,
    Arrow,

    // Comparisons
    EqualEq,
    NotEq,
    LessEq,
    GreatEq,

    // Assignment Ops
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    AmperEq,
    BarEq,
    CaretEq,

    // Keywords
    Let,
    Fn,
    Struct,
    If,
    Else,
    While,
    For,
    Return,
    True,
    False,
    And,
    Or,
    Not,
    Break,
    Continue,
}
impl TokenKind {
    pub fn is_op(&self) -> bool {
        use TokenKind::*;

        match self {
            Plus | Minus | Star | Slash | Perc | Amper | Bar | Caret | And | Or | ParenL
            | BraceL | Dot | EqualEq | NotEq | Great | GreatEq | Less | LessEq => true,
            _ => false,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if (*self as u8) < 128 {
            write!(f, "{}", *self as u8 as char)
        } else {
            let s = format!("{self:?}");
            write!(f, "{}", s.strip_prefix("TokenKind::").unwrap_or(&s))
        }
    }
}

const fn hash_keyword(s: &[u8]) -> Option<u64> {
    if s.len() > 8 {
        None
    } else {
        let mut h = 0;
        let mut it = 0;
        while it < s.len() {
            h = (h << 8) | s[it] as u64;
            it += 1;
        }

        Some(h)
    }
}

pub const KEYWORDS: &[(&'static str, TokenKind)] = &[
    ("let", TokenKind::Let),
    ("fn", TokenKind::Fn),
    ("struct", TokenKind::Struct),
    ("if", TokenKind::If),
    ("else", TokenKind::Else),
    ("while", TokenKind::While),
    ("for", TokenKind::For),
    ("return", TokenKind::Return),
    ("break", TokenKind::Break),
    ("continue", TokenKind::Continue),
    ("true", TokenKind::True),
    ("false", TokenKind::False),
    ("and", TokenKind::And),
    ("or", TokenKind::Or),
    ("not", TokenKind::Not),
];

// TODO: convert this to hashset
pub const KEYWORDS_HASHES: [u64; KEYWORDS.len()] = {
    let mut hashes = [0; _];
    let mut it = 0;
    while it < KEYWORDS.len() {
        hashes[it] = hash_keyword(KEYWORDS[it].0.as_bytes()).unwrap();
        it += 1;
    }
    hashes
};

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'{}' - {}:{}", self.kind, self.span.start, self.span.end)
    }
}

#[derive(Debug)]
enum LexError {
    InputTooBig,
    UnclosedBlockComment(Position),
}

#[derive(Debug)]
pub struct Lexer {
    pub input: String,
    // TODO: is this really needed? we might find the line directly by scanning for the next one...
    pub line_offsets: Vec<u32>,
    pub tokens: Vec<Token>,
}
impl Lexer {
    fn new(input: String) -> Self {
        Self {
            line_offsets: vec![0],
            tokens: vec![],
            input,
        }
    }
}

impl Lexer {
    pub fn line_and_column_from_offset(&self, offset: u32) -> Position {
        let line = match self.line_offsets.binary_search(&offset) {
            // token starts exactly at line, return as is
            Ok(idx) => idx,
            // token starts inside line, search returned next line
            // 0 shouldn't be returned so it is safe to subtract 1
            Err(idx) => idx - 1,
        };

        let column = (offset - self.line_offsets[line]) as usize;
        Position {
            line: line as u32,
            col: column as u32,
        }
    }

    pub fn token_string(&self, tok: &Token) -> &str {
        &self.input[tok.span.start as usize..tok.span.end as usize]
    }
}

struct LexCursor<'a> {
    bytes: &'a [u8],
    pos: u32,
}

impl<'a> LexCursor<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            bytes: s.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.bytes.get(self.pos as usize).map(|c| *c as char)
    }

    fn check(&self, target: char) -> bool {
        self.peek().filter(|c| *c == target).is_some()
    }

    fn advance(&mut self) -> Option<(char, u32)> {
        if let Some(c) = self.peek() {
            let pos = self.pos as u32;
            self.pos += 1;
            Some((c, pos))
        } else {
            None
        }
    }

    fn eat(&mut self, target: char) -> bool {
        if self.check(target) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_double_or_single(
        &mut self,
        target: char,
        double: TokenKind,
        single: TokenKind,
    ) -> TokenKind {
        if self.eat(target) { double } else { single }
    }

    fn skip_while<F: FnMut(&char) -> bool>(&mut self, f: F) -> u32 {
        let len = self.bytes[self.pos as usize..]
            .iter()
            .map(|c| *c as char)
            .take_while(f)
            .count();

        self.pos += len as u32;
        len as u32
    }

    fn slice(&self, start: u32) -> &[u8] {
        &self.bytes[start as usize..self.pos as usize]
    }
}

pub fn scan(input: String) -> Result<Lexer, LexError> {
    if input.len() > u32::MAX as usize {
        return Err(LexError::InputTooBig);
    }

    let mut lex = Lexer::new(input);
    let mut cursor = LexCursor::new(&lex.input);

    while let Some((c, start)) = cursor.advance() {
        let kind = if c.is_ascii_whitespace() {
            if c == '\n' {
                lex.line_offsets.push(start);
            }
            continue;
        } else if c.is_ascii_alphabetic() {
            cursor.skip_while(|c| c.is_ascii_alphanumeric() || *c == '_');

            let hash = hash_keyword(cursor.slice(start));
            if let Some(idx) = hash.and_then(|h| KEYWORDS_HASHES.iter().position(|x| *x == h)) {
                KEYWORDS[idx].1
            } else {
                TokenKind::Ident
            }
        } else if c.is_ascii_digit() {
            cursor.skip_while(|c| c.is_ascii_digit());
            if cursor.eat('.') {
                cursor.skip_while(|c| c.is_ascii_digit());
                TokenKind::FloatNumber
            } else {
                TokenKind::IntNumber
            }
        } else {
            match c {
                '(' => TokenKind::ParenL,
                ')' => TokenKind::ParenR,
                '[' => TokenKind::SquareL,
                ']' => TokenKind::SquareR,
                '{' => TokenKind::BraceL,
                '}' => TokenKind::BraceR,
                ',' => TokenKind::Comma,
                '.' => TokenKind::Dot,
                ':' => TokenKind::Colon,
                ';' => TokenKind::Semicolon,
                '?' => TokenKind::Question,
                '=' => cursor.eat_double_or_single('=', TokenKind::EqualEq, TokenKind::Equal),
                '!' => cursor.eat_double_or_single('=', TokenKind::NotEq, TokenKind::Bang),
                '<' => cursor.eat_double_or_single('=', TokenKind::LessEq, TokenKind::Less),
                '>' => cursor.eat_double_or_single('=', TokenKind::GreatEq, TokenKind::Great),
                '+' => TokenKind::Plus,
                '-' => cursor.eat_double_or_single('>', TokenKind::Arrow, TokenKind::Minus),
                '*' => TokenKind::Star,
                '%' => TokenKind::Perc,
                '/' => {
                    if cursor.eat('/') {
                        cursor.skip_while(|c| *c != '\n');
                        continue;
                    } else if cursor.eat('*') {
                        let mut depth = 1;

                        while let Some((c, start)) = cursor.advance() {
                            match c {
                                '\n' => lex.line_offsets.push(start),
                                '*' => {
                                    if cursor.eat('/') {
                                        depth -= 1;
                                        if depth <= 0 {
                                            break;
                                        }
                                    }
                                }
                                '/' => {
                                    if cursor.eat('*') {
                                        depth += 1;
                                    }
                                }
                                _ => {}
                            }
                        }

                        if depth > 0 {
                            let pos = lex.line_and_column_from_offset(start);
                            return Err(LexError::UnclosedBlockComment(pos));
                        }

                        continue;
                    } else {
                        TokenKind::Slash
                    }
                }

                _ => TokenKind::Invalid,
            }
        };

        lex.tokens.push(Token {
            kind,
            span: Span {
                start,
                end: cursor.pos,
            },
        });
    }

    lex.tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span {
            start: cursor.pos,
            end: cursor.pos + 1,
        },
    });

    Ok(lex)
}
