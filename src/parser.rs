use std::collections::HashMap;
use crate::{CursorIter, Err, FrontendErr, lexer::*};

#[derive(Debug)]
pub struct TokenId(usize);
impl From<usize> for TokenId {
  fn from(value: usize) -> Self { Self(value) }
}

#[derive(Debug)]
pub struct ExprId(usize);
impl From<usize> for ExprId {
  fn from(value: usize) -> Self { Self(value) }
}

#[derive(Debug)]
pub struct StmtId(usize);
impl From<usize> for StmtId {
  fn from(value: usize) -> Self { Self(value) }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct TypeId(usize);
impl From<usize> for TypeId {
  fn from(value: usize) -> Self { Self(value) }
}

#[derive(Debug)]
pub enum ExprLiteral {
  Int(TokenId),
  Float(TokenId),
  Bool(TokenId),
}

#[derive(Debug)]
pub enum Expr{
  Literal(ExprLiteral),
  Variable(TokenId),
  Unary { op: TokenId, rhs: ExprId },
  Binary { op: TokenId, lhs: ExprId, rhs: ExprId },
  Call { callee: ExprId, args: Vec<ExprId> },
  Member { lhs: ExprId, field: TokenId },
  Index { lhs: ExprId, idx: ExprId },
}

#[derive(Debug)]
pub enum Stmt {
  Decl { name: TokenId, ty: TypeId, rhs: ExprId },
  FnDecl { name: TokenId, params: Vec<TokenId>, ty: TypeId, block: StmtId },
  
  Assign { lhs: ExprId, rhs: ExprId },
  Block { stmts: Vec<StmtId> },
  IfElse { cond: ExprId, iblock: StmtId, eblock: StmtId },
  While { cond: ExprId, wblock: StmtId },
  Return { expr: ExprId },
  Expr(ExprId),
}

#[derive(PartialEq, Eq, Hash)]
pub enum Type {
  Untyped,
  Void,
  Int,
  Float,
  Bool,
  Generic(usize),
  Arr { inner: TypeId, len: usize },
  Fn { params: Vec<TypeId>, ret: TypeId },
  Struct { name: String, fields: Vec<(String, TypeId)> }
}

const UNTYPED_ID: TypeId = TypeId(0);

pub struct TypeInfo {
  kind: Type,
  size: usize,
}

pub struct Parser<'a> {
  src: &'a str,
  cursor: Cursor<'a>,

  exprs: Vec<Expr>,
  stmts: Vec<Stmt>,
  types: HashMap<Type, TypeId>,

  top_lvl: Vec<StmtId>,

  errors: Vec<FrontendErr>,
}
impl<'a> Parser<'a> {
  pub fn new(src: &'a str, lexer: &'a Lexer) -> Self {
    let mut types = HashMap::new();
    types.insert(Type::Untyped, 0.into());
    types.insert(Type::Void, 1.into());
    types.insert(Type::Bool, 2.into());
    types.insert(Type::Int, 3.into());
    types.insert(Type::Float, 4.into());

    Parser {
      src,
      cursor: Cursor { lexer, curr: 0 },
      exprs: Vec::new(),
      stmts: Vec::new(),
      types,

      top_lvl: Vec::new(),
      errors: Vec::new(),
    }
  }

  pub fn err<S: Into<String>>(&self, msg: S, t: &Token) -> Err {
    t.to_err(msg, self.cursor.lexer)
  }

  pub fn push_expr(&mut self, e: Expr) -> ExprId {
    self.exprs.push(e);
    ExprId(self.exprs.len()-1)
  }

  pub fn push_stmt(&mut self, e: Stmt) -> StmtId {
    self.stmts.push(e);
    StmtId(self.stmts.len()-1)
  }

  pub fn push_type(&mut self, e: Expr) -> TypeId {
    self.exprs.push(e);
    TypeId(self.exprs.len()-1)
  }

  
  fn parse_expr(&mut self, prec_lvl: i8) -> Result<ExprId, Err> {
    let t = self.cursor.eat();
    let id = self.cursor.prev_id();

    use TokenKind::*;
    let mut lhs = match t.kind {
      IntLit(_)   => self.push_expr(Expr::Literal(ExprLiteral::Int(id))),
      FloatLit(_) => self.push_expr(Expr::Literal(ExprLiteral::Float(id))),
      Keyword(KeywordKind::True)  => self.push_expr(Expr::Literal(ExprLiteral::Bool(id))),
      Keyword(KeywordKind::False) => self.push_expr(Expr::Literal(ExprLiteral::Bool(id))),

      Ident => self.push_expr(Expr::Variable(id)),

      // prefix op
      Minus | Bang => {
        let lvl = prefix_lvl(t.kind);
        let rhs = self.parse_expr(lvl)?;
        self.push_expr(Expr::Unary { op: id, rhs })
      }

      ParenL => {
        let lhs = self.parse_expr(0)?;
        self.cursor.eat_match(TokenKind::ParenR, "unclosed parenthesis", &t)?;
        lhs
      }

      _ => return Result::Err(self.err("invalid expression", &t)),
    };

    while !self.cursor.at_end() {
      let op = self.cursor.peek();
      if !op.kind.is_op() { break }
      let id = self.cursor.curr_id();

      if let Some(postfix) = postfix_lvl(op.kind) {
        todo!("postfix op")
      }

      let (left_lvl, right_lvl) = infix_lvl(op.kind);
      if left_lvl < prec_lvl { break }
      self.cursor.advance();

      let rhs = self.parse_expr(right_lvl)?;
      lhs = self.push_expr(Expr::Binary { op: id, lhs, rhs });
    }

    Ok(lhs)
  }

  fn parse_assign(&mut self, lhs: ExprId) -> Result<StmtId, Err> {
    // eat '='
    self.cursor.advance();
    let rhs = self.parse_expr(0)?;

    let stmt = self.push_stmt(Stmt::Assign { lhs, rhs });
    Ok(stmt)
  }

  fn parse_decl(&mut self, name: TokenId) -> Result<StmtId, Err> {
    // eat ':'
    self.cursor.advance();

    let ty_id = if self.cursor.peek().kind == TokenKind::Assign {
      self.cursor.advance();
      UNTYPED_ID
    } else {
      todo!("parse type")
    };

    let rhs = self.parse_expr(0)?;

    let stmt = self.push_stmt(Stmt::Decl { name, ty: ty_id, rhs });
    Ok(stmt)
  }

  pub fn parse_stmt(&mut self) -> Result<StmtId, Err> {
    let s = match self.cursor.peek().kind {
      TokenKind::Ident => {
        let tok_id = self.cursor.curr_id();
        let lhs = self.parse_expr(0)?;

        let op = self.cursor.peek();
        match op.kind {
          TokenKind::Assign => self.parse_assign(lhs)?,
          TokenKind::Colon => self.parse_decl(tok_id)?,
          _ => {
            let id = self.parse_expr(0)?;
            self.push_stmt(Stmt::Expr(id))
          }
        }
      }

      _ => todo!("statment kind not handled")
    };

    if let Some(_) = self.cursor.eat_if(TokenKind::Semicolon) {}

    Ok(s)
  }
}

fn prefix_lvl(kind: TokenKind) -> i8 {
  match kind {
    TokenKind::Minus  => 30,
    TokenKind::Keyword(KeywordKind::Not) => 29,
    _ => -1,
  }
}

fn postfix_lvl(kind: TokenKind) -> Option<i8> {
  let lvl = match kind {
    TokenKind::ParenL => 32,
    TokenKind::BraceL => 32,
    TokenKind::Dot => 34,
    _ => return None,
  };

  Some(lvl)
}

fn infix_lvl(kind: TokenKind) -> (i8, i8) {
  use TokenKind::*;

  match kind {
    Plus | Minus => (23, 24),
    Eq | NotEq => (16, 17),
    Great | GreatEq | Less | LessEq => (14, 15),
    Star | Slash | Perc => (23, 24),
    Keyword(KeywordKind::And) => (12, 13),
    Keyword(KeywordKind::Or) => (10, 11),
    Caret => (27, 28),
    _ => (-1, -1),
  }
}

struct Cursor<'a> {
  lexer: &'a Lexer<'a>,
  curr: usize,
}
impl<'a> CursorIter<Token, Token> for Cursor<'a> {
  fn peek_nth(&self, nth: usize) -> Token {
    self.lexer.tokens.get(self.curr() + nth)
    .cloned()
    .unwrap_or_else(|| Token { kind: TokenKind::Err('\0'), info: Span::default() } )
  }

  fn start(&self) -> &[Token] { &self.lexer.tokens }
  fn curr(&self) -> usize { self.curr }
  fn curr_mut(&mut self) -> &mut usize { &mut self.curr }
}

pub struct Ast {
  pub tokens: Vec<Token>,
  pub exprs: Vec<Expr>,
  pub stmts: Vec<Stmt>,
}

pub fn parse(src: &str) -> Ast {
  let lexer = tokenize(src);
  let mut p = Parser::new(src, &lexer);

  while !p.cursor.at_end() {
    let stmt = p.parse_stmt();

    if let Err(e) = stmt {
      eprintln!("[PARSE ERR] {e}");
      p.errors.push(e);
      p.cursor.eat_until_safe();
    }
  }

  Ast {
    exprs: p.exprs,
    stmts: p.stmts,
    tokens: lexer.tokens,
  }
}

impl<'a> Cursor<'a> {
  fn eat_if(&mut self, kind: TokenKind) -> Option<Token> {
    (self.peek().kind == kind).then(|| self.eat())
  }

  fn eat_match<S: Into<String>>(&mut self, target: TokenKind, msg: S, err_tok: &Token) -> Result<Token, Err> {
    let t = self.eat();
    if t.kind != target {
      Err(err_tok.to_err(msg, self.lexer))
    }
    else { Ok(t) }
  }

  fn eat_until_safe(&mut self) {
    while !self.at_end() {
      if self.peek().kind.is_safe() { break }
      self.advance();
    }
  }

  fn curr_id(&self) -> TokenId {
    self.curr().into()
  }

  fn prev_id(&self) -> TokenId {
    (self.curr().saturating_sub(1)).into()
  }
}