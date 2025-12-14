use std::collections::{HashMap, HashSet};

use crate::{CursorIter, FrontendErrAlias, IdSize, ast::{Ast, IdentId, StringInterner, Type, TypeEnv, TypeId, ty_id}, lexer::{self, KeywordKind, Lexer, Span, Token, TokenKind}}; 

#[derive(Debug, Clone, Copy)]
pub struct TokenId(pub IdSize);
#[derive(Debug, Clone, Copy)]
pub struct ExprId(pub IdSize);
#[derive(Debug, Clone, Copy)]
pub struct StmtId(pub IdSize);

#[derive(Debug)]
pub enum ExprLiteral {
  Int(TokenId),
  Float(TokenId),
  Bool(TokenId),
  Array(Vec<ExprId>),
  // TODO: rn members should be provided in order
  Struct(IdentId, Vec<ExprId>)
}

#[derive(Debug)]
pub enum Expr {
  Literal(ExprLiteral),
  Variable(IdentId),
  Unary { op: TokenId, rhs: ExprId },
  Binary { op: TokenId, lhs: ExprId, rhs: ExprId },
  Call { callee: ExprId, args: Vec<ExprId> },
  Member { lhs: ExprId, field: TokenId },
  Index { lhs: ExprId, idx: ExprId },
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
pub struct Decl {
  pub ident: IdentId,
  pub annot: TypeId,
  pub rhs: ExprId,
  pub constant: bool
}

#[derive(Debug)]
pub enum Stmt {
  Decl(Decl),
  Assign { lhs: ExprId, rhs: ExprId },
  Block(Vec<StmtId>),
  IfElse { cond: ExprId, iblock: StmtId, eblock: Option<StmtId> },
  While { cond: ExprId, wblock: StmtId },
  Return(ExprId),
  Expr(ExprId),
}

// TODO: might be better for error checking to keep these the same enum as Stmt
#[derive(Debug)]
pub enum StmtTopLvl {
  Decl(Decl),
  // TODO: generics should be set, params a map
  FnDecl { name: IdentId, generics: Vec<IdentId>, params: Vec<(IdentId, TypeId)>, ret: TypeId, block: StmtId },
  // TODO: fields should be a map
  StructDecl { name: IdentId, fields: Vec<(IdentId, TypeId)> },
}

type ParserResult<T> = Result<T, FrontendErrAlias>;
pub type Spanned<T> = (T, Span);

struct Parser<'a> {
  cursor: Cursor<'a>,

  exprs: Vec<Spanned<Expr>>,
  stmts: Vec<Spanned<Stmt>>,
  toplvl: Vec<Spanned<StmtTopLvl>>,
  types: TypeEnv,
  idents: StringInterner,
}

impl<'a> Parser<'a> {
  pub fn new(lexer: &'a Lexer<'a>) -> Self {
    Self {
      cursor: Cursor { lexer, curr: 0 },
      exprs: vec![],
      stmts: vec![],
      toplvl: vec![],
      types: Default::default(),
      idents: Default::default(),
    }
  }

  pub fn err<S: Into<String>>(&self, msg: S, tok: Token) -> FrontendErrAlias {
    FrontendErrAlias::new(
      &self.cursor.lexer, 
      msg,
      tok.span
    )
  }

  fn expect_ex<S: Into<String>>(&mut self, target: TokenKind, msg: S, err_tok: Token) -> ParserResult<Token> {
    let t = self.cursor.eat();
    if t.kind != target {
      Err(self.err(msg, err_tok))
    }
    else { Ok(t) }
  }

  fn expect<S: Into<String>>(&mut self, target: TokenKind, msg: S) -> ParserResult<Token> {
    self.expect_ex(target, msg, self.cursor.peek())
  }

  fn expect_ident<S: Into<String>>(&mut self, msg: S) -> ParserResult<IdentId> {
    let name = self.expect(TokenKind::Ident, msg)?;
    let id = self.idents.intern(name.span.get_str(self.cursor.lexer.src));
    Ok(id)
  }

  fn push_expr(&mut self, e: Expr, span: Span) -> ExprId {
    self.exprs.push((e, span));
    ExprId(self.exprs.len() as IdSize - 1)
  }

  fn push_stmt(&mut self, s: Stmt, span: Span) -> StmtId {
    self.stmts.push((s, span));
    StmtId(self.stmts.len() as IdSize - 1)
  }

  fn push_toplvl(&mut self, s: StmtTopLvl, span: Span) -> StmtId {
    self.toplvl.push((s, span));
    StmtId(self.toplvl.len() as IdSize - 1)
  }
  
  pub fn push_ident(&mut self, tok: Token) -> IdentId {
    self.idents.intern(tok.span.get_str(self.cursor.lexer.src))
  }

  fn collect_listing<T, F>(&mut self, getter: F, separator: TokenKind, terminator: TokenKind, err_unclosed: &str) -> ParserResult<Vec<T>>
    where
      F: Fn(&mut Self) -> ParserResult<T>,
  {
    let mut list = Vec::new();
    loop {
      if !self.cursor.has_some() {
        return Err(self.err(err_unclosed, self.cursor.lexer.eof()))
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

  fn parse_expr(&mut self, prec_lvl: i8) -> ParserResult<ExprId> {
    let tok_id = self.cursor.curr_id();
    let t = self.cursor.eat();

    use TokenKind::*;
    let mut lhs = match t.kind {
      IntLit   => self.push_expr(Expr::Literal(ExprLiteral::Int(tok_id)), t.span),
      FloatLit => self.push_expr(Expr::Literal(ExprLiteral::Float(tok_id)), t.span),
      Keyword(KeywordKind::True)  => self.push_expr(Expr::Literal(ExprLiteral::Bool(tok_id)), t.span),
      Keyword(KeywordKind::False) => self.push_expr(Expr::Literal(ExprLiteral::Bool(tok_id)), t.span),

      Ident => {
        let ident = self.push_ident(t);

        if self.cursor.eat_if(TokenKind::CurlyL).is_some() {
          // struct literal

          // TODO: this should be more complex than this
          // rn we only expect the members in order
          let members = self.collect_listing(
            |p| p.parse_expr(0),
            TokenKind::Comma,
            TokenKind::CurlyR,
          "unclosed struct literal")?;
          
          self.push_expr(Expr::Literal(ExprLiteral::Struct(ident, members)), t.span)
        } else {
          self.push_expr(Expr::Variable(ident), t.span)
        }
      }

      // prefix op
      Minus | Bang => {
        let lvl = prefix_lvl(t.kind);
        let rhs = self.parse_expr(lvl)?;
        self.push_expr(Expr::Unary { op: tok_id, rhs }, t.span)
      }

      ParenL => {
        let lhs = self.parse_expr(0)?;
        self.expect_ex(TokenKind::ParenR, "unclosed parenthesis", t)?;
        lhs
      }

      BraceL => {
        let exprs = self.collect_listing(
          |p| p.parse_expr(0),
          TokenKind::Comma,
          TokenKind::BraceR,
          "unclosed array literal")?;

        self.push_expr(Expr::Literal(ExprLiteral::Array(exprs)), t.span)
      }

      // CurlyL => todo!("parse anonymous struct literal"),

      _ => return Result::Err(self.err("invalid lhs expression", t)),
    };

    while self.cursor.has_some() {
      let op: Token = self.cursor.peek();
      if !op.kind.is_op() { break }
      let id = self.cursor.curr_id();

      if let Some(postfix_lvl) = postfix_lvl(op.kind) {
        if postfix_lvl < prec_lvl { break }
        self.cursor.advance();

        lhs = match op.kind {
          TokenKind::ParenL => {
            let args = self.collect_listing(
              |p| p.parse_expr(0),
              TokenKind::Comma, 
              TokenKind::ParenR,
              "unclosed function call")?;
            
            self.push_expr(Expr::Call { callee: lhs, args }, op.span)
          },
          TokenKind::Dot => todo!("member access"),
          TokenKind::BraceL => todo!("array indexing"),
          _ => return Result::Err(self.err("invalid postfix expression", t)),
        };

        continue;
      }

      let (left_lvl, right_lvl) = infix_lvl(op.kind);
      if left_lvl < prec_lvl { break }
      self.cursor.advance();

      let rhs = self.parse_expr(right_lvl)?;
      lhs = self.push_expr(Expr::Binary { op: id, lhs, rhs }, t.span);
    }

    Ok(lhs)
  }

  fn parse_annot(&mut self) -> ParserResult<TypeId> {
    let t = self.cursor.eat();

    let ty = match t.kind {
      TokenKind::Keyword(k) => match k {
        KeywordKind::Bool => ty_id::BOOL,
        KeywordKind::Int => ty_id::INT,
        KeywordKind::Float => ty_id::FLOAT,
        _ => return Err(self.err("invalid type annotation", t)),
      }

      TokenKind::ParenL => {
        let params = self.collect_listing(
          Self::parse_annot,
          TokenKind::Comma,
          TokenKind::ParenR,
          "unclosed parenthesis in function annotation's params")?;
 
        let ret = if self.cursor.eat_if(TokenKind::Arrow).is_some() {
          self.parse_annot()?
        } else { ty_id::VOID };

        self.types.add_ty(Type::Func { params, ret })
      }

      TokenKind::BraceL => {
        let inner = self.parse_annot()?;
        self.expect(TokenKind::Colon, "expect ':' after array inner type")?;
        
        // TODO: might be cool if this can be a constant integer expression?
        let len_tok = self.cursor.eat();

        let len = match len_tok.kind {
          // size will be inferred later
          TokenKind::Star => 0,
          TokenKind::IntLit => self.cursor.lexer.get_str(len_tok)
            .parse()
            .map_err(|e| self.err(format!("impossible to parse integer literal: {e}"), len_tok))?,

          _ => return Err(self.err("expect integer literal or '*' (inferred size) for size in array type annotation", len_tok)) 
        };

        self.expect(TokenKind::BraceR, "expect closing ']' in array type annotation")?;

        self.types.add_ty(Type::Array { inner, len })
      }

      TokenKind::Ident => {
        // this only adds a user defined dummy type to the environment
        // later the typechecker will update this type to the correct one

        let name = self.push_ident(t);
        self.types.add_userdef(name, Type::UserDef(name)).0
      },

      _ => return Err(self.err("invalid type annotation", t)),
    };

    Ok(ty)
  }

  fn parse_decl(&mut self, name: Token, constant: bool) -> ParserResult<(Decl, Span)> {
    let ident = self.push_ident(name.clone());

    // // eat ':'
    // self.cursor.advance();

    let ty_id = if self.cursor.peek().kind == TokenKind::Assign {
      self.cursor.advance();
      ty_id::UNTYPED
    } else if self.cursor.peek().kind == TokenKind::Colon {
      self.cursor.advance();
      let id = self.parse_annot()?;

      // eat '='
      self.expect(TokenKind::Assign, "expect '=' after type annotation")?;
      id
    } else {
      return Err(self.err("Expect ':' or '=' after declaration name", name))
    };
    
    let rhs = self.parse_expr(0)?;
    let decl = Decl { ident, annot: ty_id, rhs, constant };
    Ok((decl, name.span))
  }
  
  fn parse_let_or_const(&mut self, constant: bool, toplvl: bool) -> ParserResult<StmtId> {
    // eat 'let' or 'const'
    let t = self.cursor.eat();

    let name = self.cursor.peek();
    if name.kind != TokenKind::Ident {
      return Err(self.err("expected identifier after 'const' keyword", t))
    }

    // eat ident
    self.cursor.advance();

    let (decl, span) = self.parse_decl(name, constant)?;
    let id = if toplvl {
      self.push_toplvl(StmtTopLvl::Decl(decl), span)
    } else {
      self.push_stmt(Stmt::Decl(decl), span)
    };

    Ok(id)
  }

  fn parse_assign(&mut self, lhs: ExprId) -> ParserResult<StmtId> {
    // eat '='
    let t = self.cursor.eat();
    let rhs = self.parse_expr(0)?;

    let stmt = self.push_stmt(Stmt::Assign { lhs, rhs }, t.span);
    Ok(stmt)
  }

  fn parse_func(&mut self) -> ParserResult<StmtId> {
    // eat 'fn'
    let t = self.cursor.eat();

    let ident = self.expect_ident("expect name after 'fn' keyword")?;

    let generics = if self.cursor.eat_if(TokenKind::Less).is_some() {
      self.collect_listing(
        |p| p.expect_ident("expected generic identifier"),
        TokenKind::Comma,
        TokenKind::Great,
        "unclosed generics listing")?
    } else {
      Vec::new()
    };

    self.expect(TokenKind::ParenL, "expect '(' after function name")?;

    let params = self.collect_listing(
      |p| {
        let ident = p.expect_ident("expect param name in function signature")?;
        p.expect(TokenKind::Colon, "expect ':' after param name")?;
        let ty = p.parse_annot()?;

        Ok((ident, ty))
      },
      TokenKind::Comma,
      TokenKind::ParenR,
      "unclosed parenthesis in function declaration's parameters")?;

    let ret = if self.cursor.eat_if(TokenKind::Arrow).is_some() {
      self.parse_annot()?
    } else {
      ty_id::VOID
    };

    let block = self.parse_block()?;

    Ok(self.push_toplvl(StmtTopLvl::FnDecl { name: ident, generics, params, ret, block }, t.span))
  }

  fn parse_struct(&mut self) -> ParserResult<StmtId> {
    // eat 'struct'
    let t = self.cursor.eat();

    let name = self.expect_ident("expect struct name after 'struct' keyword")?;
    self.expect(TokenKind::CurlyL, "expect '{' after struct name")?;
  
    let fields = self.collect_listing(
      |p| {
        let ident = p.expect_ident("expect field name in struct declaration")?;

        p.expect(TokenKind::Colon, "expect ':' after name in struct declaration")?;
        let ty = p.parse_annot()?;

        Ok((ident, ty))
      },
      TokenKind::Comma,
      TokenKind::CurlyR,
      "expect '}' after struct fields")?;

    // let ty = self.push_annot(Type::Struct { name: ident, fields }, t.span);

    Ok(self.push_toplvl(StmtTopLvl::StructDecl { name, fields }, t.span))
  }

  fn parse_block(&mut self) -> ParserResult<StmtId> {
    // eat '{'
    let t = self.cursor.eat();
    
    let mut stmts = Vec::new();
    while self.cursor.has_some() {
      if self.cursor.peek().kind == TokenKind::CurlyR {
        self.cursor.advance();
        return Ok(self.push_stmt(Stmt::Block(stmts), t.span))
      }

      let id = self.parse_stmt()?;
      stmts.push(id);
    }

    return Err(self.err("unclosed block", t))
  }

  fn parse_ifelse(&mut self) -> ParserResult<StmtId> {
    todo!()
  }

  fn parse_while(&mut self) -> ParserResult<StmtId> {
    todo!()
  }

  fn parse_stmt(&mut self) -> ParserResult<StmtId> {
    let t = self.cursor.peek();

    let stmt = match t.kind {
      TokenKind::Ident => {
        let lhs = self.parse_expr(0)?;
        let op = self.cursor.peek();
        match op.kind {
          TokenKind::Assign => self.parse_assign(lhs)?,
          _ => {
            // expression
            self.push_stmt(Stmt::Expr(lhs), t.span)
          }
        }
      }

      TokenKind::CurlyL => self.parse_block()?,

      TokenKind::Keyword(k) => match k {
        KeywordKind::Let => self.parse_let_or_const(false, false)?,
        KeywordKind::Const => self.parse_let_or_const(true, false)?,
        KeywordKind::If => self.parse_ifelse()?,
        KeywordKind::While => self.parse_while()?,
        KeywordKind::Return => {
          self.cursor.advance();
          let expr = self.parse_expr(0)?;
          self.push_stmt(Stmt::Return(expr), t.span)
        }
        _ => {
          self.cursor.advance();
          return Err(self.err("invalid keyword", t))
        }
      }

      _ => {
        let id = self.parse_expr(0)?;
        self.push_stmt(Stmt::Expr(id), t.span)
      }
    };

    if let Some(_) = self.cursor.eat_if(TokenKind::Semicolon) {}
    Ok(stmt)
  }

  fn parse_toplvl(&mut self) -> ParserResult<StmtId> {
    let t = self.cursor.peek();

    let stmt = match t.kind {
      TokenKind::Keyword(k) => match k {
        KeywordKind::Let => self.parse_let_or_const(false, true),
        KeywordKind::Const => self.parse_let_or_const(true, true),
        KeywordKind::Fn => self.parse_func(),
        KeywordKind::Struct => self.parse_struct(),
        _ => return Err(self.err("invalid keyword at top level", t))
      }

      _ => return Err(self.err("invalid token at top level", t)),
    };
    
    if let Some(_) = self.cursor.eat_if(TokenKind::Semicolon) {}

    Ok(stmt?)
  }
}

pub fn parse(src: &str) -> ParserResult<Ast> {
  let lexer = lexer::tokenize(src)?;
  let mut parser = Parser::new(&lexer);
  
  while parser.cursor.has_some() {
    let stmt = parser.parse_toplvl();

    if let Err(err) = stmt {
      eprintln!("[PARSE ERR] {err}");
      // parser.errors.push(e);
      parser.cursor.eat_until_safe(TokenKind::is_safe)
    }
  }

  Ok(Ast {
    exprs: parser.exprs,
    stmts: parser.stmts,
    toplvl: parser.toplvl,
    types: parser.types,
    idents: parser.idents,
    lexer,
  })
}

struct Cursor<'a> {
  lexer: &'a Lexer<'a>,
  curr: u32,
}
impl<'a> CursorIter<Token> for Cursor<'a> {
  fn peek_nth(&self, nth: usize) -> Token {
    self.lexer.tokens.get(self.curr() as usize + nth)
    .cloned()
    .unwrap_or_default()
  }

  fn start(&self) -> &[Token] { &self.lexer.tokens }
  fn curr(&self) -> u32 { self.curr }
  fn curr_mut(&mut self) -> &mut u32 { &mut self.curr }
}

impl<'a> Cursor<'a> {
  fn eat_if(&mut self, target: TokenKind) -> Option<Token> {
    (self.peek().kind == target).then(|| self.eat())
  }

  fn eat_until_safe<F: Fn(&TokenKind) -> bool>(&mut self, cond: F) {
    self.advance();
    while self.has_some() {
      if cond(&self.peek().kind) { break }
      self.advance();
    }
  }

  fn curr_id(&self) -> TokenId {
    TokenId(self.curr() as IdSize)
  }

  fn prev_id(&self) -> TokenId {
    TokenId(self.curr().saturating_sub(1) as IdSize)
  }
}