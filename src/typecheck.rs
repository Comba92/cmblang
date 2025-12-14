use core::fmt;
use std::{collections::HashMap, iter::zip, mem};
use crate::{FrontendErr, FrontendErrAlias, ast::{Ast, IdentId, Type, TypeEnv, TypeId, ty_id}, lexer::Span, parser::{self, Expr, ExprId, ExprLiteral, Stmt, StmtId, StmtTopLvl}};

type Scope = HashMap<IdentId, TypeId>;

struct Typechecker {
  tbl: Vec<Scope>,
  types: TypeEnv,
}
impl fmt::Debug for Typechecker {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Typechecker").field("tbl", &self.tbl).finish()
  }
}

impl Typechecker {
  fn top_scope(&self) -> &Scope {
    let len = self.tbl.len()-1;
    &self.tbl[len]
  }

  fn top_scope_mut(&mut self) -> &mut Scope {
    let len = self.tbl.len()-1;
    &mut self.tbl[len]
  }

  fn push_scope(&mut self) {
    self.tbl.push(HashMap::new());
  }

  fn pop_scope(&mut self) {
    self.tbl.pop();
  }

  fn err<S: Into<String>>(&self, ast: &Ast, msg: S, span: Span) -> FrontendErrAlias {
    let err = FrontendErr::new(&ast.lexer, msg, span);
    eprintln!("[TYPE ERR] {err}");
    err
  }

  fn add_var(&mut self, name: IdentId, ty: TypeId) -> bool {
    self.top_scope_mut().insert(name, ty).is_some()
  }

  fn get_var(&self, ident: IdentId) -> Option<TypeId> {
    // we start from the deepest scope, and check upwards
    for scope in self.tbl.iter().rev() {
      let entry = scope.get(&ident).copied();
      if entry.is_some() { return entry }
    }

    None
  }

  fn get_var_ty(&self, ident: IdentId) -> Option<&Type> {
    self.get_var(ident)
      .map(|id| self.types.get(id))
  }

  fn check_expr(&mut self, ast: &Ast, id: ExprId) -> Result<TypeId, FrontendErrAlias> {
    let (e, span) = &ast.exprs[id.0 as usize];
    let res = match e {
      Expr::Literal(lit) => match lit {
        ExprLiteral::Bool(_) => ty_id::BOOL,
        ExprLiteral::Int(_) => ty_id::INT,
        ExprLiteral::Float(_) => ty_id::FLOAT,
        ExprLiteral::Array(exprs) => {
          // uninit array
          if exprs.len() == 0 { return Ok(ty_id::UNTYPED) }

          let prev = self.check_expr(ast, exprs[0])?;

          for i in 1..exprs.len() {
            let curr = self.check_expr(ast, exprs[i])?;
            
            if !self.types.ty_eq(prev, curr) {
              return Err(self.err(ast, "array values must be of the same type", *span));
            }
          }

          self.types.add_ty(Type::Array { inner: prev, len: exprs.len() as u32 })
        }

        ExprLiteral::Struct(name, exprs) => {
          // uninit struct
          if exprs.len() == 0 { return Ok(ty_id::UNTYPED) }

          if let Type::Struct { fields, .. } = self.types.lookup(*name) {
            if exprs.len() != fields.len() {
              return Err(self.err(ast, "wrong members count in struct literal", *span))
            }

            // TODO: we sadly have to clone it here to satisfty borrow checker
            let fields = fields.iter().map(|f| f.1).collect::<Vec<_>>();
            
            for (decl_ty, rhs) in zip(fields, exprs) {
              let rty = self.check_expr(ast, *rhs)?;
              if !self.types.ty_eq(decl_ty, rty) {
                return Err(self.err(ast, "assignin struct member of different type", *span))
              }
            }

            self.types.lookup_id(*name)
          } else {
            return Err(self.err(ast, "undefined struct name", *span))
          }
        }
      }

      Expr::Variable(ident) => {
        self.get_var(*ident)
          .ok_or_else(|| self.err(ast, "undeclared variable", *span))?
      },

      Expr::Unary { op, rhs } => todo!(),
      Expr::Binary { op, lhs, rhs } => todo!(),

      Expr::Call { callee, args } => {
        let callee_id = self.check_expr(ast, *callee)?;
        let callee_ty = self.types.get(callee_id);
        if let Type::FuncGeneric { params, ret } = callee_ty {
          if params.len() != args.len() {
            return Err(self.err(ast, "wrong arguments count in function call", *span))
          }

          // TODO: can we do something about cloning here?
          let params = params.clone();
          let ret = *ret;

          let mut generics_map = HashMap::new();

          for (param_ty_id, arg) in zip(params, args) {
            let arg_ty_id = self.check_expr(ast, *arg)?;

            let param_ty = self.types.get(param_ty_id);

            if let Type::Generic(_, id) = param_ty {
              if let Some(gen_ty_id) = generics_map.get(id) {
                // already found generic and assigned it; check for equality
                if !self.types.ty_eq(*gen_ty_id, arg_ty_id) {
                  return Err(self.err(ast, "call arguments of different type", *span))
                }
              } else {
                // we just found the generic, add to map
                generics_map.insert(*id, arg_ty_id);
              }
            } else {
              // no generic, simply check equality
              if !self.types.ty_eq(param_ty_id, arg_ty_id) {
                return Err(self.err(ast, "call arguments of different type", *span))
              }
            }
          }

          // check ret type
          if let Type::Generic(_, id) = self.types.get(ret) {
            if let Some(gen_ty_id) = generics_map.get(id) {
              // already found generic and assigned it; we have a type for return
              *gen_ty_id
            } else {
              return Err(self.err(ast, "return type couldn't be inferred from generic", *span))
            }
          } else {
            // not a generic
            ret
          }
        } else if let Type::Func { params, ret } = callee_ty {
          if params.len() != args.len() {
            return Err(self.err(ast, "wrong arguments count in function call", *span))
          }

          let params = params.clone();
          let ret = *ret;

          for (param, arg) in zip(params, args) {
            let aty = self.check_expr(ast, *arg)?;
            if !self.types.ty_eq(param, aty) {
              return Err(self.err(ast, "call arguments of different type", *span))
            }
          }

          ret
        } else {
          return Err(self.err(ast, "can't call on non function type", *span))
        }
      },
      
      Expr::Member { lhs, field } => todo!(),
      Expr::Index { lhs, idx } => todo!(),
    };

    Ok(res)
  }

  fn check_stmt(&mut self, ast: &Ast, id: StmtId) -> Result<(), FrontendErrAlias> {
    let (s, span) = &ast.stmts[id.0 as usize];
    match s {
      Stmt::Decl(decl) => self.check_decl(ast, decl, *span),
      Stmt::Assign { lhs, rhs } => {
        let lty = self.check_expr(ast, *lhs)?;
        let rty = self.check_expr(ast, *rhs)?;
        
        if !self.types.ty_eq(lty, rty)  {
          return Err(self.err(ast, "assigning value of different type", *span));
        }

        Ok(())
      },
      Stmt::Block(items) => self.check_block(ast, items),
      Stmt::IfElse { cond, iblock, eblock } => todo!(),
      Stmt::While { cond, wblock } => todo!(),
      Stmt::Return(_) => todo!(),
      Stmt::Expr(id) => self.check_expr(ast, *id).map(|_| {}),
    }
  }

  fn check_block(&mut self, ast: &Ast, stmts: &[StmtId]) -> Result<(), FrontendErrAlias> {
    for s in stmts {
      _ = self.check_stmt(ast, *s);
    }

    Ok(())
  }

  fn check_decl(&mut self, ast: &Ast, decl: &parser::Decl, span: Span) -> Result<(), FrontendErrAlias> {
    let rty_id = self.check_expr(ast, decl.rhs)?;

    let decl_ty = self.types.get(decl.annot);
    let rty = self.types.get(rty_id);

    let res = match (decl_ty, rty) {
      (Type::Untyped, Type::Untyped) => return Err(self.err(ast, "could not infer types as both are unknown", span)),
      // TODO: attention, handle generics to the right
      
      (Type::Untyped, Type::FuncGeneric { .. }) => return Err(self.err(ast, "generic function requires annotations", span)),
      (Type::Untyped, _) => rty_id,

      (_, Type::Untyped) => decl.annot,

      (_, _) => if self.types.ty_eq(decl.annot, rty_id) {
        // same type on both ends
        decl.annot
      } else {
        // different type
        return Err(self.err(ast, "different types provided in declaration", span))
      }
    };

    self.add_var(decl.ident, res);
    Ok(())
  }

  fn check_toplvl(&mut self, ast: &mut Ast) {
    // first, add all declared userdefs (structs)
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::FnDecl { .. } => {}

        StmtTopLvl::StructDecl { name, fields } => {
          // parse fields
          // TODO: we sadly have to clone it here to satisfty borrow checker

          // TODO: we should check for self referencial structs
          let (_, present) = self.types.add_userdef(*name, Type::Struct { name: *name, fields: fields.clone() });
        
          if present {
            self.err(ast, "already declared struct", *span);
          }
        }
      }
    }

    // then, add all declared functions (as they might depend on userdefs)
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::StructDecl { .. } => {}

        StmtTopLvl::FnDecl { name, generics, params, ret, .. } => {
          // TODO: this doesnt work, will edit generics of other functions
          
          let param_types = if generics.is_empty() {
            params.iter().map(|p| p.1).collect()
          } else {
            // we have generics
            params.iter()
            .map(|(_, ty_id)| {
              let ty = self.types.get_mut(*ty_id);

              if let Type::UserDef(ty_name) = ty {
                // lookup generics array
                if let Some(idx) = generics.iter().position(|g| g == ty_name) {
                  *ty = Type::Generic(*name, idx as u32)
                }
              }

              *ty_id
            })
            .collect()
          };

          let ty = if generics.is_empty() {
            Type::Func { params: param_types, ret: *ret }
          } else {
            let ret_ty = self.types.get_mut(*ret);

            if let Type::UserDef(ty_name) = ret_ty {
              // lookup generics array
              if let Some(idx) = generics.iter().position(|g| g == ty_name) {
                // change userdef to generic
                *ret_ty = Type::Generic(*name, idx as u32)
              }
            }

            Type::FuncGeneric { params: param_types, ret: *ret }
          };

          let ty_id = self.types.add_ty(ty);

          if self.add_var(*name, ty_id) {
            self.err(ast, "already declared func", *span);
            continue;
          }
        }
      }
    }

    // then, parse top level variables declarations
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::FnDecl { .. } | StmtTopLvl::StructDecl { .. } => {}
        
        StmtTopLvl::Decl(decl) => {
          _ = self.check_decl(ast, decl, *span);
        }
      }
    }

    // finally, parse function bodies
    for (stmt, _) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(_) | StmtTopLvl::StructDecl { .. } => {}

        StmtTopLvl::FnDecl { params, block, .. } => { 
          self.push_scope();
          for param in params {
            self.add_var(param.0, param.1);
          }
          _ = self.check_stmt(ast, *block);
          self.pop_scope();
        }
      }
    }
  }
}

pub fn check(ast: &mut Ast) {
  let mut typechecker = Typechecker {
    tbl: vec![],
    types: mem::take(&mut ast.types)
  };
  typechecker.push_scope();

  typechecker.check_toplvl(ast);

  // TODO: this gives types back to ast, not sure (we probably will need ast types later)
  ast.types = mem::take(&mut typechecker.types);
}