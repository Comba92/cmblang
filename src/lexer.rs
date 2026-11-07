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
    Under = b'_',

    Invalid = 128,
    Eof,

    Ident,
    IntNumber,
    FloatNumber,

    Arrow,
    DotDot,
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
    Break,
    Continue,
    Return,

    Module,

    True,
    False,

    And,
    Or,
    Not,
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

    pub fn to_c_op(&self) -> Option<&str> {
        use TokenKind::*;

        let res = match self {
            Plus => "+",
            Minus => "-",
            Star => "*",
            Slash => "/",
            Perc => "%",
            Bang => "!",
            Amper => "&",
            Bar => "|",
            Not => "~",
            Caret => "^",
            Equal => "=",
            Less => ">",
            Great => "<",
            EqualEq => "==",
            NotEq => "!=",
            LessEq => "<=",
            GreatEq => ">=",
            And => "&&",
            Or => "||",
            Arrow => "->",
            _ => return None,
        };

        Some(res)
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
    ("module", TokenKind::Module),
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

// 64bits Token. Chef kiss
#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub kind: TokenKind,
    pub len: u16,
    pub start: u32,
}
impl Default for Token {
    fn default() -> Self {
        Self {
            kind: TokenKind::Invalid,
            start: 0,
            len: 0,
        }
    }
}
impl Token {
    pub fn new(kind: TokenKind, start: u32, end: u32) -> Self {
        let len = end - start;
        Self {
            kind,
            start,
            len: len as u16,
        }
    }

    pub fn to_span(&self) -> Span {
        Span {
            start: self.start,
            end: self.start + self.len as u32,
        }
    }
}

#[derive(Debug)]
pub enum LexError {
    InputTooBig,
    TokenTooBig(Position),
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
    pub fn token_pos(&self, tok: Token) -> Positioned<TokenKind> {
        (tok.kind, self.pos_from_tok(tok))
    }

    pub fn str_from_tok(&self, tok: Token) -> &str {
        self.str_from_span(Span {
            start: tok.start,
            end: tok.start + tok.len as u32,
        })
    }

    pub fn str_from_span(&self, span: Span) -> &str {
        &self.input[span.start as usize..span.end as usize]
    }

    pub fn pos_from_tok(&self, tok: Token) -> Position {
        self.pos_from_span(tok.to_span())
    }

    pub fn pos_from_span(&self, span: Span) -> Position {
        let (line_start, col_start) = self.pos_from_offset(span.start);
        let (line_end, col_end) = self.pos_from_offset(span.end);

        Position {
            line_start,
            line_end,
            col_start,
            col_end,
        }
    }

    fn pos_from_offset(&self, offset: u32) -> (u32, u32) {
        let line = match self.line_offsets.binary_search(&offset) {
            // token starts exactly at line, return as is
            Ok(idx) => idx,
            // token starts inside line, search returned next line
            // 0 shouldn't be returned so it is safe to subtract 1
            Err(idx) => idx - 1,
        };

        let column = (offset - self.line_offsets[line]) as usize;
        (1 + line as u32, column as u32)
    }

    pub fn str_line_from_pos(&self, pos: &Position) -> &str {
        &self
            .input
            .lines()
            .nth(pos.line_start as usize - 1)
            .unwrap_or_default()
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

    // return last character and its start offset
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
                '.' => cursor.eat_double_or_single('=', TokenKind::DotDot, TokenKind::Dot),
                ':' => TokenKind::Colon,
                ';' => TokenKind::Semicolon,
                '_' => TokenKind::Under,
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
                            let pos = lex.pos_from_span(Span {
                                start,
                                end: cursor.pos,
                            });

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

        // TODO: len might be bigger than u16? what should happen in that case?
        lex.tokens.push(Token::new(kind, start, cursor.pos));
    }

    // BE CAREFUL: EOF character isn't really there, meaning you can't slice it from the input string.
    // By having len = 0 (start = end), the slice will be empty so no out of bounds accesses can occur
    lex.tokens
        .push(Token::new(TokenKind::Eof, cursor.pos, cursor.pos));

    Ok(lex)
}
