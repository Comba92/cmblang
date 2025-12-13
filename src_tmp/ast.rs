use std::{collections::HashMap, fmt::{self}, hash::{DefaultHasher, Hash, Hasher}};
use crate::{FrontendErrAlias, IdSize, lexer::*, parser::*};

fn hash_value<H: Hash>(value: H) -> u64 {
  let mut hasher = DefaultHasher::new();
  value.hash(&mut hasher);
  hasher.finish()
}

#[derive(Default)]
pub struct StringInterner<Id> {
  map: HashMap<u64, Id>,
  vec: Vec<Span>,
  buf: String,
}
impl<Id> fmt::Debug for StringInterner<Id> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("StringInterner").field("buf", &self.buf).finish()
  }
}

impl<Id: From<usize> + Copy> StringInterner<Id> {
  pub fn intern(&mut self, name: &str) -> Id {
    let hash = {
      let mut hasher = DefaultHasher::new();
      name.hash(&mut hasher);
      hasher.finish()
    };

    if let Some(id) = self.map.get(&hash) {
      return (*id).into();
    }

    let id = self.map.len();
    self.map.insert(hash, id.into());
    
    let intern_span = Span {
      start: self.buf.len() as u32,
      end: (self.buf.len() + name.len()) as u32
    };
    self.vec.push(intern_span);
    self.buf.push_str(name);

    id.into()
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
  pub buf: Vec<Type>
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

  pub fn lookup(&self, id: TypeId) -> &Type {
    &self.buf[self.vec[id.0 as usize] as usize]
  }
}

pub struct Ast<'a> {
  pub lexer: Lexer<'a>,
  pub exprs: Vec<Expr>,
  pub stmts: Vec<Stmt>,
  pub types: TypeInterner,
  pub idents: StringInterner<IdentId>,
}
impl<'a> Ast<'a> {
  pub fn new(lexer: Lexer<'a>) -> Self {
    Self {
      lexer,
      exprs: vec![],
      stmts: vec![],
      types: Default::default(),
      idents: Default::default(),
    }
  }

  pub fn top_lvl_id(&self) -> StmtId {
    (self.stmts.len() - 1).into()
  }

  pub fn top_lvl_block(&self) -> impl Iterator<Item = StmtId> {
    let Stmt::Block(block) = self.get_stmt(self.top_lvl_id()) else { unreachable!() };
    
    block.iter()
      .copied()
      .filter(|id| {
        let s = self.get_stmt(*id);
        matches!(s, Stmt::FnDecl { .. } | Stmt::StructDecl { .. } | Stmt::Decl { .. })
      })
  }

  pub fn get_tok(&self, id: TokenId) -> &Token {
    &self.lexer.tokens[id.0 as usize]
  }

  pub fn get_expr(&self, id: ExprId) -> &Expr {
    &self.exprs[id.0 as usize]
  }

  pub fn get_stmt(&self, id: StmtId) -> &Stmt {
    &self.stmts[id.0 as usize]
  }

  pub fn get_ty(&self, id: TypeId) -> &Type {
    self.types.lookup(id)
  }

  pub fn push_expr(&mut self, e: Expr) -> ExprId {
    self.exprs.push(e);
    ExprId(self.exprs.len() as u32 - 1)
  }

  pub fn push_stmt(&mut self, s: Stmt) -> StmtId {
    self.stmts.push(s);
    StmtId(self.stmts.len() as u32 - 1)
  }

  pub fn push_type(&mut self, ty: Type) -> TypeId {
    // self.types.get(&ty)
    //   .map(|x| *x)
    //   .unwrap_or_else(|| {
    //     self.types.insert(ty, self.types.len().into());
    //     self.types.len().into()
    //   })
    self.types.intern(ty)
  }

  pub fn err<S: Into<String>>(&self, msg: S, id: TokenId) -> FrontendErrAlias {
    let t = self.lexer.get_tok(id);
    let err = t.to_err(msg, &self.lexer);
    // self.errors.push(err.clone());
    err
  }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentId(pub u32);
impl From<usize> for IdentId {
  fn from(value: usize) -> Self { Self(value as IdSize) }
}

pub trait Visitor<T, E: std::error::Error> {
  fn visit_expr(&mut self, ast: &Ast, id: ExprId) -> Result<T, E>;
  fn visit_stmt(&mut self, ast: &Ast, id: StmtId) -> Result<(), E>;
  fn visit_block(&mut self, ast: &Ast, ids: &[StmtId]) -> Result<(), E>;
}
