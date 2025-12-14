use std::{collections::HashMap, fmt, mem, hash::{self, Hash, Hasher}};
use crate::{IdSize, lexer::{Lexer, Span}, parser::{Expr, Spanned, Stmt, StmtTopLvl}};

pub struct Ast<'a> {
  pub lexer: Lexer<'a>,

  pub exprs: Vec<Spanned<Expr>>,
  pub stmts: Vec<Spanned<Stmt>>,
  pub toplvl: Vec<Spanned<StmtTopLvl>>,
  pub types: TypeEnv,
  pub idents: StringInterner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentId(pub IdSize);

#[derive(Default)]
pub struct StringInterner{
  map: HashMap<u64, IdentId>,
  vec: Vec<Span>,
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

    if let Some(id) = self.map.get(&hash) {
      return (*id).into();
    }

    let id = IdentId(self.map.len() as IdSize);
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

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Type {
  Untyped,
  Void,
  Bool,
  Int,
  Float,
  Array { inner: TypeId, len: u32 },
  Func { params: Vec<TypeId>, ret: TypeId },
  Struct { name: IdentId, fields: Vec<(IdentId, TypeId)> },
  UserDef,
  Generic(u8),
}

pub mod ty_id {
  use super::TypeId;

  pub const UNTYPED:  TypeId = TypeId(0);
  pub const VOID:     TypeId = TypeId(1);
  pub const BOOL:     TypeId = TypeId(2);
  pub const INT:      TypeId = TypeId(3);
  pub const FLOAT:    TypeId = TypeId(4);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub IdSize);

pub struct TypeEnv {
  userdefs: HashMap<IdentId, TypeId>,
  pub types: Vec<Type>,
}
impl Default for TypeEnv {
  fn default() -> Self {
    Self {
      userdefs: HashMap::new(),
      types: vec![
        Type::Untyped,
        Type::Void,
        Type::Bool,
        Type::Int,
        Type::Float,
      ],
    }
  }
}

impl TypeEnv {
  pub fn lookup_id(&self, id: IdentId) -> TypeId {
    self.userdefs[&id]
  }

  pub fn lookup(&self, id: IdentId) -> &Type {
    &self.types[self.lookup_id(id).0 as usize]
  }

  pub fn lookup_mut(&mut self, id: IdentId) -> &mut Type {
    &mut self.types[self.userdefs[&id].0 as usize]
  }

  pub fn get(&self, id: TypeId) -> &Type {
    &self.types[id.0 as usize]
  }

  pub fn get_mut(&mut self, id: TypeId) -> &mut Type {
    &mut self.types[id.0 as usize]
  }

  // true if was already present
  pub fn add_userdef(&mut self, name: IdentId, ty: Type) -> (TypeId, bool) {
    if let Some(id) = self.userdefs.get(&name) {
      let old = mem::replace(&mut self.types[id.0 as usize], ty);
      
      // if we had an userdef, we have just inserted the real type
      (*id, !matches!(old, Type::UserDef))
    } else {
      let id = self.add_ty(ty);
      self.userdefs.insert(name, id);
      (id, false)
    }
  }

  pub fn add_ty(&mut self, ty: Type) -> TypeId {
    self.types.push(ty);
    TypeId(self.types.len() as IdSize - 1)
  }

  pub fn ty_eq(&self, a_id: TypeId, b_id: TypeId) -> bool {
    use Type::*;
    
    match (self.get(a_id), self.get(b_id)) {
      (Untyped, Untyped) => false,
      (Void, Void) => true,
      (Bool, Bool) => true,
      (Int, Int) => true,
      (Float, Float) => true,
      (Array { inner: inner_a, len: len_a }, Array { inner: inner_b, len: len_b }) => {
        self.ty_eq(*inner_a, *inner_b) && (*len_a == 0 || len_a == len_b)
      }

      (Func { params: params_a, ret: ret_a }, Func { params: params_b, ret: ret_b }) => {
        params_a.len() == params_b.len() && params_a.iter().zip(params_b.iter()).all(|(a, b)| self.ty_eq(*a, *b)) && self.ty_eq(*ret_a, *ret_b)
      }

      (Struct { name: name_a, fields: fields_a }, Struct { name: name_b, fields: fields_b }) => {
        name_a == name_b && fields_a.len() == fields_b.len() && fields_a.iter().zip(fields_b.iter()).all(|(a, b)| a.0 == b.0 && self.ty_eq(a.1, b.1))
      }

      (UserDef, UserDef) => todo!(),
      _ => false,
    }
  }
}