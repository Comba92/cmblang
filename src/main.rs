use std::{error, fs};

pub type Err = Box<dyn error::Error>;

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

fn main() -> Result<(), Err>{
  let src = fs::read_to_string("test.cmb")?;
  let lexer = lexer::tokenize(&src);
  lexer.tokens.iter().for_each(|t| println!("{t:?}"));
  let parser = parser::parse(&src);
  parser.exprs.iter().for_each(|e| println!("{e:?}"));
  parser.stmts.iter().for_each(|s| println!("{s:?}"));

  Ok(())
}