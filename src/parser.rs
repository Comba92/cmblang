use std::{collections::HashSet, hash::Hash};

use crate::{CursorIter, FrontendErrAlias, IdSize, ast::{Ast, IdentId, StringInterner}, lexer::{self, KeywordKind, Lexer, Span, Token, TokenKind}}; 

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenId(pub IdSize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExprId(pub IdSize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StmtId(pub IdSize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyAnnotId(pub IdSize);

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
  Member { lhs: ExprId, field: IdentId },
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
  pub annot: Option<TyAnnotId>,
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
  // we can split this into two enums
  FnDecl { name: IdentId, params: Vec<(IdentId, TyAnnotId)>, ret: Option<TyAnnotId>, block: StmtId, is_generic: bool },
  StructDecl { name: IdentId, fields: Vec<(IdentId, TyAnnotId)>, generics: Vec<IdentId> },
}

#[derive(Debug)]
pub enum TyAnnot {
  Bool,
  Int,
  Float,
  Generic(IdentId),
  Array { inner: TyAnnotId, len: Option<ExprId> },
  Func { params: Vec<TyAnnotId>, ret: Option<TyAnnotId> },
  UserDef { name: IdentId, generics: Vec<TyAnnotId> }
}

type ParseResult<T> = Result<T, FrontendErrAlias>;
pub type Spanned<T> = (T, Span);

struct Parser<'a> {
  cursor: Cursor<'a>,

  exprs: Vec<Spanned<Expr>>,
  stmts: Vec<Spanned<Stmt>>,
  toplvl: Vec<Spanned<StmtTopLvl>>,
  annots: Vec<Spanned<TyAnnot>>,
  idents: StringInterner,
}

impl<'a> Parser<'a> {
  pub fn new(lexer: &'a Lexer<'a>) -> Self {
    Self {
      cursor: Cursor { lexer, curr: 0 },
      exprs: vec![],
      stmts: vec![],
      toplvl: vec![],
      annots: Default::default(),
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

  fn expect_ex<S: Into<String>>(&mut self, target: TokenKind, msg: S, err_tok: Token) -> ParseResult<Token> {
    let t = self.cursor.eat();
    if t.kind != target {
      Err(self.err(msg, err_tok))
    }
    else { Ok(t) }
  }

  fn expect<S: Into<String>>(&mut self, target: TokenKind, msg: S) -> ParseResult<Token> {
    self.expect_ex(target, msg, self.cursor.peek())
  }

  fn expect_ident<S: Into<String>>(&mut self, msg: S) -> ParseResult<IdentId> {
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
  
  fn push_ident(&mut self, tok: Token) -> IdentId {
    self.idents.intern(tok.span.get_str(self.cursor.lexer.src))
  }

  fn push_annot(&mut self, ann: TyAnnot, span: Span) -> TyAnnotId {
    self.annots.push((ann, span));
    TyAnnotId(self.annots.len() as IdSize - 1)
  }

  fn collect_listing<T, F>(&mut self, getter: F, terminator: TokenKind, err_unclosed: &str) -> ParseResult<Vec<T>>
    where
      F: Fn(&mut Self) -> ParseResult<T>,
  {
    let mut list = Vec::new();
    loop {
      if !self.cursor.has_some() {
        return Err(self.err(err_unclosed, self.cursor.lexer.eof()))
      }
      if self.cursor.eat_if(terminator).is_some() { break }

      let item = getter(self)?;
      list.push(item);

      if self.cursor.eat_if(TokenKind::Comma).is_none() {
        // if we don't find a comma, we are expecting a paren closing
        // if we don't get a paren closing, it is an error
        if self.cursor.eat_if(terminator).is_some() { break }
      }
    }

    Ok(list)
  }

  fn collect_listing_unique<T, F, E1, E2>(&mut self, getter: F, terminator: TokenKind, err_unclosed: E1, err_duplicates: E2) -> ParseResult<(Vec<T>, HashSet<T>)> 
    where
      T: Eq + Hash + Clone,
      F: Fn(&mut Self) -> ParseResult<T>,
      E1: Into<&'a str>,
      E2: Into<String>,
  {
    let mut list = Vec::new();
    let mut found = HashSet::new();
    loop {
      if !self.cursor.has_some() {
        return Err(self.err(err_unclosed.into(), self.cursor.lexer.eof()))
      }
      if self.cursor.eat_if(terminator).is_some() { break }

      let tok = self.cursor.peek();
      let item = getter(self)?;

      if !found.insert(item.clone()) {
        return Err(self.err(err_duplicates, tok))
      }

      list.push(item);

      if self.cursor.eat_if(TokenKind::Comma).is_none() {
        // if we don't find a comma, we are expecting a paren closing
        // if we don't get a paren closing, it is an error
        if self.cursor.eat_if(terminator).is_some() { break }
      }
    }

    Ok((list, found))
  }

  fn parse_expr(&mut self, prec_lvl: i8) -> ParseResult<ExprId> {
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
        // array literal
        let exprs = self.collect_listing(
          |p| p.parse_expr(0),
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
              TokenKind::ParenR,
              "unclosed function call")?;
            
            self.push_expr(Expr::Call { callee: lhs, args }, op.span)
          },
          TokenKind::Dot => {
            let rhs = self.expect_ident("expect member idientifier after . operation")?;
            self.push_expr(Expr::Member { lhs, field: rhs }, op.span)
          },
          TokenKind::BraceL => {
            let rhs = self.parse_expr(0)?;
            self.expect(TokenKind::BraceR, "unclosed array indexing bracket")?;
            self.push_expr(Expr::Index { lhs, idx: rhs }, op.span)
          }
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

  fn parse_annot(&mut self) -> ParseResult<TyAnnotId> {
    let t = self.cursor.eat();

    let annot = match t.kind {
      TokenKind::Keyword(k) => match k {
        KeywordKind::Bool => TyAnnot::Bool,
        KeywordKind::Int => TyAnnot::Int,
        KeywordKind::Float => TyAnnot::Float,
        _ => return Err(self.err("invalid type annotation", t)),
      }

      // function
      TokenKind::ParenL => {
        let params = self.collect_listing(
          Self::parse_annot,
          TokenKind::ParenR,
          "unclosed parenthesis in function annotation's params")?;
 
        let ret = if self.cursor.eat_if(TokenKind::Arrow).is_some() {
          Some(self.parse_annot()?)
        } else { None };

        TyAnnot::Func { params, ret }
      }

      // array
      TokenKind::BraceL => {
        let inner = self.parse_annot()?;
        self.expect(TokenKind::Colon, "expect ':' after array inner type")?;
        
        // TODO: might be cool if this can be a constant integer expression?
        let len_tok = self.cursor.peek();

        let len = match len_tok.kind {
          // size will be inferred later
          TokenKind::Star => {
            self.cursor.advance();
            None
          }
          // TokenKind::IntLit => self.cursor.lexer.get_str(len_tok)
          //   .parse()
          //   .map_err(|e| self.err(format!("impossible to parse integer literal: {e}"), len_tok))?,

          // _ => return Err(self.err("expect integer literal or '*' (inferred size) for size in array type annotation", len_tok))
          // TODO: this should be const
          _ => Some(self.parse_expr(0)?)
        };

        self.expect(TokenKind::BraceR, "expect closing ']' in array type annotation")?;

        TyAnnot::Array { inner, len }
      }

      TokenKind::Ident => {
        // this only adds a user defined dummy type to the environment
        // later the typechecker will update this type to the correct one

        let name = self.push_ident(t);

        let generics = if self.cursor.eat_if(TokenKind::Less).is_some() {
          // generics
          self.collect_listing(
            Self::parse_annot,
            TokenKind::Great,
          "unclosed generics listing")?
        } else {
          Vec::new()
        };

        TyAnnot::UserDef { name, generics }
      },

      _ => return Err(self.err("invalid type annotation", t)),
    };

    Ok(self.push_annot(annot, t.span))
  }

  fn parse_decl(&mut self, name: Token, constant: bool) -> ParseResult<(Decl, Span)> {
    let ident = self.push_ident(name.clone());

    let t = self.cursor.peek();
    let annot = if t.kind == TokenKind::Assign {
      self.cursor.advance();
      None
    } else if self.cursor.peek().kind == TokenKind::Colon {
      self.cursor.advance();
      let id = self.parse_annot()?;

      // eat '='
      self.expect(TokenKind::Assign, "expect '=' after type annotation")?;
      Some(id)
    } else {
      return Err(self.err("Expect ':' or '=' after declaration name", name))
    };
    
    let rhs = self.parse_expr(0)?;
    let decl = Decl { ident, annot, rhs, constant };
    Ok((decl, name.span))
  }
  
  fn parse_let_or_const(&mut self, constant: bool, toplvl: bool) -> ParseResult<StmtId> {
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

  fn parse_assign(&mut self, lhs: ExprId) -> ParseResult<StmtId> {
    // eat '='
    let t = self.cursor.eat();
    let rhs = self.parse_expr(0)?;

    let stmt = self.push_stmt(Stmt::Assign { lhs, rhs }, t.span);
    Ok(stmt)
  }

  // TODO: this can probably be optimized by doing something similiar to parse_block_with_generics() function
  fn resolve_generics(&mut self, id: TyAnnotId, generics_set: &HashSet<IdentId>) -> ParseResult<()> {
    let (annot, span) = &mut self.annots[id.0 as usize];

    match annot {
      TyAnnot::UserDef { name, generics } => {
          if generics_set.contains(name) {
            // it is a basic generic

            if !generics.is_empty() {
              return Err(FrontendErrAlias::new(
                &self.cursor.lexer, 
                "generic type can't have a generics list",
                *span
              ))
            }

            // edit the annotation from UserDef to Generic
            *annot = TyAnnot::Generic(*name);
          } else {
            // it is a userdef with some generic listing
            // TODO: can we do something about cloning here?
            for generic in generics.clone() {
              self.resolve_generics(generic, generics_set)?;
            }
          }
        }

      TyAnnot::Generic(_) => unreachable!("shouldn't find generics during function generic resolution"),

      // not a generic
      TyAnnot::Bool | TyAnnot::Int | TyAnnot::Float => {},

      TyAnnot::Array { inner, .. } => {
        let inner = *inner;
        self.resolve_generics(inner, generics_set)?;
      },

      TyAnnot::Func { params, ret } => {
        // TODO: can we do something about cloning here?
        let params = params.clone();
        let ret = *ret;

        for param in params {
          self.resolve_generics(param, generics_set)?;
        }

        if let Some(ret) = ret {
          self.resolve_generics(ret, generics_set)?;
        }
      },
    }

    Ok(())
  }

  fn parse_func(&mut self) -> ParseResult<StmtId> {
    // eat 'fn'
    let t = self.cursor.eat();

    let ident = self.expect_ident("expect name after 'fn' keyword")?;

    let generics = if self.cursor.eat_if(TokenKind::Less).is_some() {
      self.collect_listing_unique(
        |p| p.expect_ident("expected generic identifier"),
        TokenKind::Great,
        "unclosed generics listing",
        "repeated function generic parameter name"
      )?.1
    } else {
      HashSet::new()
    };

    self.expect(TokenKind::ParenL, "expect '(' after function name")?;

    let params = self.collect_listing_unique(
      |p| {
        let ident = p.expect_ident("expect param name in function signature")?;
        p.expect(TokenKind::Colon, "expect ':' after param name")?;
        let ty = p.parse_annot()?;

        Ok((ident, ty))
      },
      TokenKind::ParenR,
      "unclosed parenthesis in function declaration's parameters",
      "repeated function parameter name"
    )?.0;

    let ret = if self.cursor.eat_if(TokenKind::Arrow).is_some() {
      Some(self.parse_annot()?)
    } else {
      None
    };

    // resolve generics
    let is_generic = if !generics.is_empty() {
      for param in &params {
        self.resolve_generics(param.1, &generics)?;
      }

      if let Some(ret) = &ret {
        self.resolve_generics(*ret, &generics)?;
      }
      true
    } else {
      false
    };

    // let block = self.parse_block()?;
    let block = self.parse_block_with_generics(&generics)?;

    Ok(self.push_toplvl(StmtTopLvl::FnDecl { name: ident, params, ret, block, is_generic }, t.span))
  }

  fn parse_struct(&mut self) -> ParseResult<StmtId> {
    // eat 'struct'
    let t = self.cursor.eat();

    let name = self.expect_ident("expect struct name after 'struct' keyword")?;

    let generics = if self.cursor.eat_if(TokenKind::Less).is_some() {
      self.collect_listing_unique(
        |p| p.expect_ident("expect generic argument name"),
        TokenKind::Great,
        "unclosed generic listing",
        "repeated generic parameter"
      )?
    } else {
      (Vec::new(), HashSet::new())
    };

    self.expect(TokenKind::CurlyL, "expect '{' after struct name")?;

    let fields = self.collect_listing_unique(
      |p| {
        let ident = p.expect_ident("expect field name in struct declaration")?;

        p.expect(TokenKind::Colon, "expect ':' after name in struct declaration")?;
        let ty = p.parse_annot()?;

        Ok((ident, ty))
      },
      TokenKind::CurlyR,
      "expect '}' after struct fields",
      "repeated struct field name",
    )?.0;

    if !generics.1.is_empty() {
      for (_, field_ty) in &fields {
        self.resolve_generics(*field_ty, &generics.1)?;
      }
    }

    Ok(self.push_toplvl(StmtTopLvl::StructDecl { name, fields, generics: generics.0 }, t.span))
  }

  fn parse_block(&mut self) -> ParseResult<StmtId> {
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

  fn parse_block_with_generics(&mut self, generics_set: &HashSet<IdentId>) -> ParseResult<StmtId> {
    // this is hacky: we store the last type we've pushed;
    // parse the block normally;
    // then check every newly added type and if it is an userdef,
    // we check if it is a generic and eventually convert it

    let top = self.annots.len();
    let block = self.parse_block()?;

    for id in top..self.annots.len() {
      let (annot, span) = &mut self.annots[id];

      match annot {
        TyAnnot::UserDef { name, generics } => {
          if generics_set.contains(name) {
            // it is a basic generic

            if !generics.is_empty() {
              return Err(FrontendErrAlias::new(
                &self.cursor.lexer, 
                "generic type can't have a generics list",
                *span
              ))
            }

            // edit the annotation from UserDef to Generic
            *annot = TyAnnot::Generic(*name);
          }
        }

        _ => {}
      }
    }

    Ok(block)
  }

  fn parse_ifelse(&mut self) -> ParseResult<StmtId> {
    todo!()
  }

  fn parse_while(&mut self) -> ParseResult<StmtId> {
    todo!()
  }

  fn parse_stmt(&mut self) -> ParseResult<StmtId> {
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

  fn parse_toplvl(&mut self) -> ParseResult<StmtId> {
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

pub fn parse(src: &str) -> ParseResult<Ast> {
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
    annots: parser.annots,
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