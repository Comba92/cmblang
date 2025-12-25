use std::{collections::HashMap, iter::zip};
use crate::{FrontendErrAlias, IdSize, ast::{Ast, IdentId}, lexer::Span, parser::{self, Expr, ExprId, ExprLiteral, Stmt, StmtId, StmtTopLvl, TyAnnot, TyAnnotId}};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
  // Untyped,
  Void,
  Bool,
  Int,
  Float,
  Array { inner: TypeId, len: u32 },
  
  Func { params: Vec<TypeId>, ret: TypeId, is_generic: bool },
  Struct { name: IdentId, fields: Vec<(IdentId, TypeId)>, is_generic: bool },
  
  Generic(IdentId),
}

impl Type {
  pub fn size(&self) -> usize { todo!() }
}

pub mod ty_id {
  use super::TypeId;

  // pub const UNTYPED:  TypeId = TypeId(0);
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
        // Type::Untyped,
        Type::Void,

        //////////
        Type::Void,
        Type::Bool,
        Type::Int,
        Type::Float,
      ],
    }
  }
}

impl TypeEnv {
  pub fn lookup_id(&self, id: IdentId) -> Option<TypeId> {
    self.user_ident_to_id.get(&id).copied()
  }

  pub fn lookup_ty(&self, id: IdentId) -> Option<&Type> {
    self.user_ident_to_id.get(&id)
      .map(|id| &self.types_pool[id.0 as usize])
  }

  pub fn lookup_ty_mut(&mut self, id: IdentId) -> Option<&mut Type> {
    self.user_ident_to_id.get(&id)
      .map(|id| &mut self.types_pool[id.0 as usize])
  }

  pub fn get(&self, id: TypeId) -> &Type {
    &self.types_pool[id.0 as usize]
  }

  pub fn get_mut(&mut self, id: TypeId) -> &mut Type {
    &mut self.types_pool[id.0 as usize]
  }

  // returns None if was already present
  pub fn add_userdef(&mut self, name: IdentId, ty: Type) -> Option<TypeId> {
    let id = self.add_ty(ty);
    match self.user_ident_to_id.insert(name, id) {
      Some(old) => None,
      None => Some(id)
    }
  }

  pub fn add_ty(&mut self, ty: Type) -> TypeId {
    self.types_pool.push(ty);
    TypeId(self.types_pool.len() as IdSize - 1)
  }

  pub fn ty_eq(&self, a: TypeId, b: TypeId) -> bool {
    match (self.get(a), self.get(b)) {
      (Type::Void, Type::Void) => true,
      (Type::Bool, Type::Bool) => true,
      (Type::Int, Type::Int) => true,
      (Type::Float, Type::Float) => true,
      (Type::Generic(ident_a), Type::Generic(ident_b)) => ident_a == ident_b,

      (Type::Array { inner: ta, len: len_a }, Type::Array { inner: tb, len: len_b }) => {
        len_a == len_b && self.ty_eq(*ta, *tb)
      },

      (Type::Func { params: pa, ret: ra, is_generic: a_is_gen }, Type::Func { params: pb, ret: rb, is_generic: b_is_gen }) => {
        ra == rb && pa.len() == pb.len() && zip(pa, pb).all(|(ta, tb)| self.ty_eq(*ta, *tb))
      }

      (Type::Struct { name: na, fields: fa, is_generic: a_is_gen }, Type::Struct { name: nb, fields: fb, is_generic: b_is_gen }) => {
        na == nb && fa.len() == fb.len() && zip(fa, fb).all(|(ta, tb)| self.ty_eq(ta.1, tb.1))
      },

      _ => false,
    }
  }

  pub fn generic_eq(&self, gen_id: TypeId, conc_id: TypeId, mapping: &mut HashMap<IdentId, TypeId>) -> Option<TypeId> {
    let id = match (self.get(gen_id), self.get(conc_id)) {
      (Type::Generic(ident), _) => match mapping.get(ident) {
        // already mapped, check for equality
        Some(a_ty) => {
          if self.ty_eq(*a_ty, conc_id) {
            conc_id
          } else { return None }
        }

        // not mapped yet, insert
        None => {
          mapping.insert(*ident, conc_id);
          conc_id
        }
      }

      // needed for function return types
      (Type::Void, Type::Void) => gen_id,
      (Type::Bool, Type::Bool) => gen_id,
      (Type::Int, Type::Int) => gen_id,
      (Type::Float, Type::Float) => gen_id,

      (Type::Array { inner: ta, len: len_a }, Type::Array { inner: tb, len: len_b }) => {
        self.generic_eq(*ta, *tb, mapping)?;
        gen_id
      },

      (Type::Func { params: pa, ret: ra, is_generic: a_is_gen }, Type::Func { params: pb, ret: rb, is_generic: b_is_gen }) => {
        if pa.len() != pb.len() { return None }

        for param in zip(pa, pb) {
          self.generic_eq(*param.0, *param.1, mapping)?;
        }

        self.generic_eq(*ra, *rb, mapping)?;
        gen_id
      }

      (Type::Struct { name: na, fields: fa, is_generic: a_is_gen }, Type::Struct { name: nb, fields: fb, is_generic: b_is_gen }) => {
        if na != nb { return None }
        if fa.len() != fb.len() { return None }

        for field in zip(fa, fb).map(|(ta, tb)| (ta.1, tb.1)) {
          self.generic_eq(field.0, field.1, mapping)?;
        }

        gen_id
      },

      _ => return None,
    };

    Some(id)
  }

  fn instantiate_generic(&mut self, id: TypeId, mapping: &HashMap<IdentId, TypeId>) -> Option<TypeId> {
    let ty = self.get(id);
    let new = match ty {
      Type::Generic(ident) => {
        let conc_id = mapping.get(ident)?;
        return Some(*conc_id)
      },

      Type::Void | Type::Bool | Type::Int | Type::Float => ty.clone(),
      Type::Array { inner, len } => {
        let inner = *inner;
        let len = *len;
        Type::Array { inner: self.instantiate_generic(inner, mapping)?, len }
      }
      Type::Func { params, ret, is_generic } => {
        if !is_generic { return Some(id) }

        let mut ret = *ret;
        let mut params = params.clone();
        for param in &mut params {
          *param = self.instantiate_generic(*param, mapping)?;
        }
        ret = self.instantiate_generic(ret, mapping)?;

        Type::Func { params, ret, is_generic: false }
      }

      Type::Struct { name, fields, is_generic } => {
        if !is_generic { return Some(id) }

        let name = *name;
        let mut fields = fields.clone();

        for field in fields.iter_mut().map(|f| &mut f.1) {
          *field = self.instantiate_generic(*field, mapping)?;
        }

        Type::Struct { name, fields, is_generic: false }
      }
    };

    // TODO: this will duplicate types
    Some(self.add_ty(new))
  }
}

#[derive(Default, Debug)]
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
}

#[derive(Default)]
pub struct Typechecker {
  binds: Bindings,
}

impl Typechecker {
  fn err<S: Into<String>>(&self, ast: &Ast, msg: S, span: Span) -> FrontendErrAlias {
    let err = FrontendErrAlias::new(&ast.lexer, msg, span);
    eprintln!("[TYPE ERR] {err}");
    err
  }

  pub fn scope<F: FnMut(&mut Self)>(&mut self, mut f: F) {
    let sp = self.binds.stack.len();
    f(self);
    self.binds.unwind(sp);
  }

  // TODO: this duplicates types
  // TODO: this can fail
  pub fn annot_to_ty(&mut self, ast: &Ast, types: &mut TypeEnv, id: TyAnnotId) -> TypeId {
    let (annot, _) = &ast.annots[id.0 as usize];
    let id = match annot {
      TyAnnot::Bool => ty_id::BOOL,
      TyAnnot::Int => ty_id::INT,
      TyAnnot::Float => ty_id::FLOAT,
      
      TyAnnot::Generic(ident) => types.add_ty(Type::Generic(*ident)),

      TyAnnot::Array { inner, .. } => {
        let inner_id = self.annot_to_ty(ast, types, *inner);
        // TODO: array len
        let ty = Type::Array { inner: inner_id, len: 0 };
        types.add_ty(ty)
      },

      TyAnnot::Func { params, ret } => {
        let params_ids = params.iter()
          .map(|param| self.annot_to_ty(ast, types, *param))
          .collect();

        let ret_id = ret
          .map(|id| self.annot_to_ty(ast, types, id))
          .unwrap_or(ty_id::VOID);

        let ty = Type::Func { params: params_ids, ret: ret_id, is_generic: false };
        types.add_ty(ty)
      },
      
      TyAnnot::UserDef { name, generics } => {
        match types.lookup_id(*name) {
          Some(ty) => ty,
          None => todo!("undeclared types not handled yet")
        }
      },
    };

    id
  }

  fn check_expr(&mut self, ast: &Ast, types: &mut TypeEnv, id: ExprId) -> Result<TypeId, FrontendErrAlias> {
    let (expr, span) = &ast.exprs[id.0 as usize];

    let id = match expr {
      Expr::Literal(lit) => match lit {
        ExprLiteral::Bool(_) => ty_id::BOOL,
        ExprLiteral::Int(_) => ty_id::INT,
        ExprLiteral::Float(_) => ty_id::FLOAT,
        ExprLiteral::Array(expr_ids) => todo!(),
        ExprLiteral::Struct(ident, members) => {
          let struct_ty = types.lookup_ty(*ident)
            .ok_or_else(|| self.err(ast, "undeclared struct name", *span))?;
          match struct_ty {
            Type::Struct { name, fields, is_generic } => {
              if fields.len() != members.len() {
                return Err(self.err(ast, "wrong fields count in struct literal", *span))
              }

              // TODO: can we do something about the cloning here?
              let name = *name;
              let fields = fields.clone();
              let is_generic = *is_generic;

              if is_generic {
                let mut generics_map= HashMap::new();

                for ((_, field_ty), member) in zip(fields, members) {
                  let member_ty = self.check_expr(ast, types, *member)?;

                  types.generic_eq(field_ty, member_ty, &mut generics_map)
                    .ok_or_else(|| self.err(ast, "impossible to instantiate generic struct", *span))?;
                }

                // we can do unwrap here as we know the struct is present
                let struct_id = types.lookup_id(name).unwrap();
                let instance_id = types.instantiate_generic(struct_id, &generics_map)
                  .ok_or_else(|| self.err(ast, "impossible to instantiate generic struct", *span))?;

                instance_id
              } else {
                for ((_, field_ty), member) in zip(fields, members){
                  let member_ty = self.check_expr(ast, types, *member)?;
                  if !types.ty_eq(field_ty, member_ty) {
                    return Err(self.err(ast, "invalid members in struct literal", *span))
                  }
                }

                types.lookup_id(name).unwrap()
              }
            }

            _ => return Err(self.err(ast, "wrong name type for struct literal", *span))
          }
        },
      }

      Expr::Variable(ident) => self.binds.get(*ident)
        .ok_or_else(|| self.err(ast, "undeclared variable", *span))?,
      
      Expr::Unary { op, rhs } => todo!(),
      Expr::Binary { op, lhs, rhs } => todo!(),

      Expr::Call { callee, args } => {
        let callee_id = self.check_expr(ast, types, *callee)?;
        let callee_ty = types.get(callee_id);

        match callee_ty {
          Type::Func { params, ret, is_generic, } => {
            if params.len() != args.len() {
              return Err(self.err(ast, "wrong arguments count in function call", *span))
            }

            // TODO: can we do something about the cloning here?
            let params = params.clone();
            let ret = *ret;
            let is_generic = *is_generic;

            if is_generic {
              let mut generics_map = HashMap::new();
              for (param, arg) in zip(params, args) {
                let arg_ty = self.check_expr(ast, types, *arg)?;
  
                types.generic_eq(param, arg_ty, &mut generics_map)
                  .ok_or_else(|| self.err(ast, "impossible to instantiate generic function", *span))?;
              }
  
              let instance_id = types.instantiate_generic(callee_id, &generics_map)
              .ok_or_else(|| self.err(ast, "impossible to infer function return type", *span))?;
            
              // TODO: little hack for now...
              let Type::Func { ret, .. } = types.get(instance_id) else { unreachable!() };
              *ret
            } else {
              for (param, arg) in zip(params, args) {
                let arg_ty = self.check_expr(ast, types, *arg)?;
                if !types.ty_eq(param, arg_ty) {
                  return Err(self.err(ast, "invalid arguments in function call", *span))
                }
              }

              ret
            }
          }

          _ => return Err(self.err(ast, "can't do function call on non function type", *span))
        }
      }
      Expr::Member { lhs, field } => todo!(),
      Expr::Index { lhs, idx } => todo!(),
    };

    Ok(id)
  }

  fn check_stmt(&mut self, ast: &Ast, types: &mut TypeEnv, id: StmtId) -> Result<(), FrontendErrAlias> {
    let (stmt, span) = &ast.stmts[id.0 as usize];

    match stmt {
      Stmt::Decl(decl) => self.check_decl(ast, types, decl, *span),
      Stmt::Assign { lhs, rhs } => {
        let lty = self.check_expr(ast, types, *lhs)?;
        let rty = self.check_expr(ast, types, *rhs)?;

        if !types.ty_eq(lty, rty)  {
          return Err(self.err(ast, "assigning value of different type", *span));
        }

        Ok(())
      },
      
      Stmt::Block(stmt_ids) => self.check_block(ast, types, stmt_ids),
      Stmt::IfElse { cond, iblock, eblock } => todo!(),
      Stmt::While { cond, wblock } => todo!(),
      Stmt::Return(expr_id) => todo!(),
      Stmt::Expr(id) => self.check_expr(ast, types, *id).map(|_| ()),
    }
  }

  /*
    primitive = primitive OK

    untyped = primitive OK
    untyped = concrete_func OK
    untyped = generic_func NO
    untyped = generic_struct NO

    concrete_func = concrete_func OK
    concrete_func = generic_func MUST BE CHECKED

    array; len = array; len OK
    array; * = array; len OK

    concrete_struct = concrete_struct OK
    concrete_struct = generic_struct MUST BE CHECKED
  */

  fn check_decl(&mut self, ast: &Ast, types: &mut TypeEnv, decl: &parser::Decl, span: Span) -> Result<(), FrontendErrAlias> {
    let rhs_id = self.check_expr(ast, types, decl.rhs)?;
    
    if let Some(annot) = decl.annot {
      // if we have the annotation, we do a ty equality
      let decl_id = self.annot_to_ty(ast, types, annot);
      if !types.ty_eq(decl_id, rhs_id) {
        return Err(self.err(ast, "different types provided in declaration", span));
      } else {
        self.binds.add(decl.ident, rhs_id);
      }
    } else {
      // untyped: take rhs type
      let rhs_ty = types.get(rhs_id);

      match rhs_ty {
        // generics cannot be assigned
        Type::Func { is_generic, .. } if *is_generic => return Err(self.err(ast, "can't assign generic function", span)),
        Type::Struct { is_generic, .. } if *is_generic => return Err(self.err(ast, "can't assign generic struct", span)),
        _ => { self.binds.add(decl.ident, rhs_id); }
      }
    }

    Ok(())
  }

  fn check_block(&mut self, ast: &Ast, types: &mut TypeEnv, stmts: &[StmtId]) -> Result<(), FrontendErrAlias> {
    self.scope(|c| {
      for id in stmts {
        _ = c.check_stmt(ast, types, *id);
      }
    });

    Ok(())
  }

  fn check_toplvl(&mut self, ast: &Ast, types: &mut TypeEnv) {
    // add all declared userdefs (structs)
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::FnDecl {..} => {},
        StmtTopLvl::StructDecl { name, is_generic, .. } => {
          // only add the name to the types, do not parse fields yet
          let ty = Type::Struct { name: *name, fields: Vec::new(), is_generic: *is_generic };
          match types.add_userdef(*name, ty) {
            Some(_) => {}
            None => { self.err(ast, "already declared struct", *span); }
          }
        },
      }
    }
    
    // then add all declared functions signatures and check struct fields
    for (stmt, _) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) => {}
        
        StmtTopLvl::StructDecl { name, fields, is_generic } => {
          // check struct fields

          let fields_ids = fields.iter()
          .map(|(ident, field)| (*ident, self.annot_to_ty(ast, types, *field)))
          .collect();

          // TODO: this will be duplicated
          let ty = Type::Struct { name: *name, fields: fields_ids, is_generic: *is_generic };
          types.add_userdef(*name, ty);
        },

        StmtTopLvl::FnDecl { name, params, ret, is_generic, .. } => {
          let params_ids = params.iter()
            .map(|(_, annot)| self.annot_to_ty(ast, types, *annot))
            .collect();

          let ret_id = ret
            .map(|id| self.annot_to_ty(ast, types, id))
            .unwrap_or(ty_id::VOID);

          let ty = Type::Func { params: params_ids, ret: ret_id, is_generic: *is_generic };
          let ty_id = types.add_ty(ty);
          self.binds.add(*name, ty_id);
        },
      }
    }
    
    // then parse top level variables declarations
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::FnDecl { .. } | StmtTopLvl::StructDecl { .. } => {},
        StmtTopLvl::Decl(decl) => {
          _ = self.check_decl(ast, types, decl, *span);
        }
      }
    }

    // finally we can parse function bodies
    for (stmt, _) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::StructDecl { .. } => {},
        StmtTopLvl::FnDecl { params, block, .. } => {
          let params = params.clone();

          self.scope(|c| {
            for param in &params {
              // TODO: this is done twice!
              let ty = c.annot_to_ty(ast, types, param.1);

              c.binds.add(param.0, ty);
            }

            _ = c.check_stmt(ast, types, *block);
          });
        }
      }
    }
  }
}

pub fn check(ast: &mut Ast) -> Typechecker {
  let mut checker = Typechecker::default();
  let mut types = TypeEnv::default();
  checker.check_toplvl(&ast, &mut types);

  for (i, ty) in types.types_pool.iter().enumerate() {
    println!("{i} -> {:?}", ty)
  }
  checker
}