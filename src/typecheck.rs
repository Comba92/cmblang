use std::collections::HashMap;
use crate::{FrontendErrAlias, ast::{self, Ast, Visitor}, lexer::{KeywordKind, Token, TokenKind}, parser::*};

struct Sym {
  ty: Type,
}

struct TypeChecker<'a> {
  symtbl: HashMap<String, Sym>,
  ast: &'a Ast<'a>,
  errors: Vec<FrontendErrAlias>,
}
impl<'a> TypeChecker<'a> {
  fn add_sym<S: Into<String>>(&mut self, name: S, sym: Sym) {
    self.symtbl.insert(name.into(), sym);
  }

  fn get_sym_ty(&self, name: &str) -> Type {
    self.symtbl.get(name)
      .map(|s| s.ty.clone())
      .unwrap_or_default()
  }

  fn ty_eq(&self, a: &Type, b: &Type) -> bool {
    todo!()
  }

  fn err<S: Into<String>>(&self, msg: S, t: &Token) -> FrontendErrAlias {
    let err = t.to_err(msg, &self.ast.lexer);
    // self.errors.push(err.clone());
    err
  }
}

pub fn check(ast: &Ast) -> bool {
  let mut typechecker = TypeChecker { ast, symtbl: HashMap::new(), errors: Vec::new() };
  typechecker.visit_stmt(typechecker.ast.top_lvl_id());
  todo!()
}

impl<'a> ast::Visitor<FrontendErrAlias> for TypeChecker<'a> {
  fn visit_expr(&mut self, id: ExprId) -> Result<Type, FrontendErrAlias> {
    match &self.ast.exprs[id.0 as usize] {
      Expr::Literal(lit) => {
        let ty = match lit {
          ExprLiteral::Int(_) => Type::Int,
          ExprLiteral::Float(_) => Type::Float,
          ExprLiteral::Bool(_) => Type::Bool,
          ExprLiteral::Array(expr_ids) => {
            if expr_ids.len() == 0 { return Ok(Type::Untyped) }
            else if expr_ids.len() == 1 { return self.visit_expr(expr_ids[0]) }

            for i in 1..expr_ids.len() {
              let a = self.visit_expr(expr_ids[i-1])?;
              let b = self.visit_expr(expr_ids[i])?;

              if !self.ty_eq(&a, &b) {
                // return Err(self.err("array values must be of the same type", t))
                todo!("handle err")
              }
            }

            // TODO: this is done twice...
            self.visit_expr(expr_ids[0]).unwrap()
          }
        };

        Ok(ty)
      },

      Expr::Variable(id) => {
        let name = self.ast.lexer.str_from_id(*id);
        Ok(self.get_sym_ty(name))
      },

      Expr::Unary { op, rhs } => {
        let ty = self.visit_expr(*rhs)?;

        let ok = match self.ast.get_tok(*op).kind {
          TokenKind::Minus => ty == Type::Int || ty == Type::Float, 
          TokenKind::Keyword(KeywordKind::Not) => ty == Type::Bool,
          _ => false,
        };

        if ok { Ok(ty) } else { todo!("handle err") }
      }
      Expr::Binary { op, lhs, rhs } => {
        let lty = self.visit_expr(*lhs)?;
        let rty = self.visit_expr(*rhs)?;

        if !self.ty_eq(&lty, &rty) { todo!("handle err") }

        use TokenKind::*;
        let res = match self.ast.get_tok(*op).kind {
          Plus | Minus | Star | Slash | Perc if lty == Type::Int || lty == Type::Float => lty,
          Keyword(KeywordKind::And) | Keyword(KeywordKind::Or) if lty == Type::Bool => lty,
          Eq | NotEq | Great | Less | GreatEq | LessEq => Type::Bool,
          _ => return todo!("handle err")
        };

        Ok(res)
      }

      Expr::Call { callee, args } => todo!(),
      Expr::Member { lhs, field } => todo!(),
      Expr::Index { lhs, idx } => todo!(),
    }
  }

  fn visit_stmt(&mut self, id: StmtId) -> Result<(), FrontendErrAlias> {
    match &self.ast.stmts[id.0 as usize] {
      Stmt::Block { stmts } => self.visit_block(stmts.as_slice()),
      Stmt::Decl { name, ty, rhs, constant } => {
        let expr_ty = self.visit_expr(*rhs)?;
        todo!()
      }
      Stmt::FnDecl { name, param_names, ty, block } => todo!(),
      Stmt::StructDecl { ty } => todo!(),
      Stmt::Assign { lhs, rhs } => todo!(),
      Stmt::IfElse { cond, iblock, eblock } => todo!(),
      Stmt::While { cond, wblock } => todo!(),
      Stmt::Return { expr } => todo!(),
      Stmt::Expr(expr_id) => todo!(),
    }
  }
}