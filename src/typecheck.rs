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

  fn add_sym(&mut self, tok_id: TokenId, ty_id: TypeId) {
    let sym = Sym { ty: ty_id };
    let ident = self.add_or_find_ident(tok_id);
    
    self.top_scope_mut().insert(ident, sym);
  }

  fn add_or_find_ident(&mut self, id: TokenId) -> IdentId {
    self.idents.intern(self.ast.lexer.str_from_id(id))
  }

  fn get_sym_ty(&mut self, id: TokenId) -> Type {
    // TODO: is this necessary?
    let ident = self.add_or_find_ident(id);

    todo!()
    // self.top_scope().get(&ident)
    //   .map(|s| s.ty.clone())
    //   .unwrap_or_default()
  }

  fn ty_eq(&self, a: &Type, b: &Type) -> bool {
    todo!()
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
    idents: Default::default(),
    types: Default::default(),
    errors: Vec::new()
  };
  
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

          ExprLiteral::Array(tok_id, expr_ids) => {
            if expr_ids.len() == 0 { return Ok(Type::Untyped) }
            else if expr_ids.len() == 1 { return self.visit_expr(expr_ids[0]) }

            for i in 1..expr_ids.len() {
              let a = self.visit_expr(expr_ids[i-1])?;
              let b = self.visit_expr(expr_ids[i])?;

              if !self.ty_eq(&a, &b) {
                return Err(self.err("array values must be of the same type", *tok_id)); 
              }
            }

            // TODO: this is done twice...
            self.visit_expr(expr_ids[0]).unwrap()
          }
        };

        Ok(ty)
      },

      Expr::Variable(id) => {
        todo!()
      },

      Expr::Unary { op, rhs } => {
        let ty = self.visit_expr(*rhs)?;

        let ok = match self.ast.get_tok(*op).kind {
          TokenKind::Minus => ty == Type::Int || ty == Type::Float, 
          TokenKind::Keyword(KeywordKind::Not) => ty == Type::Bool,
          _ => false,
        };

        if ok { Ok(ty) } else { Err(self.err("unexpected unary op", *op)) }
      }
      Expr::Binary { op, lhs, rhs } => {
        let lty = self.visit_expr(*lhs)?;
        let rty = self.visit_expr(*rhs)?;

        if !self.ty_eq(&lty, &rty) { return Result::Err(self.err("binary op on different types", *op)) }

        use TokenKind::*;
        let res = match self.ast.get_tok(*op).kind {
          Plus | Minus | Star | Slash | Perc if lty == Type::Int || lty == Type::Float => lty,
          Keyword(KeywordKind::And) | Keyword(KeywordKind::Or) if lty == Type::Bool => lty,
          Eq | NotEq | Great | Less | GreatEq | LessEq => Type::Bool,
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
      Stmt::Decl { name, ty, rhs, constant } => {
        self.add_sym(*name, *ty);
        let expr_ty = self.visit_expr(*rhs)?;

        todo!()
        // match (ty, expr_ty) {
          
        // }
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