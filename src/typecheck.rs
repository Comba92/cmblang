use std::collections::HashMap;
use crate::{FrontendErrAlias, ast::{self, Ast, IdentId, Visitor}, lexer::{KeywordKind, TokenId, TokenKind}, parser::*};

struct Sym {
  ty: TypeId,
}

type Scope = HashMap<IdentId, Sym>;

struct TypeChecker<'a> {
  symtbl: Vec<Scope>,
  ast: &'a Ast<'a>,
  errors: Vec<FrontendErrAlias>,
}
impl<'a> TypeChecker<'a> {
  fn top_scope(&self) -> &Scope {
    let len = self.symtbl.len()-1;
    &self.symtbl[len]
  }

  fn top_scope_mut(&mut self) -> &mut Scope {
    let len = self.symtbl.len()-1;
    &mut self.symtbl[len]
  }

  fn push_scope(&mut self) {
    self.symtbl.push(HashMap::new());
  }

  fn pop_scope(&mut self) {
    self.symtbl.pop();
  }

  fn add_sym(&mut self, ident: IdentId, ty_id: TypeId) {
    let sym = Sym { ty: ty_id };
    self.top_scope_mut().insert(ident, sym);
  }

  fn get_sym_ty(&mut self, id: IdentId) -> Option<TypeId> {
    self.top_scope_mut().get(&id).map(|sym| sym.ty)
  }

  fn ty_eq(&self, a: TypeId, b: TypeId) -> bool {
    let a = self.ast.get_ty(a);
    let b = self.ast.get_ty(b);
    
    use Type::*;
    match (a, b) {
      (Bool, Bool) | (Int, Int) | (Float, Float) => true,
      (Array { inner: inner_a, len: len_a }, Array { inner: inner_b, len: len_b }) => {
        self.ty_eq(*inner_a, *inner_b) && len_a == len_b
      }
      (Func { params: params_a, ret: ret_a }, Func { params: params_b, ret: ret_b }) => {
        params_a.iter().zip(params_b.iter()).all(|(a, b)| self.ty_eq(*a, *b)) && self.ty_eq(*ret_a, *ret_b)
      }
      (Struct { name: name_a, fields: fields_a }, Struct { name: name_b, fields: fields_b }) => {
        todo!("struct type eq")
      }
      _ => false,
    }
  }

  fn err<S: Into<String>>(&self, msg: S, id: TokenId) -> FrontendErrAlias {
    let t = self.ast.lexer.get_tok(id);
    let err = t.to_err(msg, &self.ast.lexer);
    // self.errors.push(err.clone());
    err
  }
}

pub fn check(ast: &Ast) -> bool {
  let mut typechecker = TypeChecker {
    ast,
    symtbl: vec![HashMap::new()],
    errors: Vec::new()
  };
  
  typechecker.visit_stmt(typechecker.ast.top_lvl_id());
  true
}

impl<'a> ast::Visitor<FrontendErrAlias> for TypeChecker<'a> {
  fn visit_expr(&mut self, id: ExprId) -> Result<TypeId, FrontendErrAlias> {
    match &self.ast.exprs[id.0 as usize] {
      Expr::Literal(lit) => {
        let ty = match lit {
          ExprLiteral::Int(_)   => ty_id::INT,
          ExprLiteral::Float(_) => ty_id::FLOAT,
          ExprLiteral::Bool(_)  => ty_id::BOOL,

          ExprLiteral::Array(tok_id, expr_ids) => {
            if expr_ids.len() == 0 { return Ok(ty_id::UNTYPED) }
            else if expr_ids.len() == 1 { return self.visit_expr(expr_ids[0]) }

            for i in 1..expr_ids.len() {
              let a = self.visit_expr(expr_ids[i-1])?;
              let b = self.visit_expr(expr_ids[i])?;

              if !self.ty_eq(a, b) {
                return Err(self.err("array values must be of the same type", *tok_id)); 
              }
            }

            // TODO: this is done twice...
            self.visit_expr(expr_ids[0]).unwrap()
          }
        };

        Ok(ty)
      },

      Expr::Variable(tok, ident) => {
        self.get_sym_ty(*ident).ok_or_else(|| self.err("undeclared variable", *tok))
      },

      Expr::Unary { op, rhs } => {
        let ty = self.visit_expr(*rhs)?;

        let ok = match self.ast.get_tok(*op).kind {
          TokenKind::Minus => ty == ty_id::INT || ty == ty_id::FLOAT, 
          TokenKind::Keyword(KeywordKind::Not) => ty == ty_id::BOOL,
          _ => false,
        };

        if ok { Ok(ty) } else { Err(self.err("unexpected unary op", *op)) }
      }
      Expr::Binary { op, lhs, rhs } => {
        let lty = self.visit_expr(*lhs)?;
        let rty = self.visit_expr(*rhs)?;

        if !self.ty_eq(lty, rty) { return Result::Err(self.err("binary op on different types", *op)) }

        use TokenKind::*;
        let res = match self.ast.get_tok(*op).kind {
          Plus | Minus | Star | Slash | Perc if lty == ty_id::INT || lty == ty_id::FLOAT => lty,
          Keyword(KeywordKind::And) | Keyword(KeywordKind::Or) if lty == ty_id::BOOL => lty,
          Eq | NotEq | Great | Less | GreatEq | LessEq => ty_id::BOOL,
          _ => return Result::Err(self.err("unexpected binary op", *op))
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

      Stmt::Decl { name, ident, ty: ty_id, rhs, constant } => {
        let expr_ty_id = self.visit_expr(*rhs)?;
        
        let decl_ty = self.ast.get_ty(*ty_id);
        let expr_ty = self.ast.get_ty(expr_ty_id);
        
        let res = match (decl_ty, expr_ty) {
          (Type::Untyped, Type::Untyped) => return Err(self.err("could not infer types as both are unknown", *name)),
          (Type::Untyped, _) => expr_ty_id,
          (_, Type::Untyped) => *ty_id,
          (a, b) => if self.ty_eq(*ty_id, expr_ty_id) {
            // same type on both ends
            *ty_id
          } else {
            // different type
            return Err(self.err("different types provided in declaration", *name))
          }
        };

        self.add_sym(*ident, res);
        Ok(())
      }

      Stmt::FnDecl { name, ident, param_names, ty: ty_id, block } => {
        self.add_sym(*ident, *ty_id);
        let Type::Func { params: param_types, ret: ret_ty } = self.ast.get_ty(*ty_id) else {
          unreachable!()
        };

        self.push_scope();
        for ((_, ident), ty) in param_names.iter().zip(param_types.iter()) {
          self.add_sym(*ident, *ty);
        }
        self.visit_stmt(*block)?;
        self.pop_scope();

        Ok(())
      }

      Stmt::StructDecl { ty } => todo!(),

      Stmt::Assign { tok, lhs, rhs } => {
        let lty = self.visit_expr(*lhs)?;
        let rty = self.visit_expr(*rhs)?;
        self.ty_eq(lty, rty)
          .then_some(())
          .ok_or_else(|| {
            self.err("assignin value of different type", *tok)
          })
      },

      Stmt::IfElse { tok, cond, iblock, eblock } => {
        let ty = self.visit_expr(*cond)?;

        if !matches!(self.ast.get_ty(ty), Type::Bool) {
          return Err(self.err("if condition must be bool", *tok))
        }

        self.push_scope();
        self.visit_stmt(*iblock)?;
        self.pop_scope();

        if let Some(eblock) = eblock {
          self.push_scope();
          self.visit_stmt(*eblock)?;
          self.pop_scope();
        }

        Ok(())
      },

      Stmt::While { tok, cond, wblock } => {
        let ty = self.visit_expr(*cond)?;

        if !matches!(self.ast.get_ty(ty), Type::Bool) {
          return Err(self.err("while condition must be bool", *tok))
        }

        self.push_scope();
        self.visit_stmt(*wblock)?;
        self.pop_scope();

        Ok(())
      },
      Stmt::Return { expr } => todo!(),

      Stmt::Expr(id) => self.visit_expr(*id).map(|_| ()),
    }
  }
}