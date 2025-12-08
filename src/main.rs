use std::{error::{self, Error}, fmt, fs};

pub type FrontendErrAlias = FrontendErr;
pub type IdSize = u32;

mod lexer;
mod parser;
mod ast;
mod typecheck;

#[derive(Debug, Clone)]
pub struct FrontendErr {
  msg: String,
  span: lexer::Span,
  str: String,
  column: usize,
  line: usize,
}
impl FrontendErr {
  pub fn new(lexer: &lexer::Lexer, msg: String, span: lexer::Span) -> Self {
    // TODO: consider evaluating this lazily, computing column and line only when it is printed
    // TODO: is binary search really that fast?
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
      str: span.get_str(lexer.src).to_owned(),
      span,
      line,
      column
    }
  }
}

impl fmt::Display for FrontendErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{} at token: '{}', line: {}, column: {}", self.msg, self.str, self.line+1, self.column+1)
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
  fn has_some(&self) -> bool { self.curr() < self.start().len() }
}

fn main() -> Result<(), Box<dyn Error>> {
  let src = fs::read_to_string("test.cmb")?;

  let parser = parser::parse(&src);
  // parser.tokens.iter().enumerate().for_each(|(i, t)| println!("{i}: {t:?}"));
  // println!();
  // println!();

  // parser.exprs.iter() .enumerate().for_each(|(i, e)| println!("{i}: {e:?}"));
  // println!();  
  // parser.stmts.iter() .enumerate().for_each(|(i, s)| println!("{i}: {s:?}"));
  // println!();
  // parser.types.buf.iter() .enumerate().for_each(|(i, ty)| println!("{i}: {ty:?}"));
  println!("{:?}", parser.types);
  println!("{:?}", parser.idents);

  // let typecheck = typecheck::check(&parser);

  Ok(())
}