use std::{error, fmt};

mod lexer;
mod parser;
mod ast;
mod typecheck;

pub type FrontendErrAlias = FrontendErr;
pub type IdSize = u32;

#[derive(Debug, Clone)]
pub struct FrontendErr {
  msg: String,
  span: lexer::Span,
  str: String,
  column: usize,
  line: usize,
}
impl FrontendErr {
  pub fn new<S: Into<String>>(lexer: &lexer::Lexer, msg: S, span: lexer::Span) -> Self {
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
      msg: msg.into(),
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

trait CursorIter<Inner> {
  fn peek(&self) -> Inner { self.peek_nth(0) }
  fn peek_nth(&self, nth: usize) -> Inner;
  fn advance(&mut self) { self.advance_nth(1); }
  fn advance_nth(&mut self, n: usize) { *self.curr_mut() += n as u32; }
  fn undo(&mut self) { *self.curr_mut() -= 1; }

  fn eat(&mut self) -> Inner {
    let res = self.peek();
    self.advance();
    res
  }

  fn start(&self) -> &[Inner];
  fn slice(&self) -> &[Inner] { &self.start()[self.curr() as usize..] }
  fn curr(&self) -> u32;
  fn curr_mut(&mut self) -> &mut u32;
  fn has_some(&self) -> bool { self.curr() < self.start().len() as u32 }
}

fn dbg_list<T: fmt::Debug>(list: &[T]) {
  for item in list.iter().enumerate() {
    println!("{} -> {:?}", item.0, item.1);
  }
  println!()
}

fn main() {
  println!("Hello World!");

  let src = include_str!("../test2.cmb");

  let mut ast = parser::parse(src).unwrap();
  let checker = typecheck::check(&mut ast);
  
  println!();

  // ast.dbg_exprs();
  // ast.dbg_stmts();
  // ast.dbg_toplvls();
  
  ast.dbg_idents();
  ast.dbg_annots();
}