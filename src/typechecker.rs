use std::{collections::HashMap, hash::Hash};
use crate::{FrontendErr, FrontendErrAlias, IdSize, lexer::Span, parser::{self, Ast, Expr, ExprId, ExprLiteral, IdentId, Stmt, StmtId, StmtTopLvl, TyAnnot, TyAnnotId}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TypeId(IdSize);
type Scope = HashMap<IdentId, TypeId>;

#[derive(Debug, PartialEq, Eq, Hash)]
enum Type {
  Untyped,
  Void,
  Bool,
  Int,
  Float,
  Array { inner: TypeId, len: u32 },
  Func { params: Vec<TypeId>, ret: TypeId },
  // struct name is stored as key in hashmap
  Struct { fields: Vec<(IdentId, TypeId)> }
}

pub mod ty_id {
  use super::TypeId;

  pub const UNTYPED:  TypeId = TypeId(0);
  pub const VOID:     TypeId = TypeId(1);
  pub const BOOL:     TypeId = TypeId(2);
  pub const INT:      TypeId = TypeId(3);
  pub const FLOAT:    TypeId = TypeId(4);
}

struct TypeEnv {
  userdefs: HashMap<IdentId, TypeId>,
  types: Vec<Type>,
}
impl TypeEnv {
  fn new() -> Self {
    Self {
      userdefs: HashMap::new(),
      types: vec![
        Type::Untyped,
        Type::Void,
        Type::Bool,
        Type::Int,
        Type::Float,
      ],
    }
  }

  fn lookup(&self, id: IdentId) -> &Type {
    &self.types[self.userdefs[&id].0 as usize]
  }

  fn lookup_mut(&mut self, id: IdentId) -> &mut Type {
    &mut self.types[self.userdefs[&id].0 as usize]
  }

  fn get(&self, id: TypeId) -> &Type {
    &self.types[id.0 as usize]
  }

  fn get_mut(&mut self, id: TypeId) -> &mut Type {
    &mut self.types[id.0 as usize]
  }

  // true if was already present
  fn add_userdef(&mut self, name: IdentId, ty: Type) -> (TypeId, bool) {
    if let Some(id) = self.userdefs.get(&name) {
      (*id, true)
    } else {
      let id = TypeId(self.types.len() as IdSize);
      self.types.push(ty);
      self.userdefs.insert(name, id);
      (id, false)
    }
  }

  fn add_ty(&mut self, ty: Type) -> TypeId {
    self.types.push(ty);
    TypeId(self.types.len() as IdSize - 1)
  }
}

struct Typechecker<'a> {
  tbl: Vec<Scope>,
  types: TypeEnv,
  ast: &'a Ast<'a>,
}

impl<'a> Typechecker<'a> {
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

  fn err<S: Into<String>>(&self, msg: S, span: Span) -> FrontendErrAlias {
    let err = FrontendErr::new(&self.ast.lexer, msg, span);
    err
  }

  fn add_var(&mut self, name: IdentId, ty: TypeId) -> bool {
    self.top_scope_mut().insert(name, ty).is_some()
  }

  fn get_var_ty(&self, ident: IdentId) -> Option<&Type> {
    self.top_scope()
      .get(&ident)
      .map(|id| self.types.get(*id))
  }

  fn check_expr(&mut self, id: ExprId) -> Result<&Type, FrontendErrAlias> {
    let (e, span) = &self.ast.exprs[id.0 as usize];
    let res = match e {
      Expr::Literal(lit) => match lit {
        ExprLiteral::Int(_) => &Type::Int,
        ExprLiteral::Float(_) => &Type::Float,
        ExprLiteral::Bool(_) => &Type::Bool,
        ExprLiteral::Array(items) => todo!(),
      },
      Expr::Variable(ident) => {
        self.get_var_ty(*ident)
          .ok_or_else(|| self.err("undeclared variable", *span))?
      },
      Expr::Unary { op, rhs } => todo!(),
      Expr::Binary { op, lhs, rhs } => todo!(),
      Expr::Call { callee, args } => todo!(),
      Expr::Member { lhs, field } => todo!(),
      Expr::Index { lhs, idx } => todo!(),
    };

    Ok(res)
  }

  fn check_stmt(&mut self, id: StmtId) -> Result<(), FrontendErrAlias> {
    let (s, span) = &self.ast.stmts[id.0 as usize];
    match s {
      Stmt::Decl(decl) => todo!(),
      Stmt::Assign { lhs, rhs } => {
        let lty = self.check_expr(*lhs)?;
        let rty = self.check_expr(*rhs)?;
        todo!()
      },
      Stmt::Block(items) => self.check_block(items),
      Stmt::IfElse { cond, iblock, eblock } => todo!(),
      Stmt::While { cond, wblock } => todo!(),
      Stmt::Return(_) => todo!(),
      Stmt::Expr(id) => self.check_expr(*id),
    }
  }

  fn check_block(&mut self, stmts: &[StmtId]) -> Result<(), FrontendErrAlias> {
    for s in stmts {
      if let Err(err) = self.check_stmt(*s) {
        eprintln!("[TYPE ERR] {err}");
      }
    }

    Ok(())
  }

  fn check_decl(&mut self, decl: &parser::Decl) {
    todo!()
  }

  fn check_toplvl(&mut self) {
    // first, add all declared userdefs (structs)
    for (stmt, span) in &self.ast.toplvl {
      match stmt {
        // StmtTopLvl::Decl(decl) => {
        //   self.add_var(decl.ident, ty_id::UNTYPED);
        // }
        // StmtTopLvl::FnDecl { name, .. } => { 
        //   self.add_var(*name, ty_id::UNTYPED);
        // }
        StmtTopLvl::Decl(_) | StmtTopLvl::FnDecl { .. } => {}

        StmtTopLvl::StructDecl { name, .. } => {
          let (_, present) = self.types.add_userdef(*name, Type::Struct { fields: Vec::new() });
          if present {
            self.err("already declared struct", *span);
          }
        }
      }
    }

    // now we can parse the top level declarations
    for (stmt, span) in &self.ast.toplvl {
      match stmt {
        StmtTopLvl::Decl(decl) => self.check_decl(decl),
        StmtTopLvl::FnDecl { name, params, ret, block } => { 
          // parse params, ret, and block
          let param_types = params.iter().map(|(_, ty)| *ty).collect();
          let ty = Type::Func { params: param_types, ret:  }
        }

        StmtTopLvl::StructDecl { name, fields } => {
          // parse fields
        }
      }
    }
  }
}

pub fn typecheck(ast: &mut Ast) -> bool {

}