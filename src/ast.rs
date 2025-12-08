use std::{collections::HashMap, fmt, hash::{DefaultHasher, Hash, Hasher}};
use crate::{IdSize, lexer::*, parser::*};

fn hash_value<H: Hash>(value: H) -> u64 {
  let mut hasher = DefaultHasher::new();
  value.hash(&mut hasher);
  hasher.finish()
}

#[derive(Default)]
pub struct StringInterner {
  map: HashMap<u64, IdentId>,
  vec: Vec<Span>,
  buf: String,
}
impl fmt::Debug for StringInterner {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("StringInterner").field("buf", &self.buf).finish()
  }
}

impl StringInterner {
  pub fn intern(&mut self, name: &str) -> IdentId {
    let hash = {
      let mut hasher = DefaultHasher::new();
      name.hash(&mut hasher);
      hasher.finish()
    };

    if let Some(id) = self.map.get(&hash) {
      return *id;
    }

    let id = IdentId(self.map.len() as u32);
    self.map.insert(hash, id);
    
    let intern_span = Span {
      start: self.buf.len() as u32,
      end: (self.buf.len() + name.len()) as u32
    };
    self.vec.push(intern_span);
    self.buf.push_str(name);

    id
  }

  fn lookup(&self, id: IdentId) -> &str {
    let span = &self.vec[id.0 as usize];
    &self.buf[span.start as usize..span.end as usize]
  }
}

#[derive(Default)]
pub struct TypeInterner {
  map: HashMap<u64, TypeId>,
  vec: Vec<u32>,
  buf: Vec<Type>
}
impl fmt::Debug for TypeInterner {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("TypeInterner").field("buf", &self.buf).finish()
  }
}

impl TypeInterner {
  pub fn intern(&mut self, ty: Type) -> TypeId {
    let hash = {
      let mut hasher = DefaultHasher::new();
      ty.hash(&mut hasher);
      hasher.finish()
    };

    if let Some(id) = self.map.get(&hash) {
      return *id;
    }

    let id = TypeId(self.map.len() as u32);
    self.map.insert(hash, id);
    
    self.vec.push(self.buf.len() as u32);
    self.buf.push(ty);

    id
  }

  pub fn lookup(&mut self, id: TypeId) -> &Type {
    &self.buf[self.vec[id.0 as usize] as usize]
  }
}

pub struct Ast<'a> {
  pub lexer: Lexer<'a>,
  pub exprs: Vec<Expr>,
  pub stmts: Vec<Stmt>,
  // pub types: HashMap<Type, TypeId>,
  pub types: TypeInterner,
  pub idents: StringInterner,
}
impl<'a> Ast<'a> {
  pub fn top_lvl_id(&self) -> StmtId {
    (self.stmts.len() - 1).into()
  }

  pub fn get_tok(&self, id: TokenId) -> Token {
    self.lexer.tokens[id.0 as usize].clone()
  }

  // pub fn get_tok_from_expr(&self, id: ExprId) -> Token {
  //   let id = self.exprs[id.0 as usize].token();
  //   self.lexer.tokens[id.0 as usize].clone()
  // }

  pub fn get_ty(&self, id: TypeId) -> Type {
    todo!()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentId(pub u32);
impl From<usize> for IdentId {
  fn from(value: usize) -> Self { Self(value as IdSize) }
}

pub trait Visitor<E> {
  fn visit_expr(&mut self, id: ExprId) -> Result<Type, E>;
  fn visit_stmt(&mut self, id: StmtId) -> Result<(), E>;


  fn visit_block(&mut self, ids: &[StmtId]) -> Result<(), E> {
    for id in ids { self.visit_stmt(*id); }
    Ok(())
  }
}
