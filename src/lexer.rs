use std::{collections::HashMap, sync::LazyLock};
use strum::IntoEnumIterator;
use crate::{CursorIter, FrontendErr, IdSize};

static KEYWORDS: LazyLock<HashMap<String, KeywordKind>> = LazyLock::new(|| {
  let mut map = HashMap::new();

  for name in KeywordKind::iter() {
    map.insert(name.to_string(), name);
  }

  map
});

#[derive(Debug, Default, Clone, Copy)]
pub struct Span {
  pub start: u32,
  pub end: u32,
}
impl Span {
  pub fn len(&self) -> u32 { self.end - self.start }
  pub fn get_str<'a, 'b>(&'a self, src: &'b str) -> &'b str { &src[self.start as usize .. self.end as usize] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumIter, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum KeywordKind {
  If, Else, While,
  Fn, Return,
  Struct,
  True, False,
  And, Or, Not,
  Let, Const,
  Int, Float, Bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
  #[default]
  Err,
  Eof,

  Assign,
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

  IntLit,
  FloatLit,
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
    
    self.is_safe_toplvl() || match self {
      Comma | ParenL | BraceL | CurlyL |
      Keyword(KeywordKind::If) | Keyword(KeywordKind::Else) | Keyword(KeywordKind::While) => true,
      _ => false,
    }
  }

  pub fn is_safe_toplvl(&self) -> bool {
    use TokenKind::*;

    match self {
      Semicolon | Keyword(KeywordKind::Fn) | Keyword(KeywordKind::Struct) => true,
      _ => false,
    }
  }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Token {
  pub kind: TokenKind,
  pub span: Span,
}

pub struct Lexer<'a> {
  pub src: &'a str,
  pub line_offsets: Vec<u32>,
  pub tokens: Vec<Token>,
}
impl<'a> Lexer<'a> {
  pub fn eof(&self) -> Token {
    let span = Span { start: self.src.len() as u32, end: self.src.len() as u32 };
    Token { kind: TokenKind::Eof, span }
  }

  pub fn get_str(&self, t: Token) -> &str {
    &self.src[t.span.start as usize .. t.span.end as usize]
  }
}

pub struct Cursor<'a> {
  bytes: &'a [u8],
  curr: u32,
}

impl<'a> CursorIter<u8> for Cursor<'a> {
  fn peek_nth(&self, nth: usize) -> u8 {
    self.bytes.get(self.curr as usize + nth).copied().unwrap_or_default()
  }

  fn start(&self) -> &[u8] { self.bytes }
  fn curr(&self) -> u32 { self.curr }
  fn curr_mut(&mut self) -> &mut u32 { &mut self.curr }
}

impl<'a> Cursor<'a> {
  pub fn match2_or1(&mut self, target: char, m: TokenKind, o: TokenKind) -> TokenKind  {
    if self.peek_nth(1) == target as u8 {
      // eat second char
      self.advance();
      m
    } else { o }
  }

  pub fn eat_if(&mut self, target: u8) -> Option<u8> {
    (self.peek() == target).then(|| self.eat())
  }
}

pub fn tokenize(src: &str) -> Result<Lexer, FrontendErr> {
  let mut lexer = Lexer { src, tokens: vec![], line_offsets: vec![0] };
  let mut cursor = Cursor {bytes: src.as_bytes(), curr: 0};
  
  'start: while cursor.has_some() {
    let spaces = cursor.slice().iter()
      .take_while(|c| c.is_ascii_whitespace() && **c != b'\n')
      .count();
    cursor.advance_nth(spaces);
    if !cursor.has_some() { break }

    let start = cursor.curr() as usize;
    let mut len = 1;
    let c = cursor.peek() as char;

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
      ':' => TokenKind::Colon,
      ';' => TokenKind::Semicolon,
      
      '-' => cursor.match2_or1('>', TokenKind::Arrow, TokenKind::Minus),
      '=' => cursor.match2_or1('=', TokenKind::Eq, TokenKind::Assign),
      '!' => cursor.match2_or1('=', TokenKind::Bang, TokenKind::NotEq),
      '<' => cursor.match2_or1('=', TokenKind::LessEq, TokenKind::Less),
      '>' => cursor.match2_or1('=', TokenKind::GreatEq, TokenKind::Great),

      '/' => if cursor.peek_nth(1) as char == '/' {
        len = cursor.slice().iter()
          .take_while(|c| **c != b'\n')
          .count();

        cursor.advance_nth(len);
        continue 'start;
      } else if cursor.peek_nth(1) as char == '*' {
        let mut openings = vec![cursor.curr()];
        
        // eat first '/*'
        cursor.advance_nth(2);

        while cursor.has_some() {
          let c = cursor.eat() as char;
          if c == '/' && cursor.eat_if(b'*').is_some() {
            openings.push(cursor.curr()-2);
          } else if c == '*' && cursor.eat_if(b'/').is_some() {
            openings.pop();
          } else if c == '\n' {
            lexer.line_offsets.push(cursor.curr() as u32-1);
          }

          if openings.len() == 0 { break; }
        }

        if openings.len() > 0 {
          let start = openings.pop().unwrap() as u32;
          return Err(FrontendErr::new(&lexer, "unclosed block comment", Span { start, end: start + 2 }))
        }

        continue 'start;
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
        if cursor.peek_nth(len) as char == '.' {
          // float
          len += 1;
          while cursor.peek_nth(len).is_ascii_digit() { len += 1; }
          TokenKind::FloatLit
        } else {
          TokenKind::IntLit
        }
      }

      '\n' => {
        cursor.advance();
        lexer.line_offsets.push(cursor.curr() as u32);
        continue 'start;
      }

      _ => TokenKind::Err,
    };

    
    let span = Span { start: start as u32, end: (start + len) as u32 };
    lexer.tokens.push(Token { kind, span });
    
    cursor.advance_nth(len);
  }
  
  Ok(lexer)
}