use std::collections::HashMap;
use crate::{lexer::*, parser::*};

pub struct Ast<'a> {
  pub lexer: Lexer<'a>,
  pub exprs: Vec<Expr>,
  pub stmts: Vec<Stmt>,
  pub types: HashMap<Type, TypeId>
}
impl<'a> Ast<'a> {
  pub fn top_lvl_id(&self) -> StmtId {
    (self.stmts.len() - 1).into()
  }

  pub fn get_tok(&self, id: TokenId) -> Token {
    self.lexer.tokens[id.0 as usize].clone()
  }

  pub fn get_tok_from_expr(&self, id: ExprId) -> Token {
    let id = self.exprs[id.0 as usize].token();
    self.lexer.tokens[id.0 as usize].clone()
  }

  pub fn get_ty(&self, id: TypeId) -> Type {
    todo!()
  }
}

pub trait Visitor<E> {
  fn visit_expr(&mut self, id: ExprId) -> Result<Type, E>;
  fn visit_stmt(&mut self, id: StmtId) -> Result<(), E>;


  fn visit_block(&mut self, ids: &[StmtId]) -> Result<(), E> {
    for id in ids { self.visit_stmt(*id); }
    Ok(())
  }
}
