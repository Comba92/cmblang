use std::{error::{self, Error}, fmt, fs};

pub type Err = FrontendErr;

#[test]
fn partition() {
  let vec = (0..10).step_by(2).collect::<Vec<_>>();
  let res = vec.binary_search(&1);
  println!("{res:?}");
}

#[derive(Debug, Clone)]
pub struct FrontendErr {
  msg: String,
  span: lexer::Span,
  token: String,
  column: usize,
  line: usize,
}
impl FrontendErr {
  pub fn new(msg: String, span: lexer::Span, lexer: &lexer::Lexer) -> Self {
    let line = match lexer.line_offsets.binary_search(&span.start) {
      // token starts exactly at line, return as is
      Ok(idx) => idx,
      // token starts inside line, search returned next line
      // 0 shouldn't be returned so it is safe to subtract 1
      Err(idx) => idx - 1,
    };

    let column = (span.start - lexer.line_offsets[line]) as usize;
    
    Self {
      msg,
      token: span.slice(lexer.src).to_owned(),
      span,
      line,
      column
    }
  }
}

impl fmt::Display for FrontendErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{} at token: '{}', line: {}, column: {}", self.msg, self.token, self.line, self.column)
  }
}
impl error::Error for FrontendErr {}

trait CursorIter<Inner, Mapped> {
  fn peek(&self) -> Mapped { self.peek_nth(0) }
  fn peek_nth(&self, nth: usize) -> Mapped;
  fn advance(&mut self) { self.advance_nth(1); }
  fn advance_nth(&mut self, n: usize) { *self.curr_mut() += n; }
  fn eat(&mut self) -> Mapped {
    let res = self.peek();
    self.advance();
    res
  }

  fn start(&self) -> &[Inner];
  fn slice(&self) -> &[Inner] { &self.start()[self.curr()..] }
  fn curr(&self) -> usize;
  fn curr_mut(&mut self) -> &mut usize;
  fn at_end(&self) -> bool { self.curr() >= self.start().len() }
}

mod lexer;
mod parser;

fn main() -> Result<(), Box<dyn Error>> {
  let src = fs::read_to_string("test.cmb")?;

  let parser = parser::parse(&src);
  parser.tokens.iter().for_each(|t| println!("{t:?}"));
  parser.exprs.iter().for_each(|e| println!("{e:?}"));
  parser.stmts.iter().for_each(|s| println!("{s:?}"));

  Ok(())
}