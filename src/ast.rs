use std::{collections::HashMap, fmt, hash::{self, Hash, Hasher}};
use crate::{IdSize, lexer::{Lexer, Span}, parser::{Expr, Spanned, Stmt, StmtTopLvl, TyAnnot}};

pub struct Ast<'a> {
  pub lexer: Lexer<'a>,

  pub exprs: Vec<Spanned<Expr>>,
  pub stmts: Vec<Spanned<Stmt>>,
  pub toplvl: Vec<Spanned<StmtTopLvl>>,
  pub annots: Vec<Spanned<TyAnnot>>,
  pub idents: StringInterner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentId(pub IdSize);

#[derive(Default)]
pub struct StringInterner{
  hash_to_index: HashMap<u64, IdentId>,
  indexes: Vec<Span>,
  pub buf: String,
}
impl fmt::Debug for StringInterner {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("StringInterner").field("buf", &self.buf).finish()
  }
}

impl StringInterner {
  pub fn intern(&mut self, name: &str) -> IdentId {
    let hash = {
      let mut hasher = hash::DefaultHasher::new();
      name.hash(&mut hasher);
      hasher.finish()
    };

    if let Some(id) = self.hash_to_index.get(&hash) {
      return (*id).into();
    }

    let id = IdentId(self.hash_to_index.len() as IdSize);
    self.hash_to_index.insert(hash, id);
    
    let intern_span = Span {
      start: self.buf.len() as u32,
      end: (self.buf.len() + name.len()) as u32
    };
    self.indexes.push(intern_span);
    self.buf.push_str(name);

    id
  }

  fn lookup(&self, id: IdentId) -> &str {
    let span = &self.indexes[id.0 as usize];
    &self.buf[span.start as usize..span.end as usize]
  }
}