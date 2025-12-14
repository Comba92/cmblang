use std::{collections::HashMap, fmt, hash::{self, Hash, Hasher}, iter::{self, zip}, mem};
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
  Untyped,
  Void,
  Bool,
  Int,
  Float,
  Array { inner: TypeId, len: u32 },
  
  Func { params: Vec<TypeId>, ret: TypeId },
  FuncGeneric { params: Vec<TypeId>, ret: TypeId },
  
  Struct { name: IdentId, fields: Vec<(IdentId, TypeId)> },
  
  UserDef(IdentId),
  // TODO: ident might be not neccesary here
  Generic(IdentId, u32),
}

impl Type {
  pub fn size(&self) -> usize { todo!() }
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
  user_ident_to_id: HashMap<IdentId, TypeId>,
  pub types_heap: Vec<Type>,
}
impl Default for TypeEnv {
  fn default() -> Self {
    Self {
      user_ident_to_id: HashMap::new(),
      types_heap: vec![
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
    self.user_ident_to_id[&id]
  }

  pub fn lookup(&self, id: IdentId) -> &Type {
    &self.types_heap[self.lookup_id(id).0 as usize]
  }

  pub fn lookup_mut(&mut self, id: IdentId) -> &mut Type {
    &mut self.types_heap[self.user_ident_to_id[&id].0 as usize]
  }

  pub fn get(&self, id: TypeId) -> &Type {
    &self.types_heap[id.0 as usize]
  }

  pub fn get_mut(&mut self, id: TypeId) -> &mut Type {
    &mut self.types_heap[id.0 as usize]
  }

  // TODO: this doesn't check for duplicates
  // TODO: we need an hashset + vec combo
  pub fn set(&mut self, dst: TypeId, src: TypeId) {
    let ty = self.get(src).clone();
    self.types_heap[dst.0 as usize] = ty;
  }

  // true if was already present
  pub fn add_userdef(&mut self, name: IdentId, ty: Type) -> (TypeId, bool) {
    if let Some(id) = self.user_ident_to_id.get(&name) {
      let old = mem::replace(&mut self.types_heap[id.0 as usize], ty);
      
      // if we had an userdef, we have just inserted the real type
      (*id, !matches!(old, Type::UserDef(_)))
    } else {
      let id = self.add_ty(ty);
      self.user_ident_to_id.insert(name, id);
      (id, false)
    }
  }

  pub fn add_ty(&mut self, ty: Type) -> TypeId {
    self.types_heap.push(ty);
    TypeId(self.types_heap.len() as IdSize - 1)
  }

  pub fn ty_eq(&self, a_id: TypeId, b_id: TypeId) -> bool {
    use Type::*;
    
    match (self.get(a_id), self.get(b_id)) {
      (Untyped, Untyped) => false,
      (Void, Void) => true,
      (Bool, Bool) => true,
      (Int, Int) => true,
      (Float, Float) => true,
      (Generic(_, a), Generic(_, b)) => a == b,

      (Array { inner: inner_a, len: len_a }, Array { inner: inner_b, len: len_b }) => {
        self.ty_eq(*inner_a, *inner_b) && (*len_a == 0 || len_a == len_b)
      }

      (Func { params: params_a, ret: ret_a }, Func { params: params_b, ret: ret_b }) => {
        params_a.len() == params_b.len() && self.ty_eq(*ret_a, *ret_b) && zip(params_a, params_b).all(|(a, b)| self.ty_eq(*a, *b))
      }

      // generic is always b
      (Func { params: params_a, ret: ret_a }, FuncGeneric { params: params_b, ret: ret_b }) |
      (FuncGeneric { params: params_b, ret: ret_b }, Func { params: params_a, ret: ret_a }) => {
        if params_a.len() != params_b.len() { return false; }

        let mut generics_map = HashMap::new();

        for (a, b) in zip(params_a, params_b) {
          let param_ty = self.get(*b);

          if let Type::Generic(_, id) = param_ty {
            if let Some(gen_ty_id) = generics_map.get(id) {
              // already found generic and assigned it; check for equality
              if !self.ty_eq(*a, *gen_ty_id) {
                return false
              }
            } else {
              // we just found the generic, add to map
              generics_map.insert(*id, *a);
            }
          } else {
            // no generic, simply check equality
            if !self.ty_eq(*a, *b) {
              return false
            }
          }
        }

        // check ret type
        if let Type::Generic(_, id) = self.get(*ret_b) {
          if let Some(gen_ty_id) = generics_map.get(id) {
            // already found generic and assigned it; we have a type for return
            self.ty_eq(*ret_a, *gen_ty_id)
          } else {
            // TODO: no idea when this case should happen
            false
          }
        } else {
          // not a generic
          self.ty_eq(*ret_a, *ret_b)
        }
      }

      (Struct { name: name_a, fields: fields_a }, Struct { name: name_b, fields: fields_b }) => {
        name_a == name_b && fields_a.len() == fields_b.len() && zip(fields_a, fields_b).all(|(a, b)| a.0 == b.0 && self.ty_eq(a.1, b.1))
      }

      (UserDef(name_a), UserDef(name_b)) => name_a == name_b,
      
      _ => false,
    }
  }
}