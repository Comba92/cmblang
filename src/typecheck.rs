use std::{collections::HashMap, mem};
use crate::{FrontendErr, FrontendErrAlias, ast::{Ast, IdentId, Type, TypeEnv, TypeId, ty_id}, lexer::Span, parser::{self, Expr, ExprId, ExprLiteral, Stmt, StmtId, StmtTopLvl}};

type Scope = HashMap<IdentId, TypeId>;

struct Typechecker {
  tbl: Vec<Scope>,
  types: TypeEnv,
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
    self.top_scope().get(&ident).copied()
  }

  fn get_var_ty(&self, ast: &Ast, ident: IdentId) -> Option<&Type> {
    self.top_scope()
      .get(&ident)
      .map(|id| self.types.get(*id))
  }

  fn check_expr(&mut self, ast: &Ast, id: ExprId) -> Result<TypeId, FrontendErrAlias> {
    let (e, span) = &ast.exprs[id.0 as usize];
    let res = match e {
      Expr::Literal(lit) => match lit {
        ExprLiteral::Bool(_) => ty_id::BOOL,
        ExprLiteral::Int(_) => ty_id::INT,
        ExprLiteral::Float(_) => ty_id::FLOAT,
        ExprLiteral::Array(exprs) => {
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
      },
      Expr::Variable(ident) => {
        self.get_var(*ident)
          .ok_or_else(|| self.err(ast, "undeclared variable", *span))?
      },
      Expr::Unary { op, rhs } => todo!(),
      Expr::Binary { op, lhs, rhs } => todo!(),
      Expr::Call { callee, args } => todo!(),
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
          // TODO: we're cloning here, not really sure about that
          let (_, present) = self.types.add_userdef(*name, Type::Struct { name: *name, fields: fields.clone() });
        
          if present {
            self.err(ast, "already declared struct", *span);
          }
        }
      }
    }

    // now we can parse the top level declarations
    for (stmt, span) in &ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(decl) => {
          _ = self.check_decl(ast, decl, *span);
        }

        StmtTopLvl::FnDecl { name, params, ret, block } => { 
          // parse params, ret, and block
          let param_types = params.iter().map(|param| param.1).collect();
          let ty = Type::Func { params: param_types, ret: *ret };
          let ty_id = self.types.add_ty(ty);
          
          if self.add_var(*name, ty_id) {
            self.err(ast, "already declared func", *span);
            continue;
          }

          self.push_scope();
          for param in params {
            self.add_var(param.0, param.1);
          }
          
          _ = self.check_stmt(ast, *block);
          self.pop_scope();
        }

        StmtTopLvl::StructDecl { .. } => {}
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
}