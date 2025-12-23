use std::{collections::{HashMap, HashSet}, iter::zip};
use crate::{FrontendErrAlias, IdSize, ast::{Ast, IdentId}, lexer::Span, parser::{Expr, ExprId, ExprLiteral, Stmt, StmtId, StmtTopLvl, TyAnnot, TyAnnotId}};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
  Void,
  Bool,
  Int,
  Float,
  Array { inner: TypeId, len: u32 },
  
  Func { params: Vec<TypeId>, ret: TypeId },
  Struct { name: IdentId, fields: Vec<(IdentId, TypeId)> },
  
  Generic(IdentId),
}

impl Type {
  pub fn size(&self) -> usize { todo!() }
}

pub mod ty_id {
  use super::TypeId;

  pub const VOID:     TypeId = TypeId(0);
  pub const BOOL:     TypeId = TypeId(1);
  pub const INT:      TypeId = TypeId(2);
  pub const FLOAT:    TypeId = TypeId(3);
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

  pub fn lookup_ty_mut(&mut self, id: IdentId) -> &mut Type {
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
  // pub fn set(&mut self, dst: TypeId, src: TypeId) {
  //   let ty = self.get(src).clone();
  //   self.types_pool[dst.0 as usize] = ty;
  // }

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

      (Type::Func { params: pa, ret: ra }, Type::Func { params: pb, ret: rb }) => {
        ra == rb && pa.len() == pb.len() && zip(pa, pb).all(|(ta, tb)| self.ty_eq(*ta, *tb))
      }

      (Type::Struct { name: na, fields: fa }, Type::Struct { name: nb, fields: fb }) => todo!(),

      _ => false,
    }
  }

  pub fn generic_eq(&self, gen_id: TypeId, conc_id: TypeId, mapping: &mut HashMap<IdentId, TypeId>) -> Option<TypeId>  {
    let gen_ty = self.get(gen_id);

    let id = match gen_ty {
      Type::Generic(ident) => match mapping.get(ident) {
        // already mapped, check for equality
        Some(ty) => {
          if self.ty_eq(*ty, conc_id) {
            conc_id
          } else { return None }
        }

        // not mapped yet, insert
        None => {
          mapping.insert(*ident, conc_id);
          conc_id
        }
      }
      
      // not generic
      Type::Void => gen_id,
      Type::Bool => gen_id,
      Type::Int => gen_id,
      Type::Float => gen_id,

      Type::Array { inner, .. } => {
        self.generic_eq(*inner, conc_id, mapping)?
      },
      Type::Func { params, ret } => {
        for param in params {
          self.generic_eq(*param, conc_id, mapping)?;
        }

        self.generic_eq(*ret, conc_id, mapping)?;
        gen_id
      },

      Type::Struct { name, fields } => todo!(),
    };

    Some(id)
  }
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
}

#[derive(Default)]
struct Typechecker {
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

  pub fn annot_to_ty(&mut self, ast: &Ast, types: &mut TypeEnv, id: TyAnnotId) -> TypeId {
    let (annot, span) = &ast.annots[id.0 as usize];
    let id = match annot {
      // TODO: temporary hack
      TyAnnot::Untyped => ty_id::VOID,
      TyAnnot::Bool => ty_id::BOOL,
      TyAnnot::Int => ty_id::INT,
      TyAnnot::Float => ty_id::FLOAT,
      TyAnnot::Void => ty_id::VOID,
      
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

        let ty = Type::Func { params: params_ids, ret: ret_id };
        types.add_ty(ty)
      },
      
      TyAnnot::UserDef { name, generics } => todo!(),
    };

    id
  }

  fn check_expr(&mut self, ast: &Ast, types: &TypeEnv, id: ExprId) -> Result<TypeId, FrontendErrAlias> {
    let (expr, span) = &ast.exprs[id.0 as usize];

    let id = match expr {
      Expr::Literal(lit) => match lit {
        ExprLiteral::Bool(_) => ty_id::BOOL,
        ExprLiteral::Int(_) => ty_id::INT,
        ExprLiteral::Float(_) => ty_id::FLOAT,
        ExprLiteral::Array(expr_ids) => todo!(),
        ExprLiteral::Struct(ident_id, expr_ids) => todo!(),
      }

      Expr::Variable(ident) => self.binds.get(*ident)
        .ok_or_else(|| self.err(ast, "undeclared variable", *span))?,
      
      Expr::Unary { op, rhs } => todo!(),
      Expr::Binary { op, lhs, rhs } => todo!(),
      Expr::Call { callee, args } => {
        let callee_id = self.check_expr(ast, types, *callee)?;
        let callee_ty = types.get(callee_id);

        match callee_ty {
          Type::Func { params, ret } => {
            if params.len() != args.len() {
              return Err(self.err(ast, "wrong arguments count in function call", *span))
            }
            // TODO: has type to be instantiated as global?

            let mut generics_map = HashMap::new();
            for (param, arg) in zip(params, args) {
              let arg_ty = self.check_expr(ast, types, *arg)?;

              types.generic_eq(*param, arg_ty, &mut generics_map)
                .ok_or_else(|| self.err(ast, "impossible to instantiate generic function", *span))?;
            }

            // TODO: temporary hack
            // types.generic_eq(*ret, ty_id::VOID, &mut generics_map)
            //   .ok_or_else(|| self.err(ast, "impossible to instantiate generic function", *span))?;

            *ret
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
      Stmt::Decl(decl) => todo!(),
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

  fn check_block(&mut self, ast: &Ast, types: &mut TypeEnv, stmts: &[StmtId]) -> Result<(), FrontendErrAlias> {
    self.scope(|c| {
      for id in stmts {
        c.check_stmt(ast, types, *id);
      }
    });

    Ok(())
  }

  fn check_toplvl(&mut self, ast: &Ast, types: &mut TypeEnv) {
    // add all declared userdefs (structs)
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::FnDecl {..} => {},
        StmtTopLvl::StructDecl { name, .. } => {
          // TODO: not sure if parser should return hashmap

          // only add the name to the types, do not parse fields yet
          let ty = Type::Struct { name: *name, fields: Vec::new() };
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
        
        StmtTopLvl::StructDecl { name, fields } => {
          // check struct fields

          let fields_ids = fields.iter()
          .map(|(ident, field)| (*ident, self.annot_to_ty(ast, types, *field)))
          .collect();

          let ty = Type::Struct { name: *name, fields: fields_ids };
          types.add_userdef(*name, ty);
        },

        StmtTopLvl::FnDecl { name, params, ret, .. } => {
          let params_ids = params.iter()
            .map(|(_, annot)| self.annot_to_ty(ast, types, *annot))
            .collect();

          let ret_id = ret
            .map(|id| self.annot_to_ty(ast, types, id))
            .unwrap_or(ty_id::VOID);

          let ty = Type::Func { params: params_ids, ret: ret_id };
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
          let rhs_ty = self.check_expr(ast, types, decl.rhs);

          match rhs_ty {
            Ok(ty_id) => {
              
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
        StmtTopLvl::FnDecl { params, block, .. } => {
          let params = params.clone();

          self.scope(|c| {
            for param in &params {
              // TODO: this is done twice!
              let ty = c.annot_to_ty(ast, types, param.1);

              if !c.binds.add(param.0, ty) {
                c.err(ast, "parameter with same name already declared", *span);
                continue;
              }
            }

            _ = c.check_stmt(ast, types, *block);
          });
        }
      }
    }
  }
}

pub fn check(ast: &mut Ast) {
  let mut checker = Typechecker::default();
  let mut types = TypeEnv::default();
  checker.check_toplvl(&ast, &mut types);
}