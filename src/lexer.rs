use std::{collections::HashMap, sync::LazyLock};
use strum::IntoEnumIterator;
use crate::CursorIter;

static KEYWORDS: LazyLock<HashMap<String, KeywordKind>> = LazyLock::new(|| {
  let mut map = HashMap::new();

  for name in KeywordKind::iter() {
    map.insert(name.to_string(), name);
  }

  map
});

#[derive(Debug, Default, Clone)]
pub struct Span {
  start: u16,
  end: u16,
}
impl Span {
  pub fn len(&self) -> u16 { self.end - self.start }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumIter, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum KeywordKind {
  If, Else, While,
  Fn, Return,
  True, False,
  And, Or, Not,
  Int, Float, Bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum TokenKind {
  #[default]
  Err,

  Assign,
  Decl,
  Comma,
  Dot,
  Colon,
  Semicolon,
  TokQuote,
  Tok2Quotes,
  Arrow,
  
  ParenL, ParenR,
  BraceL, BraceR,
  CurlyL, CurlyR,

  Plus, Star, Minus, Slash, Perc, Caret,
  Eq, Bang, NotEq, Great, Less, GreatEq, LessEq,

  Keyword(KeywordKind),
  Ident,
  IntLit(u64),
  FloatLit(f64),
}
impl TokenKind {
  pub fn is_op(&self) -> bool {
    use TokenKind::*;
    
    match self {
      Plus | Star | Minus | Slash | Perc | Caret |
      ParenL | BraceL | Dot |
      Eq | NotEq | Great | GreatEq | Less | LessEq |
      Keyword(KeywordKind::And) | Keyword(KeywordKind::Or) |
      Keyword(KeywordKind::Not) => true,
      _ => false
    }
  }

  pub fn is_safe(&self) -> bool {
    use TokenKind::*;
    
    match self {
      CurlyL | Keyword(KeywordKind::If)|
      Keyword(KeywordKind::While) | Keyword(KeywordKind::Fn) => true,
      _ => false,
    }
  }
}

#[derive(Debug, Default, Clone)]
pub struct Token {
  pub kind: TokenKind,
  pub info: Span,
}

pub struct Lexer {
  pub line_offsets: Vec<u16>,
  pub tokens: Vec<Token>,
}

pub struct Cursor<'a> {
  bytes: &'a [u8],
  curr: usize,
}

impl<'a> CursorIter<u8, char> for Cursor<'a> {
  fn peek_nth(&self, nth: usize) -> char {
    self.bytes.get(self.curr + nth).copied().unwrap_or_default() as char
  }

  fn start(&self) -> &[u8] { self.bytes }
  fn curr(&self) -> usize { self.curr }
  fn curr_mut(&mut self) -> &mut usize { &mut self.curr }
}

impl<'a> Cursor<'a> {
//   pub fn peek(&self) -> char {
//     self.peek_nth(0)
//   }

//   pub fn peek_nth(&self, nth: usize) -> char {
//     self.bytes.get(self.curr + nth).copied().unwrap_or_default() as char
//   }

  pub fn match2_or1(&mut self, target: char, m: TokenKind, o: TokenKind) -> TokenKind  {
    if self.peek_nth(1) == target {
      // eat second char
      self.advance();
      m
    } else {
      o
    }
  }

//   pub fn slice(&self) -> &[u8] { &self.bytes[self.curr..] }
//   pub fn at_end(&self) -> bool { self.curr >= self.bytes.len() }
}

pub fn tokenize(src: &str) -> Lexer {
  let mut lexer = Lexer { tokens: Vec::new(), line_offsets: Vec::new() };
  let mut cursor = Cursor {bytes: src.as_bytes(), curr: 0};
  
  'start: while !cursor.at_end() {
    let spaces = cursor.slice().iter()
      .take_while(|c| c.is_ascii_whitespace() && **c != b'\n')
      .count();
    cursor.advance_nth(spaces);
    if cursor.at_end() { break }

    let start = cursor.curr();
    let mut len = 1;
    let c = cursor.peek();

    let kind = match c {
      '+' => TokenKind::Plus,
      '*' => TokenKind::Star,
      '%' => TokenKind::Perc,
      '^' => TokenKind::Caret,

      '(' => TokenKind::ParenL,
      ')' => TokenKind::ParenR,
      '[' => TokenKind::BraceL,
      ']' => TokenKind::BraceR,
      '{' => TokenKind::CurlyL,
      '}' => TokenKind::CurlyR,

      ',' => TokenKind::Comma,
      '.' => TokenKind::Dot,
      ';' => TokenKind::Semicolon,
      
      '-' => cursor.match2_or1('>', TokenKind::Arrow, TokenKind::Minus),
      ':' => cursor.match2_or1('=', TokenKind::Decl, TokenKind::Colon),
      '=' => cursor.match2_or1('=', TokenKind::Eq, TokenKind::Assign),
      '!' => cursor.match2_or1('=', TokenKind::Bang, TokenKind::NotEq),
      '<' => cursor.match2_or1('=', TokenKind::LessEq, TokenKind::Less),
      '>' => cursor.match2_or1('=', TokenKind::GreatEq, TokenKind::Great),

      '/' => if cursor.peek_nth(1) == '/' {
        len = cursor.slice().iter()
          .take_while(|c| **c != b'\n')
          .count();

        cursor.advance_nth(len);
        continue 'start;
      } else if cursor.peek_nth(1) == '*' {
        todo!("block comment")
      } else {
        TokenKind::Slash
      }

      c if c.is_alphabetic() => {
        len = cursor.slice().iter()
          .take_while(|c| c.is_ascii_alphanumeric())
          .count();

        let ident = &src[start..start+len];
        if let Some(keyword) = KEYWORDS.get(ident) {
          TokenKind::Keyword(keyword.clone())
        } else {
          TokenKind::Ident
        }
      }

      c if c.is_ascii_digit() => {
        len = 0;
        while cursor.peek_nth(len).is_ascii_digit() { len += 1; }
        if cursor.peek_nth(len) == '.' {
          // float
          len += 1;
          while cursor.peek_nth(len).is_ascii_digit() { len += 1; }
          let n: f64 = src[start..start + len].parse().unwrap();
          TokenKind::FloatLit(n)
        } else {
          // int
          let n: u64 = src[start..start + len].parse().unwrap();
          TokenKind::IntLit(n)
        }
      }

      '\n' => {
        lexer.line_offsets.push(start as u16);
        cursor.advance();
        continue 'start;
      }

      _ => TokenKind::Err,
    };

    let t = Token {
      kind,
      info: Span { start: start as u16, end: (start + len) as u16 }
    };

    lexer.tokens.push(t);
    cursor.advance_nth(len);
  }

  lexer
}