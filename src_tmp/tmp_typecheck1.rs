use std::{collections::HashMap, fmt, mem};

use crate::{FrontendErrAlias, ast::{self, Ast, IdentId, TypeInterner, Visitor}, lexer::{KeywordKind, TokenKind}, parser::{Expr, ExprId, ExprLiteral, Stmt, StmtId, Type, TypeId, ty_id}};

#[derive(Debug)]
pub struct Sym {
  ty: TypeId,
}

type Scope = HashMap<IdentId, Sym>;

// https://www.reasoning.page/2021/10/21/hindley-milner-type-inference-in-rust/
#[derive(Default)]
pub struct TypeChecker {
  pub symtbl: Vec<Scope>,
  pub types: TypeInterner,
  errors: Vec<FrontendErrAlias>,
}

impl TypeChecker {
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

  fn add_sym(&mut self, ident: IdentId, ty: TypeId) {
    let sym = Sym { ty };
    self.top_scope_mut().insert(ident, sym);
  }

  fn get_sym_ty(&mut self, id: IdentId) -> Option<&Type> {
    self.top_scope_mut()
      .get(&id)
      .map(|sym| sym.ty)
      .map(|id| self.types.lookup(id))
  }

  fn ty_eq(&self, a: TypeId, b: TypeId) -> bool {
    let a = self.types.lookup(a);
    let b = self.types.lookup(b);
    
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
        name_a == name_b && fields_a.iter().zip(fields_b.iter()).all(|(a, b)| a.0 == b.0 && self.ty_eq(a.1, b.1))
      }
      _ => false,
    }
  }

  pub fn ty_is_recursive(&self, root: TypeId) -> bool {
    fn rec(tc: &TypeChecker, root: TypeId, id: TypeId) -> bool {
      let curr = tc.types.lookup(id);

      match curr {
        Type::Array { inner, .. } => {
          root == *inner || rec(tc, root, *inner)
        }
        Type::Struct { fields, .. } => {
          fields.iter().any(|(_, param_ty)| root == *param_ty || rec(tc, root, *param_ty))
        }
        _ => false,
      }
    }
    
    rec(self, root, root)
  }

  fn ty_resolve_user_defs(&mut self, ty: &mut Type) {

  }

  fn top_lvl_pass(&mut self, ast: &Ast) -> Result<(), FrontendErrAlias> {
    let stmts = ast.top_lvl_block();

    // add top level declarations to symtbl
    for id in stmts {
      let stmt = ast.get_stmt(id);

      match stmt {
        Stmt::Decl { ident, ty, .. } => self.add_sym(*ident, *ty),
        Stmt::FnDecl { ident, ty, .. } => self.add_sym(*ident, *ty),
        Stmt::StructDecl { ident, ty, .. } => self.add_sym(*ident, *ty),
        _ => {}
      }
    }

    // resolve all UserDefs
    for ty in self.types.buf.iter_mut() {
      *ty = mem::take(match ty {
        Type::Array { inner, len } => todo!(),
        Type::Func { params, ret } => todo!(),
        Type::Struct { name, fields } => todo!(),
        Type::UserDef { name } => todo!(),

        Type::Bool | Type::Int | Type::Float | Type::Void | Type::Untyped => ty,
      })
    }

    Ok(())
  }
}

pub fn check(ast: &mut Ast) -> TypeChecker {
  let mut tc = TypeChecker {
    symtbl: vec![],
    // TODO: maybe its more clear to pass typesEnv as second parameter and not keep it into ast
    types: mem::take(&mut ast.types),
    errors: vec![],
  };
  // global scope
  tc.push_scope();

  tc.top_lvl_pass(ast);
  tc
}

// impl ast::Visitor<TypeId, FrontendErrAlias> for TypeChecker {
//   fn visit_expr(&mut self, ast: &Ast, id: ExprId) -> Result<TypeId, FrontendErrAlias> {
//     match ast.get_expr(id) {
//       Expr::Literal(lit) => {
//         let ty = match lit {
//           ExprLiteral::Int(_)   => ty_id::INT.into(),
//           ExprLiteral::Float(_) => ty_id::FLOAT.into(),
//           ExprLiteral::Bool(_)  => ty_id::BOOL.into(),

//           ExprLiteral::Array(tok_id, expr_ids) => {
//             if expr_ids.len() == 0 { return Ok(ty_id::UNTYPED.into()) }
//             else if expr_ids.len() == 1 { return self.visit_expr(ast, expr_ids[0]) }

//             for i in 1..expr_ids.len() {
//               let a = self.visit_expr(ast, expr_ids[i-1])?;
//               let b = self.visit_expr(ast, expr_ids[i])?;

//               if !self.ty_eq(a, b) {
//                 return Err(ast.err("array values must be of the same type", *tok_id)); 
//               }
//             }

//             // TODO: this is done twice...
//             self.visit_expr(ast, expr_ids[0]).unwrap()
//           }
//         };

//         Ok(ty)
//       },

//       Expr::Variable(tok, ident) => {
//         self.get_sym_ty(*ident).ok_or_else(|| ast.err("undeclared variable", *tok))
//       },

//       Expr::Unary { op, rhs } => {
//         let ty = self.visit_expr(ast, *rhs)?;

//         let ok = match ast.get_tok(*op).kind {
//           TokenKind::Minus => ty == ty_id::INT || ty == ty_id::FLOAT, 
//           TokenKind::Keyword(KeywordKind::Not) => ty == ty_id::BOOL,
//           _ => false,
//         };

//         if ok { Ok(ty) } else { Err(ast.err("unexpected unary op", *op)) }
//       }
//       Expr::Binary { op, lhs, rhs } => {
//         let lty = self.visit_expr(ast, *lhs)?;
//         let rty = self.visit_expr(ast, *rhs)?;

//         if !self.ty_eq(lty, rty) { return Result::Err(ast.err("binary op on different types", *op)) }

//         use TokenKind::*;
//         let res = match ast.get_tok(*op).kind {
//           Plus | Minus | Star | Slash | Perc if lty == ty_id::INT || lty == ty_id::FLOAT => lty,
//           Keyword(KeywordKind::And) | Keyword(KeywordKind::Or) if lty == ty_id::BOOL => lty,
//           Eq | NotEq | Great | Less | GreatEq | LessEq => ty_id::BOOL,
//           _ => return Result::Err(ast.err("unexpected binary op", *op))
//         };

//         Ok(res)
//       }

//       Expr::Call { callee, args } => todo!(),
//       Expr::Member { lhs, field } => todo!(),
//       Expr::Index { lhs, idx } => todo!(),
//     }
//   }

//   fn visit_stmt(&mut self, ast: &Ast, id: StmtId) -> Result<(), FrontendErrAlias> {
//     match ast.get_stmt(id) {
//       Stmt::Block(stmts) => self.visit_block(ast, stmts.as_slice()),

//       Stmt::Decl { name, ident, ty: ty_id, rhs, constant } => {
//         let expr_ty_id = self.visit_expr(ast, *rhs)?;
        
//         let decl_ty = ast.get_ty(*ty_id);
//         let expr_ty = ast.get_ty(expr_ty_id);
        
//         let res = match (decl_ty, expr_ty) {
//           (Type::Untyped, Type::Untyped) => return Err(ast.err("could not infer types as both are unknown", *name)),
//           (Type::Untyped, _) => expr_ty_id,
//           (_, Type::Untyped) => *ty_id,
//           (_, _) => if self.ty_eq(*ty_id, expr_ty_id) {
//             // same type on both ends
//             *ty_id
//           } else {
//             // different type
//             return Err(ast.err("different types provided in declaration", *name))
//           }
//         };

//         self.add_sym(*ident, res);
//         Ok(())
//       }

//       Stmt::FnDecl { name, ident, param_names, ty: ty_id, block } => {
//         self.add_sym(*ident, *ty_id);
//         let Type::Func { params: param_types, ret: ret_ty } = ast.get_ty(*ty_id) else {
//           unreachable!()
//         };

//         self.push_scope();
//         for ((_, ident), ty) in param_names.iter().zip(param_types.iter()) {
//           self.add_sym(*ident, *ty);
//         }
//         self.visit_stmt(ast, *block)?;
//         self.pop_scope();

//         Ok(())
//       }

//       Stmt::StructDecl { tok, ident, ty } => {
//         // check for recursive struct declaration; that is not allowed
//         if self.ty_is_recursive(*ty) {
//           Err(ast.err("declared recursive struct", *tok))
//         } else {
//           Ok(())
//         }
//       },

//       Stmt::Assign { tok, lhs, rhs } => {
//         let lty = self.visit_expr(ast, *lhs)?;
//         let rty = self.visit_expr(ast, *rhs)?;
//         self.ty_eq(lty, rty)
//           .then_some(())
//           .ok_or_else(|| {
//             ast.err("assignin value of different type", *tok)
//           })
//       },

//       Stmt::IfElse { tok, cond, iblock, eblock } => {
//         let ty = self.visit_expr(ast, *cond)?;

//         if !matches!(ast.get_ty(ty), Type::Bool) {
//           return Err(ast.err("if condition must be bool", *tok))
//         }

//         self.push_scope();
//         self.visit_stmt(ast, *iblock)?;
//         self.pop_scope();

//         if let Some(eblock) = eblock {
//           self.push_scope();
//           self.visit_stmt(ast, *eblock)?;
//           self.pop_scope();
//         }

//         Ok(())
//       },

//       Stmt::While { tok, cond, wblock } => {
//         let ty = self.visit_expr(ast, *cond)?;

//         if !matches!(ast.get_ty(ty), Type::Bool) {
//           return Err(ast.err("while condition must be bool", *tok))
//         }

//         self.push_scope();
//         self.visit_stmt(ast, *wblock)?;
//         self.pop_scope();

//         Ok(())
//       },

//       Stmt::Return { tok, expr } => todo!(),

//       Stmt::Expr(id) => self.visit_expr(ast, *id).map(|_| ()),
//     }
//   }

//   fn visit_block(&mut self, ast: &Ast, ids: &[StmtId]) -> Result<(), FrontendErrAlias> {
//     for id in ids {
//       if let Err(e) = self.visit_stmt(ast, *id) {
//         eprintln!("[TYPE ERR]: {e}");
//         self.errors.push(e);
//       }
//     }
    
//     Ok(())
//   }
// }