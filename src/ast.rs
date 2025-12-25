use std::{collections::HashMap, hash::{self, Hash, Hasher}};
use crate::{IdSize, lexer::{Lexer, Span, Token}, parser::{Expr, ExprId, ExprLiteral, Spanned, Stmt, StmtId, StmtTopLvl, TokenId, TyAnnot, TyAnnotId}};

pub struct Ast<'a> {
  pub lexer: Lexer<'a>,

  pub exprs: Vec<Spanned<Expr>>,
  pub stmts: Vec<Spanned<Stmt>>,
  pub toplvl: Vec<Spanned<StmtTopLvl>>,
  pub annots: Vec<Spanned<TyAnnot>>,
  pub idents: StringInterner,
}
impl<'a> Ast<'a> {
  fn get_token(&self, id: TokenId) -> &Token {
    &self.lexer.tokens[id.0 as usize]
  }

  fn get_expr(&self, id: ExprId) -> &Expr {
    &self.exprs[id.0 as usize].0
  }

  fn get_stmt(&self, id: StmtId) -> &Stmt {
    &self.stmts[id.0 as usize].0
  }

  fn get_toplvl(&self, id: StmtId) -> &StmtTopLvl {
    &self.toplvl[id.0 as usize].0
  }

  fn get_annot(&self, id: TyAnnotId) -> &TyAnnot {
    &self.annots[id.0 as usize].0
  }

  fn get_ident(&self, id: IdentId) -> &str {
    &self.idents.lookup(id)
  }

  

  pub fn dbg_exprs(&self) {
    println!("EXPRS:");
    for id in 0..self.exprs.len() {
      println!("{id} -> {}", self.dbg_expr(ExprId(id as IdSize)));
    }
    println!()
  }

  fn dbg_expr(&self, id: ExprId) -> String {
    let e = self.get_expr(id);
    match e {
      Expr::Literal(lit) => match lit {
        ExprLiteral::Int(id) 
        | ExprLiteral::Float(id) 
        | ExprLiteral::Bool(id) => format!("Primitive lit: {:?}", self.get_token(*id)),
        ExprLiteral::Array(expr_ids) => {
          let mut str = format!("Array lit: [");
          for id in expr_ids {
            str.push_str(&format!("{:?}, ", self.dbg_expr(*id)))
          }
          str.push(']');
          str
        },
        ExprLiteral::Struct(ident, expr_ids) => {
          let mut str = format!("Struct lit {}: {{", self.get_ident(*ident));
          for id in expr_ids {
            str.push_str(&format!("{:?}, ", self.dbg_expr(*id)))
          }
          str.push('}');
          str
        },
      },
      Expr::Variable(id) => format!("Variable: {:?}", self.get_ident(*id)),
      Expr::Unary { op, rhs } => format!("Unary: {:?} {}", self.get_token(*op), self.dbg_expr(id)),
      Expr::Binary { op, lhs, rhs } => format!("Binary: {} {:?} {}", self.dbg_expr(*lhs), op, self.dbg_expr(*rhs)),
      Expr::Call { callee, args } => {
        let mut str = format!("Call of {}: (", self.dbg_expr(*callee));
        for id in args {
          str.push_str(&format!("{}", self.dbg_expr(*id)));
        }
        str.push(')');
        str
      }
      Expr::Member { lhs, field } => todo!(),
      Expr::Index { lhs, idx } => todo!(),
    }
  }

  pub fn dbg_stmts(&self) {
    println!("STMTS:");
    for id in 0..self.stmts.len() {
      println!("{id} -> {}", self.dbg_stmt(StmtId(id as IdSize)));
    }
    println!()
  }

  fn dbg_stmt(&self, id: StmtId) -> String {
    let s = self.get_stmt(id);
    
    match s {
      Stmt::Decl(decl) => format!("Decl: {} {:?} {}", self.get_ident(decl.ident), decl.annot.map(|id| self.dbg_annot(id)), self.dbg_expr(decl.rhs)),
      Stmt::Assign { lhs, rhs } => format!("Assign: {} {}", self.dbg_expr(*lhs), self.dbg_expr(*rhs)),
      Stmt::Block(stmt_ids) => {
        let mut str = format!("Block: {{\n");

        for id in stmt_ids {
          str.push_str(&format!("{}\n", self.dbg_stmt(*id)));
        }

        str.push('}');
        str
      },
      Stmt::IfElse { cond, iblock, eblock } => todo!(),
      Stmt::While { cond, wblock } => todo!(),
      Stmt::Return(expr_id) => todo!(),
      
      Stmt::Expr(id) => self.dbg_expr(*id),
    }
  }

  pub fn dbg_toplvls(&self) {
    println!("TOPLVLS:");
    for id in 0..self.toplvl.len() {
      println!("{id} -> {}", self.dbg_toplvl(StmtId(id as IdSize)));
    }
    println!()
  }

  fn dbg_toplvl(&self, id: StmtId) -> String {
    let s = self.get_toplvl(id);
    
    match s {
      StmtTopLvl::Decl(decl) => format!("Decl: {} {:?} {}", self.get_ident(decl.ident), decl.annot.map(|id| self.dbg_annot(id)), self.dbg_expr(decl.rhs)),
      StmtTopLvl::FnDecl { name, params, ret, block, .. } => {
        let mut str = format!("Func {}: (", self.get_ident(*name));
        for param in params {
          str.push_str(&format!("{}: {}, ", self.get_ident(param.0), self.dbg_annot(param.1)));
        }
        str.push_str(") -> ");
        str.push_str(&format!("{:?}\n", ret.map(|id| self.dbg_annot(id))));
        str.push_str(&self.dbg_stmt(*block));
        str
      },
      StmtTopLvl::StructDecl { name, fields, is_generic } => {
        let mut str = format!("Struct {}: {{", self.get_ident(*name));
        for field in fields {
          str.push_str(&format!("{}: {}, ", self.get_ident(field.0), self.dbg_annot(field.1)));
        }
        str.push('}');
        str
      },
    }
  }

  pub fn dbg_annots(&self) {
    println!("ANNOTS:");
    for id in 0..self.annots.len() {
      println!("{id} -> {}", self.dbg_annot(TyAnnotId(id as IdSize)));
    }
    println!()
  } 

  fn dbg_annot(&self, id: TyAnnotId) -> String {
    let t = self.get_annot(id);

    match t {
      TyAnnot::Bool
      | TyAnnot::Int
      | TyAnnot::Float => format!("{:?}", t),
      TyAnnot::Generic(id) => format!("Generic: {}", self.get_ident(*id)),
      TyAnnot::Array { inner, len } => format!("Array: {} {:?}", self.dbg_annot(*inner), len.map(|id| self.dbg_expr(id))),
      TyAnnot::Func { params, ret } => {
        let mut str = format!("Func: (");
        for param in params {
          str.push_str(&format!("{}, ", self.dbg_annot(*param)));
        }
        str.push_str(") -> ");
        str.push_str(&format!("{:?}", ret.map(|id| self.dbg_annot(id))));
        str
      },
      TyAnnot::UserDef { name, generics } => format!("Array: {} {:?}", self.get_ident(*name), generics),
    }
  }

  pub fn dbg_idents(&self) {
    println!("IDENTS:");
    for i in 0..self.idents.hash_to_index.len() {
      println!("{i}\t-> {}", self.idents.lookup(IdentId(i as IdSize)));
    }
    println!()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentId(pub IdSize);

#[derive(Default)]
pub struct StringInterner{
  hash_to_index: HashMap<u64, IdentId>,
  indexes: Vec<Span>,
  pub buf: String,
}

impl StringInterner {
  pub fn intern(&mut self, name: &str) -> IdentId {
    let hash = {
      let mut hasher = hash::DefaultHasher::new();
      name.hash(&mut hasher);
      hasher.finish()
    };

    if let Some(id) = self.hash_to_index.get(&hash) {
      return (*id).into();
    }

    let id = IdentId(self.hash_to_index.len() as IdSize);
    self.hash_to_index.insert(hash, id);
    
    let intern_span = Span {
      start: self.buf.len() as u32,
      end: (self.buf.len() + name.len()) as u32
    };
    self.indexes.push(intern_span);
    self.buf.push_str(name);

    id
  }

  pub fn lookup(&self, id: IdentId) -> &str {
    let span = &self.indexes[id.0 as usize];
    &self.buf[span.start as usize..span.end as usize]
  }
}