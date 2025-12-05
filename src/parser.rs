use std::collections::HashMap;
use crate::{CursorIter, FrontendErr, FrontendErrAlias, IdSize, lexer::*};

// TODO: better error messages

#[derive(Debug, Clone, Copy)]
pub struct ExprId(pub IdSize);
impl From<usize> for ExprId {
  fn from(value: usize) -> Self { Self(value as IdSize) }
}

#[derive(Debug, Clone, Copy)]
pub struct StmtId(pub IdSize);
impl From<usize> for StmtId {
  fn from(value: usize) -> Self { Self(value as IdSize) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub IdSize);
impl From<usize> for TypeId {
  fn from(value: usize) -> Self { Self(value as IdSize) }
}

#[derive(Debug)]
pub enum ExprLiteral {
  Int(TokenId),
  Float(TokenId),
  Bool(TokenId),
  Array(Vec<ExprId>),
}
impl ExprLiteral {
  pub fn token(&self) -> TokenId {
    match self {
      ExprLiteral::Int(id) => *id,
      ExprLiteral::Float(id) => *id,
      ExprLiteral::Bool(id) => *id,
      ExprLiteral::Array(expr_ids) => todo!("no way to get a token from empty array!"),
    }
  }
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
impl Expr {
  pub fn token(&self) -> TokenId {
    match self {
      Expr::Literal(lit) => lit.token(),
      Expr::Variable(id) => *id,
      Expr::Unary { op, rhs } => *op,
      Expr::Binary { op, lhs, rhs } => *op,
      Expr::Call { callee, args } => todo!("get token from callee?"),
      Expr::Member { lhs, field } => todo!("get token from member?"),
      Expr::Index { lhs, idx } => todo!("get token from lhs?"),
    }
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

#[derive(Debug)]
pub enum Stmt {
  Decl { name: TokenId, ty: TypeId, rhs: ExprId, constant: bool },
  FnDecl { name: TokenId, param_names: Vec<TokenId>, ty: TypeId, block: StmtId },
  StructDecl { ty: TypeId },

  Assign { lhs: ExprId, rhs: ExprId },
  Block { stmts: Vec<StmtId> },
  IfElse { cond: ExprId, iblock: StmtId, eblock: Option<StmtId> },
  While { cond: ExprId, wblock: StmtId },
  Return { expr: ExprId },
  Expr(ExprId),
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, strum::EnumCount)]
pub enum Type {
  #[default]
  Untyped,
  Void,
  Int,
  Float,
  Bool,
  Array { inner: TypeId, len: Option<usize> },
  Func { params: Vec<TypeId>, ret: TypeId },
  // TODO: not sure about keeping strings here
  Struct { name: TokenId, fields: Vec<(TokenId, TypeId)> }
}

pub struct TypeInfo {
  kind: Type,
  constant: bool,
}

pub struct Parser<'a> {
  cursor: Cursor<'a>,

  exprs: Vec<Expr>,
  stmts: Vec<Stmt>,
  types: HashMap<Type, TypeId>,

  errors: Vec<FrontendErr>,
}

type ParseResult<T> = Result<T, FrontendErrAlias>;

pub mod ty_id {
  use super::TypeId;

  pub const UNTYPED:  TypeId = TypeId(0);
  pub const VOID:     TypeId = TypeId(1);
  pub const BOOL:     TypeId = TypeId(2);
  pub const INT:      TypeId = TypeId(3);
  pub const FLOAT:    TypeId = TypeId(4);
}

impl<'a> Parser<'a> {
  pub fn new(lexer: &'a Lexer) -> Self {
    let mut types = HashMap::new();

    use ty_id::*;
    types.insert(Type::Untyped, UNTYPED);
    types.insert(Type::Void, VOID);
    types.insert(Type::Bool, BOOL);
    types.insert(Type::Int, INT);
    types.insert(Type::Float, FLOAT);

    Parser {
      cursor: Cursor { lexer, curr: 0 },
      exprs: Vec::new(),
      stmts: Vec::new(),
      types,

      errors: Vec::new(),
    }
  }

  pub fn err<S: Into<String>>(&self, msg: S, t: &Token) -> FrontendErrAlias {
    let err = t.to_err(msg, self.cursor.lexer);
    // self.errors.push(err.clone());
    err
  }

  pub fn push_expr(&mut self, e: Expr) -> ExprId {
    self.exprs.push(e);
    ExprId(self.exprs.len() as u32 - 1)
  }

  pub fn push_stmt(&mut self, s: Stmt) -> StmtId {
    self.stmts.push(s);
    StmtId(self.stmts.len() as u32 - 1)
  }

  pub fn push_type(&mut self, ty: Type) -> TypeId {
    self.types.get(&ty)
      .map(|x| *x)
      .unwrap_or_else(|| {
        self.types.insert(ty, self.types.len().into());
        self.types.len().into()
      })
  }

  fn parse_type(&mut self) -> ParseResult<TypeId> {
    let t = self.cursor.eat();
    
    let ty = match t.kind {
      TokenKind::Keyword(k) => match k {
        KeywordKind::Bool => Type::Bool,
        KeywordKind::Int => Type::Int,
        KeywordKind::Float => Type::Float,
        _ => return Err(self.err("invalid type annotation", &t)),
      }

      TokenKind::ParenL => {
        let params = self.collect_listing(
          Self::parse_type,
          TokenKind::Comma,
          TokenKind::ParenR,
          "unclosed parenthesis in function annotation's params")?;
 
        let ret = if self.cursor.eat_if(TokenKind::Arrow).is_some() {
          self.parse_type()?
        } else { ty_id::VOID };

        Type::Func { params, ret }
      }

      TokenKind::BraceL => {
        let inner = self.parse_type()?;
        self.cursor.eat_match(TokenKind::Colon, "expect ':' after array inner type")?;
        
        // TODO: might be cool if this can be a constant integer expression?
        let len_tok = self.cursor.eat();

        let len = match len_tok.kind {
          TokenKind::Star => None,
          TokenKind::IntLit(len) => Some(len as usize),
          _ => return Err(self.err("expect integer literal or '*' (inferred size) for size in array type annotation", &len_tok)) 
        };

        self.cursor.eat_match(TokenKind::BraceR, "expect closing ']' in array type annotation")?;

        Type::Array { inner, len }
      }
      TokenKind::Ident => todo!("parse user defined type"),

      _ => return Err(self.err("invalid type annotation", &t)),
    };

    Ok(self.push_type(ty))
  }

  fn collect_listing<T, F>(&mut self, getter: F, separator: TokenKind, terminator: TokenKind, err_unclosed: &str) -> ParseResult<Vec<T>>
    where
      F: Fn(&mut Self) -> ParseResult<T>,
  {
    let mut list = Vec::new();
    loop {
      if !self.cursor.has_some() {
        return Err(self.err(err_unclosed, &self.cursor.lexer.eof()))
      }
      if self.cursor.eat_if(terminator).is_some() { break }

      let item = getter(self)?;
      list.push(item);

      if self.cursor.eat_if(separator).is_none() {
        // if we don't find a comma, we are expecting a paren closing
        // if we don't get a paren closing, it is an error
        if self.cursor.eat_if(terminator).is_some() { break }
      }
    }

    Ok(list)
  }

  // TODO: there should probably be a wrapper version which takes a custom error as arg if parsing fails 
  fn parse_expr(&mut self, prec_lvl: i8) -> ParseResult<ExprId> {
    let id = self.cursor.curr_id();
    let t = self.cursor.eat();

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
        self.cursor.eat_match_ex(TokenKind::ParenR, "unclosed parenthesis", &t)?;
        lhs
      }

      BraceL => {
        let exprs = self.collect_listing(
          |p| p.parse_expr(0),
          TokenKind::Comma,
          TokenKind::BraceR,
          "unclosed array literal")?;

        self.push_expr(Expr::Literal(ExprLiteral::Array(exprs)))
      }

      _ => return Result::Err(self.err("invalid lhs expression", &t)),
    };

    while self.cursor.has_some() {
      let op = self.cursor.peek();
      if !op.kind.is_op() { break }
      let id = self.cursor.curr_id();

      if let Some(postfix_lvl) = postfix_lvl(op.kind) {
        if postfix_lvl < prec_lvl { break }
        self.cursor.advance();

        lhs = match op.kind {
          TokenKind::ParenL => todo!("function call"),
          TokenKind::BraceL => todo!("array indexing"),
          TokenKind::Dot => todo!("member access"),
          _ => return Result::Err(self.err("invalid postfix expression", &op)),
        };

        continue;
      }

      let (left_lvl, right_lvl) = infix_lvl(op.kind);
      if left_lvl < prec_lvl { break }
      self.cursor.advance();

      let rhs = self.parse_expr(right_lvl)?;
      lhs = self.push_expr(Expr::Binary { op: id, lhs, rhs });
    }

    Ok(lhs)
  }

  fn parse_assign(&mut self, lhs: ExprId) -> ParseResult<StmtId> {
    // eat '='
    self.cursor.advance();
    let rhs = self.parse_expr(0)?;

    let stmt = self.push_stmt(Stmt::Assign { lhs, rhs });
    Ok(stmt)
  }

  fn parse_decl(&mut self, name: TokenId, constant: bool) -> ParseResult<StmtId> {
    // eat ':'
    self.cursor.advance();

    let ty_id = if self.cursor.peek().kind == TokenKind::Assign {
      self.cursor.advance();
      ty_id::UNTYPED
    } else {
      let id = self.parse_type()?;

      // eat '='
      self.cursor.eat_match(TokenKind::Assign, "expect '=' after type annotation")?;
      id
    };

    let rhs = self.parse_expr(0)?;

    let stmt = self.push_stmt(Stmt::Decl { name, ty: ty_id, rhs, constant });
    Ok(stmt)
  }

  fn parse_const(&mut self) -> ParseResult<StmtId> {
    // eat 'const'
    self.cursor.advance();

    let name = self.cursor.peek();
    if name.kind != TokenKind::Ident {
      return Err(self.err("expected identifier after 'const' keyword", &name))
    }

    // eat ident
    let id = self.cursor.curr_id();
    self.cursor.advance();
    
    self.parse_decl(id, true)
  }

  fn parse_block(&mut self) -> ParseResult<StmtId> {
    // eat '{'
    self.cursor.advance();
    
    let mut stmts = Vec::new();
    while self.cursor.has_some() {
      if self.cursor.peek().kind == TokenKind::CurlyR {
        self.cursor.advance();
        return Ok(self.push_stmt(Stmt::Block { stmts }))
      }

      let id = self.parse_stmt()?;
      stmts.push(id);
    }

    return Err(self.err("unclosed block", &self.cursor.lexer.eof()))
  }

  fn parse_ifelse(&mut self) -> ParseResult<StmtId> {
    self.cursor.advance();
    let cond = self.parse_expr(0)?;
    let iblock = self.parse_block()?;

    let eblock = if self.cursor.eat_if(TokenKind::Keyword(KeywordKind::Else)).is_some() {
      Some(self.parse_block()?)
    } else {
      None
    };

    Ok(self.push_stmt(Stmt::IfElse { cond, iblock, eblock }))
  }

  fn parse_while(&mut self) -> ParseResult<StmtId> {
    self.cursor.advance();
    let cond = self.parse_expr(0)?;
    let wblock = self.parse_block()?;

    Ok(self.push_stmt(Stmt::While { cond, wblock }))
  }

  fn parse_func(&mut self) -> ParseResult<StmtId> {
    self.cursor.advance();

    self.cursor.eat_match(TokenKind::Ident, "expect name after 'fn' keyword")?;
    let name = self.cursor.prev_id();

    self.cursor.eat_match(TokenKind::ParenL, "expect '(' after function name")?;
    
    // let mut param_names = Vec::new();
    // let mut param_types = Vec::new();
    // while self.cursor.has_some() {
    //   if self.cursor.eat_if(TokenKind::ParenR).is_some() { break; }

    //   self.cursor.eat_match(TokenKind::Ident, "expect param name in function signature")?;
    //   param_names.push(self.cursor.prev_id());

    //   self.cursor.eat_match(TokenKind::Colon, "expect ':' after param name")?;
    //   param_types.push(self.parse_type()?);

    //   // let t = self.cursor.peek();
    //   // if t.kind != TokenKind::Comma {
    //   //   // if we don't find a comma, we are expecting a paren closing
    //   //   // if we don't get a paren closing, it is an error
    //   //   self.cursor.eat_match(TokenKind::ParenR, "expect ')' after function parameters")?;
    //   // } else {
    //   //   self.cursor.advance();
    //   // }

    //   if self.cursor.eat_if(TokenKind::Comma).is_none() {
    //     // if we don't find a comma, we are expecting a paren closing
    //     // if we don't get a paren closing, it is an error
    //     self.cursor.eat_match(TokenKind::ParenR, "expect ')' after function parameters")?;
    //   }
    // }

    let params = self.collect_listing(
      |p| {
        p.cursor.eat_match(TokenKind::Ident, "expect param name in function signature")?;
        let name = p.cursor.prev_id();

        p.cursor.eat_match(TokenKind::Colon, "expect ':' after param name")?;
        let ty = p.parse_type()?;

        Ok((name, ty))
      },
      TokenKind::Comma,
      TokenKind::ParenR,
      "unclosed parenthesis in function declaration's parameters")?;

    let ret = if self.cursor.eat_if(TokenKind::Arrow).is_some() {
      self.parse_type()?
    } else {
      ty_id::VOID
    };

    let block = self.parse_block()?;

    let (param_names, param_types) = params.into_iter().unzip();
    let ty = Type::Func { params: param_types, ret };
    let ty_id = self.push_type(ty);

    Ok(self.push_stmt(Stmt::FnDecl { name, param_names, ty: ty_id, block }))
  }

  fn parse_struct(&mut self) -> ParseResult<StmtId> {
    self.cursor.advance();

    let name = self.cursor.eat_match(TokenKind::Ident, "expect struct name after 'struct' keyword")?;
    let name_id = self.cursor.prev_id();
    self.cursor.eat_match(TokenKind::CurlyL, "expect '{' after struct name")?;
  
    let fields = self.collect_listing(
      |p| {
        let name = p.cursor.eat_match(TokenKind::Ident, "expect field name in struct declaration")?;
        let id = p.cursor.prev_id();
        p.cursor.eat_match(TokenKind::Colon, "expect ':' after name in struct declaration")?;
        let ty = p.parse_type()?;

        Ok((id, ty))
      },
      TokenKind::Comma,
      TokenKind::CurlyR,
      "expect '}' after struct fields")?;

    let ty = self.push_type(Type::Struct { name: name_id, fields });

    Ok(self.push_stmt(Stmt::StructDecl { ty }))
  }

  pub fn parse_stmt(&mut self) -> ParseResult<StmtId> {
    let t = self.cursor.peek();

    let s = match t.kind {
      TokenKind::Ident => {
        // we peeked an ident. this either means lvalue, or rvalue expression
        let tok_id = self.cursor.curr_id();
        let lhs = self.parse_expr(0)?;

        let op = self.cursor.peek();
        match op.kind {
          TokenKind::Assign => self.parse_assign(lhs)?,
          TokenKind::Colon => self.parse_decl(tok_id, false)?,
          _ => {
            // expression
            self.push_stmt(Stmt::Expr(lhs))
          }
        }
      }

      TokenKind::CurlyL => self.parse_block()?,

      TokenKind::Keyword(k) => match k {
        KeywordKind::Const => self.parse_const()?,
        KeywordKind::If => self.parse_ifelse()?,
        KeywordKind::While => self.parse_while()?,
        KeywordKind::Fn => self.parse_func()?,
        KeywordKind::Return => {
          self.cursor.advance();
          let expr = self.parse_expr(0)?;
          self.push_stmt(Stmt::Return { expr })
        }
        KeywordKind::Struct => self.parse_struct()?,
        _ => return Err(self.err("invalid keyword", &t)),
      }

      _ => {
        // expression
        let id = self.parse_expr(0)?;
        self.push_stmt(Stmt::Expr(id))
      }
    };

    if let Some(_) = self.cursor.eat_if(TokenKind::Semicolon) {}

    Ok(s)
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


pub fn parse(src: &str) -> crate::ast::Ast {
  let lexer = tokenize(src);
  let mut p = Parser::new(&lexer);

  let mut top_lvl = Vec::new();
  while p.cursor.has_some() {
    let stmt = p.parse_stmt();

    match stmt {
      Ok(id) => top_lvl.push(id),
      Err(e) => {
        eprintln!("[PARSE ERR] {e}");
        p.errors.push(e);
        p.cursor.eat_until_safe();
      } 
    }
  }

  p.stmts.push(Stmt::Block { stmts: top_lvl });

  crate::ast::Ast {
    exprs: p.exprs,
    stmts: p.stmts,
    types: p.types,
    lexer,
  }
}

impl<'a> Cursor<'a> {
  fn eat_if(&mut self, kind: TokenKind) -> Option<Token> {
    (self.peek().kind == kind).then(|| self.eat())
  }

  fn eat_match_ex<S: Into<String>>(&mut self, target: TokenKind, msg: S, err_tok: &Token) -> ParseResult<Token> {
    let t = self.eat();
    if t.kind != target {
      Err(err_tok.to_err(msg, self.lexer))
    }
    else { Ok(t) }
  }

  fn eat_match<S: Into<String>>(&mut self, target: TokenKind, msg: S) -> ParseResult<Token> {
    self.eat_match_ex(target, msg, &self.peek())
  }

  fn eat_until_safe(&mut self) {
    while self.has_some() {
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