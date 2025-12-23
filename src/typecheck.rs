use std::{collections::HashMap, iter::zip};
use crate::{FrontendErrAlias, IdSize, ast::{Ast, IdentId}, lexer::Span, parser::{ExprId, StmtTopLvl, TyAnnot, TyAnnotId}};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
  Untyped,
  Void,
  Bool,
  Int,
  Float,
  Array { inner: TypeId, len: u32 },
  
  Func { params: Vec<TypeId>, ret: TypeId },
  Struct { name: IdentId, fields: Vec<(IdentId, TypeId)> },

  Generic {func_name: IdentId, id: u16 }
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
  pub types_pool: Vec<Type>,
}
impl Default for TypeEnv {
  fn default() -> Self {
    Self {
      user_ident_to_id: HashMap::new(),
      types_pool: vec![
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

  pub fn lookup_ty(&self, id: IdentId) -> &Type {
    &self.types_pool[self.lookup_id(id).0 as usize]
  }

  pub fn lookup_mut(&mut self, id: IdentId) -> &mut Type {
    &mut self.types_pool[self.user_ident_to_id[&id].0 as usize]
  }

  pub fn get(&self, id: TypeId) -> &Type {
    &self.types_pool[id.0 as usize]
  }

  pub fn get_mut(&mut self, id: TypeId) -> &mut Type {
    &mut self.types_pool[id.0 as usize]
  }

  // TODO: this doesn't check for duplicates
  // TODO: we need an hashset + vec combo
  pub fn set(&mut self, dst: TypeId, src: TypeId) {
    let ty = self.get(src).clone();
    self.types_pool[dst.0 as usize] = ty;
  }

  // true if was already present
  // pub fn add_userdef(&mut self, name: IdentId, ty: Type) -> (TypeId, bool) {
  //   if let Some(id) = self.user_ident_to_id.get(&name) {
  //     let old = mem::replace(&mut self.types_pool[id.0 as usize], ty);
      
  //     // if we had an userdef, we have just inserted the real type
  //     (*id, !matches!(old, Type::UserDef(_)))
  //   } else {
  //     let id = self.add_ty(ty);
  //     self.user_ident_to_id.insert(name, id);
  //     (id, false)
  //   }
  // }

  pub fn add_userdef(&mut self, name: IdentId, ty: Type) -> Option<TypeId> {
    let id = self.add_ty(ty);
    match self.user_ident_to_id.insert(name, id) {
      Some(_) => None,
      None => Some(id)
    }
  }

  pub fn add_ty(&mut self, ty: Type) -> TypeId {
    self.types_pool.push(ty);
    TypeId(self.types_pool.len() as IdSize - 1)
  }

  pub fn ty_eq(&self, a_id: TypeId, b_id: TypeId) -> bool {
    use Type::*;
    
    match (self.get(a_id), self.get(b_id)) {
      (Untyped, Untyped) => false,
      (Void, Void) => true,
      (Bool, Bool) => true,
      (Int, Int) => true,
      (Float, Float) => true,
      // (Generic(_, a), Generic(_, b)) => a == b,

      (Array { inner: inner_a, len: len_a }, Array { inner: inner_b, len: len_b }) => {
        self.ty_eq(*inner_a, *inner_b) && (*len_a == 0 || len_a == len_b)
      }

      // (Func { params: params_a, ret: ret_a }, Func { params: params_b, ret: ret_b }) => {
      //   params_a.len() == params_b.len() && self.ty_eq(*ret_a, *ret_b) && zip(params_a, params_b).all(|(a, b)| self.ty_eq(*a, *b))
      // }

      // generic is always b
      // (Func { params: params_a, ret: ret_a }, FuncGeneric { params: params_b, ret: ret_b }) |
      // (FuncGeneric { params: params_b, ret: ret_b }, Func { params: params_a, ret: ret_a }) => {
      //   // TODO: this doesnt work
      //   if params_a.len() != params_b.len() { return false; }

      //   let mut generics_map = HashMap::new();

      //   for (a, b) in zip(params_a, params_b) {
      //     let param_ty = self.get(*b);

      //     if let Type::Generic(_, id) = param_ty {
      //       if let Some(gen_ty_id) = generics_map.get(id) {
      //         // already found generic and assigned it; check for equality
      //         if !self.ty_eq(*a, *gen_ty_id) {
      //           return false
      //         }
      //       } else {
      //         // we just found the generic, add to map
      //         generics_map.insert(*id, *a);
      //       }
      //     } else {
      //       // no generic, simply check equality
      //       if !self.ty_eq(*a, *b) {
      //         return false
      //       }
      //     }
      //   }

      //   // check ret type
      //   if let Type::Generic(_, id) = self.get(*ret_b) {
      //     if let Some(gen_ty_id) = generics_map.get(id) {
      //       // already found generic and assigned it; we have a type for return
      //       self.ty_eq(*ret_a, *gen_ty_id)
      //     } else {
      //       // TODO: no idea when this case should happen
      //       false
      //     }
      //   } else {
      //     // not a generic
      //     self.ty_eq(*ret_a, *ret_b)
      //   }
      // }

      (Struct { name: name_a, fields: fields_a }, Struct { name: name_b, fields: fields_b }) => {
        name_a == name_b && fields_a.len() == fields_b.len() && zip(fields_a, fields_b).all(|(a, b)| a.0 == b.0 && self.ty_eq(a.1, b.1))
      }

      // (UserDef(name_a), UserDef(name_b)) => name_a == name_b,
      
      _ => false,
    }
  }

  // pub fn resolve_type_generics(&self, virt_id: TypeId, real_id: TypeId, mapped: &mut HashMap<u16, TypeId>) -> TypeId {
  //   let virt_ty = self.get(virt_id);

  //   let res = match virt_ty {
  //     Type::Untyped => virt_id,
  //     Type::Void => virt_id,
  //     Type::Bool => virt_id,
  //     Type::Int => virt_id,
  //     Type::Float => virt_id,
  //     Type::Array { inner, .. } => {
  //       self.resolve_type_generics(*inner, real_id, mapped)
  //     },
  //     // we know this won't have any generic
  //     Type::Func { .. } => virt_id,
  //     Type::FuncGeneric { .. } => todo!("generic function generics"),
  //     Type::Struct { .. } => todo!("struct generics"),
  //     Type::UserDef(_) => unreachable!("user defs should not be present at this point"),
  //     Type::Generic { order, .. } => {
  //       if let Some(gen_ty_id) = mapped.get(order) {
  //         // // already found generic and assigned it; check for equality
  //         // if self.ty_eq(*gen_ty_id, real_id) {
  //         //   // types are equal, we're good
  //         //   *gen_ty_id
  //         // } else {
  //         //   // types are different, instantiation error
  //         //   return Err("call arguments of different type")
  //         // }
  //         *gen_ty_id
  //       } else {
  //         // we just found the generic, add to map
  //         mapped.insert(*order, real_id);
  //         real_id
  //       }
  //     },
  //   };

  //   // Ok(res)
  //   res
  // }
}

#[derive(Default)]
struct Bindings {
  tbl: HashMap<IdentId, TypeId>,
  // None if not already present, Some if shadowed
  stack: Vec<(IdentId, Option<TypeId>)>,
}
impl Bindings {
  pub fn get(&self, id: IdentId) -> Option<TypeId> {
    self.tbl.get(&id).copied()
  }

  // returns true if value was not present, false if present
  pub fn add(&mut self, id: IdentId, ty: TypeId) -> bool {
    // insert new value, swap with old
    let old = self.tbl.insert(id, ty);
    self.stack.push((id, old));
    old.is_none()
  }

  pub fn unwind(&mut self, amount: usize) {
    while self.stack.len() > amount {
      let (ident, ty) = self.stack.pop().unwrap();
      match ty {
        // restore shadowed
        Some(t) => self.tbl.insert(ident, t),
        None => self.tbl.remove(&ident),
      };
    }
  }

  pub fn scope<F: Fn(&mut Self)>(&mut self, f: F) {
    let sp = self.stack.len();
    f(self);
    self.unwind(sp);
  }
}

#[derive(Default)]
struct Typechecker {
  binds: Bindings,
  types: TypeEnv,
}

impl Typechecker {
  fn err<S: Into<String>>(&self, ast: &Ast, msg: S, span: Span) -> FrontendErrAlias {
    let err = FrontendErrAlias::new(&ast.lexer, msg, span);
    eprintln!("[TYPE ERR] {err}");
    err
  }

  pub fn annot_to_ty(&mut self, ast: &Ast, id: TyAnnotId) -> TypeId {
    let (annot, span) = &ast.annots[id.0 as usize];
    match annot {
      TyAnnot::Untyped => ty_id::UNTYPED,
      TyAnnot::Bool => ty_id::BOOL,
      TyAnnot::Int => ty_id::INT,
      TyAnnot::Float => ty_id::FLOAT,
      TyAnnot::Void => ty_id::VOID,
      
      TyAnnot::Array { inner, len } => {
        let inner_id = self.annot_to_ty(ast, *inner);
        // TODO: array len
        let ty = Type::Array { inner: inner_id, len: 0 };
        self.types.add_ty(ty)
      },

      TyAnnot::Func { params, ret } => {
        let params_ids = params.iter()
          .map(|param| self.annot_to_ty(ast, *param))
          .collect();

        let ret_id = ret
          .map(|id| self.annot_to_ty(ast, id))
          .unwrap_or(ty_id::VOID);

        let ty = Type::Func { params: params_ids, ret: ret_id };
        self.types.add_ty(ty)
      },
      
      // at this point it can only be a generic
      TyAnnot::UserDef { name, generics } => todo!("userdef annot to type"),
    }
  }

  fn check_expr(&mut self, ast: &Ast, id: ExprId) -> Result<TypeId, FrontendErrAlias> {
    todo!()
  }

  fn check_toplvl(&mut self, ast: &Ast) {
    // add all declared userdefs (structs)
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::FnDecl {..} => {},
        StmtTopLvl::StructDecl { name, fields } => {
          // TODO: not sure if parser should return hashmap

          let fields_ids = fields.iter()
            .map(|(ident, field)| (*ident, self.annot_to_ty(ast, *field)))
            .collect();
          let ty = Type::Struct { name: *name, fields: fields_ids };

          if self.types.add_userdef(*name, ty).is_some() {
            self.err(ast, "already declared struct", *span);
          }
        },
      }
    }

    // then add all declared functions
    for (stmt, _) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::StructDecl { .. } => {},
        StmtTopLvl::FnDecl { name, generics, params, ret, .. } => {
          // handle generics
          if !generics.is_empty() {
            for param in params {

            }
          }
          
          let params_ids = params.iter()
            .map(|(_, annot)| self.annot_to_ty(ast, *annot))
            .collect();

          let ret_id = ret
            .map(|id| self.annot_to_ty(ast, id))
            .unwrap_or(ty_id::VOID);

          let ty = Type::Func { params: params_ids, ret: ret_id };
          let ty_id = self.types.add_ty(ty);
          self.binds.add(*name, ty_id);
        },
      }
    }
    
    // then parse top level variables declarations
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::FnDecl { .. } | StmtTopLvl::StructDecl { .. } => {},
        StmtTopLvl::Decl(decl) => {
          let rhs_ty = self.check_expr(ast, decl.rhs);

          match rhs_ty {
            Ok(ty_id) => {
              todo!("check variable declaration")
            }
            Err(e) => continue,
          }
        },
      }
    }

    // finally we can parse function bodies
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::StructDecl { .. } => {},
        StmtTopLvl::FnDecl { name, generics, params, ret, block } => {
          todo!("check function body")
        }
      }
    }
  }
}

pub fn check(ast: &mut Ast) {
  let mut checker = Typechecker::default();
  checker.check_toplvl(&ast);
}