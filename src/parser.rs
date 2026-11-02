use crate::ParseError::MalformedExpression;
use crate::TokenKind::BraceL;
use crate::intern::StringInterner;
use crate::lexer::*;
use crate::*;

// OPT:
// https://jhwlr.io/super-flat-ast/
// https://thunderseethe.dev/posts/parser-base/

pub type Id = u32;
#[derive(Debug, Clone, Copy)]
pub struct ExprId(pub Id);
#[derive(Debug, Clone, Copy)]
pub struct StmtId(pub Id);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdentId(pub Id);
#[derive(Debug, Clone, Copy)]
pub struct TyAnnotId(pub Id);

pub type List<T> = Box<[T]>;

#[derive(Debug, Clone)]
// Statements indexes are [start, end)
pub struct Block {
    pub start: StmtId,
    pub end: StmtId,
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLiteral(u32),
    FloatLiteral(f32),
    BoolLiteral(bool),
    // ArrayLiteral(List<ExprId>),
    Variable(IdentId),
    // Assign { lhs: ExprId, rhs: ExprId },
    Return(ExprId),

    Unary {
        op: TokenKind,
        rhs: ExprId,
    },
    Binary {
        op: TokenKind,
        lhs: ExprId,
        rhs: ExprId,
    },
    Ternary {
        cond: ExprId,
        first: ExprId,
        second: ExprId,
    },

    Call {
        callee: ExprId,
        args: List<ExprId>,
    },
    Indexing {
        src: ExprId,
        idx: ExprId,
    },
    Member {
        src: ExprId,
        member: IdentId,
    },

    Block(Block),
}
impl Expr {
    fn is_assignable(&self) -> bool {
        use Expr::*;

        match self {
            Variable(_) | Indexing { .. } | Member { .. } => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub enum TyAnnot {
    Type {
        name: IdentId,
        args: List<TyAnnotId>,
    },
    Array {
        inner: TyAnnotId,
        size: ExprId,
    },
    Tuple(List<TyAnnotId>),
    Func {
        params: List<TyAnnotId>,
        ret: Option<TyAnnotId>,
    },
}

#[derive(Debug, Clone)]
pub struct VarContent {
    pub name: IdentId,
    pub ty: Option<TyAnnotId>,
    pub rhs: ExprId,
}

#[derive(Debug, Clone)]
pub struct FuncContent {
    pub name: IdentId,
    pub generics: List<IdentId>,
    pub params: List<(IdentId, TyAnnotId)>,
    pub ret: Option<TyAnnotId>,
    pub code: Block,
}

#[derive(Debug, Clone)]
pub struct StructContent {
    pub name: IdentId,
    pub generics: List<IdentId>,
    pub fields: List<(IdentId, TyAnnotId)>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl(VarContent),
    FuncDecl(FuncContent),
    StructDecl(StructContent),
    Expr(ExprId),
}

type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug)]
pub enum ListingKind {
    Block,
    TypeGenericsAnnot,
    FuncParamsAnnot,
    TupleAnnot,
    FuncCallArgs,
    StructFields,
}

#[derive(Debug)]
pub enum ParseError {
    WrongToken {
        pos: Position,
        expected: TokenKind,
        actual: TokenKind,
        at: String, // TODO: should be enum? or just a message?
    },

    InvalidStmtInTopLevel(Positioned<TokenKind>),
    InvalidStmtInBlock(Positioned<TokenKind>),

    IntParseError {
        inner: std::num::ParseIntError,
        pos: Position,
    },
    FloatParseError {
        inner: std::num::ParseFloatError,
        pos: Position,
    },

    UnclosedParenthesis {
        kind: TokenKind,
        start: Position,
    },

    UnclosedListing {
        start: Position,
        end: Position,
        what: ListingKind,
    },

    MalformedExpression(Positioned<TokenKind>),
    MalformedTypeAnnotation(Positioned<TokenKind>),

    ReachedEof,
}

#[derive(Default)]
pub struct Parser {
    pub errors: Vec<ParseError>,

    pub idents: StringInterner,
    pub exprs: Vec<Expr>,
    pub stmts: Vec<Stmt>,
    pub toplvl_stmts: Vec<StmtId>,
    pub types: Vec<TyAnnot>,
}

impl Parser {
    fn add_ident(&mut self, c: &ParseCursor, tok: &Token) -> IdentId {
        IdentId(self.idents.add(c.lex.token_string(tok)))
    }

    fn add_ty_annot(&mut self, ty: TyAnnot) -> TyAnnotId {
        let id = self.types.len();
        self.types.push(ty);
        TyAnnotId(id as Id)
    }

    fn add_stmt(&mut self, stmt: Stmt) -> StmtId {
        let id = self.stmts.len();
        self.stmts.push(stmt);
        StmtId(id as Id)
    }

    fn add_expr(&mut self, expr: Expr) -> ExprId {
        let id = self.exprs.len();
        self.exprs.push(expr);
        ExprId(id as Id)
    }

    pub fn get_expr(&self, expr: ExprId) -> &Expr {
        &self.exprs[expr.0 as usize]
    }

    pub fn get_stmt(&self, stmt: StmtId) -> &Stmt {
        &self.stmts[stmt.0 as usize]
    }

    pub fn get_ident(&self, ident: IdentId) -> &str {
        self.idents.get(ident.0)
    }

    fn report_error(&mut self, c: &mut ParseCursor, e: ParseError) {
        eprintln!("{e:?}");
        self.errors.push(e);
    }

    fn parse_ident(
        &mut self,
        c: &mut ParseCursor,
        anchors: &[TokenKind],
        after: &str,
    ) -> ParseResult<IdentId> {
        let ident_tok = c.expect(TokenKind::Ident, anchors, || after)?.clone();
        Ok(self.add_ident(c, &ident_tok))
    }

    // https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html
    fn parse_expr(&mut self, c: &mut ParseCursor, prec_lvl: i32) -> ParseResult<ExprId> {
        fn prefix_lvl(kind: TokenKind) -> i32 {
            match kind {
                TokenKind::Minus => 30,
                TokenKind::Bang => 29,
                _ => -1,
            }
        }

        fn postfix_lvl(kind: TokenKind) -> Option<i32> {
            let lvl = match kind {
                TokenKind::ParenL => 32,
                TokenKind::SquareL => 33,
                TokenKind::Dot => 34,
                _ => return None,
            };

            Some(lvl)
        }

        fn infix_lvl(kind: TokenKind) -> Option<(i32, i32)> {
            use TokenKind::*;

            let lvl = match kind {
                Plus | Minus => (23, 24),
                EqualEq | NotEq => (16, 17),
                Great | GreatEq | Less | LessEq => (14, 15),
                Star | Slash | Perc => (25, 26),
                And => (12, 13),
                Or => (10, 11),
                Caret => (27, 28),
                _ => return None,
            };

            Some(lvl)
        }

        let tok = &c.advance().clone();

        use TokenKind::*;
        let mut lhs =
            match tok.kind {
                IntNumber => {
                    let num =
                        c.lex
                            .token_string(tok)
                            .parse()
                            .map_err(|e| ParseError::IntParseError {
                                inner: e,
                                pos: c.lex.pos_from_tok(tok),
                            })?;
                    self.add_expr(Expr::IntLiteral(num))
                }

                FloatNumber => {
                    let num = c.lex.token_string(tok).parse().map_err(|e| {
                        ParseError::FloatParseError {
                            inner: e,
                            pos: c.lex.pos_from_tok(tok),
                        }
                    })?;
                    self.add_expr(Expr::FloatLiteral(num))
                }

                True => self.add_expr(Expr::BoolLiteral(true)),
                False => self.add_expr(Expr::BoolLiteral(false)),

                Ident => {
                    let ident = self.add_ident(c, tok);
                    self.add_expr(Expr::Variable(ident))
                }

                // prefix op
                Minus | Bang => {
                    let lvl = prefix_lvl(tok.kind);
                    let op = tok.kind;
                    let rhs = self.parse_expr(c, lvl)?;
                    self.add_expr(Expr::Unary { op, rhs })
                }

                ParenL => {
                    let lhs = self.parse_expr(c, 0)?;
                    c.expect(
                        TokenKind::ParenR,
                        &[TokenKind::Semicolon],
                        || "expression parenthesis grouping",
                    )?;
                    lhs
                }

                BraceL => {
                    let block = self.parse_block(c)?;
                    self.add_expr(Expr::Block(block))
                }

                //       // array literal
                //       SquareL => {
                //         let exprs = self.collect_listing(
                //           |p| p.parse_expr(0),
                //           TokenKind::Comma,
                //           TokenKind::BraceR,
                //           "unclosed array literal")?;
                //
                //         self.push_expr(Expr::Literal(ExprLiteral::Array(exprs)), t.span)
                //       }
                _ => {
                    return Err(ParseError::MalformedExpression(c.lex.token_pos(tok)));
                }
            };

        while c.has_some() {
            let op = c.peek();
            if !op.kind.is_op() {
                break;
            }

            if let Some(postfix_lvl) = postfix_lvl(op.kind) {
                if postfix_lvl < prec_lvl {
                    break;
                }
                let op = op.clone();
                c.advance();

                lhs = match op.kind {
                    TokenKind::ParenL => {
                        let args = self.parse_listing(
                            c,
                            TokenKind::Comma,
                            TokenKind::ParenR,
                            |p, c| p.parse_expr(c, 0),
                            ListingKind::FuncCallArgs,
                        )?;
                        self.add_expr(Expr::Call { callee: lhs, args })
                    }

                    TokenKind::SquareL => {
                        let idx = self.parse_expr(c, 0)?;
                        self.add_expr(Expr::Indexing { src: lhs, idx })
                    }

                    TokenKind::Dot => {
                        let member = self.parse_ident(
                            c,
                            &[TokenKind::Semicolon, TokenKind::Equal],
                            "struct member access",
                        )?;
                        self.add_expr(Expr::Member { src: lhs, member })
                    }
                    _ => unreachable!(), // this shouldnt happen! precondition: postfix_lvl returns only these variants
                };

                continue;
            }

            if let Some((left_lvl, right_lvl)) = infix_lvl(op.kind) {
                if left_lvl < prec_lvl {
                    break;
                }
                let op = op.clone();
                c.advance();

                if c.eat(TokenKind::Question).is_some() {
                    let first = self.parse_expr(c, 0)?;
                    c.expect(
                        TokenKind::Colon,
                        &[TokenKind::Semicolon],
                        || "ternary operator first choice",
                    )?;

                    let second = self.parse_expr(c, 0)?;
                    lhs = self.add_expr(Expr::Ternary {
                        cond: lhs,
                        first,
                        second,
                    });
                } else {
                    // TODO: check for assignment here

                    let rhs = self.parse_expr(c, right_lvl)?;
                    lhs = self.add_expr(Expr::Binary {
                        op: op.kind,
                        lhs,
                        rhs,
                    });
                }

                continue;
            }

            break;
        }

        Ok(lhs)
    }

    fn parse_ty_annot(&mut self, c: &mut ParseCursor) -> ParseResult<TyAnnotId> {
        let tok = &c.advance().clone();

        let ty = match tok.kind {
            TokenKind::Ident => {
                let name = self.add_ident(c, tok);

                let args = if c.eat(TokenKind::Less).is_some() {
                    self.parse_listing(
                        c,
                        TokenKind::Comma,
                        TokenKind::Great,
                        |p, c| p.parse_ty_annot(c),
                        ListingKind::TypeGenericsAnnot,
                    )?
                } else {
                    Box::default()
                };

                let ty = if c.eat(TokenKind::SquareL).is_some() {
                    let inner = self.add_ty_annot(TyAnnot::Type { name, args });
                    let size = self.parse_expr(c, 0)?;
                    c.expect(
                        TokenKind::SquareR,
                        &[TokenKind::Semicolon, TokenKind::Equal],
                        || "array indexing value",
                    )?;

                    TyAnnot::Array { inner, size }
                } else {
                    TyAnnot::Type { name, args }
                };

                self.add_ty_annot(ty)
            }

            TokenKind::Fn => {
                let params = self.parse_listing(
                    c,
                    TokenKind::Comma,
                    TokenKind::ParenR,
                    |p, c| p.parse_ty_annot(c),
                    ListingKind::FuncParamsAnnot,
                )?;

                let ret = if c.eat(TokenKind::Arrow).is_some() {
                    Some(self.parse_ty_annot(c)?)
                } else {
                    None
                };

                self.add_ty_annot(TyAnnot::Func { params, ret })
            }

            TokenKind::ParenL => {
                let inner = self.parse_listing(
                    c,
                    TokenKind::Comma,
                    TokenKind::ParenR,
                    |p, c| p.parse_ty_annot(c),
                    ListingKind::TupleAnnot,
                )?;

                self.add_ty_annot(TyAnnot::Tuple(inner))
            }
            _ => {
                return Err(ParseError::MalformedTypeAnnotation(c.lex.token_pos(tok)));
            }
        };

        Ok(ty)
    }

    fn parse_listing<T, F>(
        &mut self,
        c: &mut ParseCursor,
        separator: TokenKind,
        terminator: TokenKind,
        mut getter: F,
        what: ListingKind,
    ) -> ParseResult<List<T>>
    where
        F: FnMut(&mut Self, &mut ParseCursor) -> ParseResult<T>,
    {
        let mut list = Vec::new();
        let start = c.peek().clone();

        loop {
            if c.eat(terminator).is_some() {
                break;
            }

            match getter(self, c) {
                Ok(item) => list.push(item),
                Err(e) => self.report_error(c, e),
            }

            if c.eat(separator).is_none() {
                // if we don't find a separator, we are expecting a terminator
                // if we don't get a terminator, it is an error

                if c.eat(terminator).is_none() {
                    return Err(ParseError::UnclosedListing {
                        start: c.lex.pos_from_tok(&start),
                        end: c.lex.pos_from_tok(c.peek()),
                        what,
                    });
                }

                break;
            }
        }

        Ok(list.into())
    }

    fn parse_block(&mut self, c: &mut ParseCursor) -> ParseResult<Block> {
        let start = StmtId(self.stmts.len() as Id);
        let start_tok = c.prev().clone();

        let mut is_closed = false;
        while c.has_some() {
            if c.eat(TokenKind::BraceR).is_some() {
                is_closed = true;
                break;
            }

            match self.parse_stmt(c) {
                Ok(_) => {}
                Err(e) => self.report_error(c, e),
            }
        }

        if !is_closed {
            return Err(ParseError::UnclosedParenthesis {
                kind: TokenKind::BraceR,
                start: c.lex.pos_from_tok(&start_tok),
            });
        }

        let end = StmtId(self.stmts.len() as Id);
        Ok(Block { start, end })
    }

    fn parse_var_decl(&mut self, c: &mut ParseCursor) -> ParseResult<StmtId> {
        let name = self.parse_ident(c, &[TokenKind::Semicolon, TokenKind::Equal], "let token")?;

        let ty = if c.eat(TokenKind::Colon).is_some() {
            Some(self.parse_ty_annot(c)?)
        } else {
            None
        };

        c.expect(
            TokenKind::Equal,
            &[TokenKind::Semicolon],
            || "variable declaration identifier",
        )?;

        let expr = self.parse_expr(c, 0)?;
        c.expect_semicolon("var declaration")?;

        let stmt = Stmt::VarDecl(VarContent {
            name,
            ty,
            rhs: expr,
        });
        Ok(self.add_stmt(stmt))
    }

    fn parse_stmt(&mut self, c: &mut ParseCursor) -> ParseResult<StmtId> {
        let tok = c.advance();

        let id = match tok.kind {
            TokenKind::Let => self.parse_var_decl(c)?,
            TokenKind::Return => {
                let expr = self.parse_expr(c, 0)?;
                c.expect_semicolon("return")?;
                let ret = self.add_expr(Expr::Return(expr));
                self.add_stmt(Stmt::Expr(ret))
            }

            TokenKind::Fn => {
                let name_tok = c
                    .expect(
                        TokenKind::Ident,
                        &[TokenKind::ParenL, TokenKind::ParenR, TokenKind::BraceL],
                        || "fn token",
                    )?
                    .clone();
                let name = self.add_ident(c, &name_tok);

                let generics = if c.eat(TokenKind::Less).is_some() {
                    self.parse_listing(
                        c,
                        TokenKind::Comma,
                        TokenKind::Great,
                        |p, c| {
                            p.parse_ident(
                                c,
                                &[TokenKind::Comma, TokenKind::Great, TokenKind::ParenL],
                                "function generics list",
                            )
                        },
                        ListingKind::TypeGenericsAnnot,
                    )?
                } else {
                    Box::default()
                };

                c.expect(
                    TokenKind::ParenL,
                    &[TokenKind::ParenR, TokenKind::BraceL],
                    || "function identifier or generics list",
                )?;

                let params = self.parse_listing(
                    c,
                    TokenKind::Comma,
                    TokenKind::ParenR,
                    |p, c| {
                        let name = p.parse_ident(
                            c,
                            &[TokenKind::Comma, TokenKind::ParenR, TokenKind::BraceL],
                            "function parameters start",
                        )?;
                        c.expect(
                            TokenKind::Colon,
                            &[TokenKind::Comma, TokenKind::ParenR, TokenKind::BraceL],
                            || "function parameter identifier",
                        )?;
                        Ok((name, p.parse_ty_annot(c)?))
                    },
                    ListingKind::FuncParamsAnnot,
                )?;

                let ret = if c.eat(TokenKind::Arrow).is_some() {
                    Some(self.parse_ty_annot(c)?)
                } else {
                    None
                };

                c.expect(
                    TokenKind::BraceL,
                    &[
                        TokenKind::BraceR,
                        TokenKind::Let,
                        TokenKind::Fn,
                        TokenKind::Struct,
                    ],
                    || "function parameters closing or function return type",
                )?;
                let code = self.parse_block(c)?;

                self.add_stmt(Stmt::FuncDecl(FuncContent {
                    name,
                    generics,
                    params,
                    ret,
                    code,
                }))
            }

            TokenKind::Struct => {
                let name =
                    self.parse_ident(c, &[TokenKind::BraceL, TokenKind::BraceR], "struct token")?;

                let generics = if c.eat(TokenKind::Less).is_some() {
                    self.parse_listing(
                        c,
                        TokenKind::Comma,
                        TokenKind::Great,
                        |p, c| {
                            p.parse_ident(
                                c,
                                &[TokenKind::Great, TokenKind::Comma, TokenKind::BraceL],
                                "struct generics list",
                            )
                        },
                        ListingKind::TypeGenericsAnnot,
                    )?
                } else {
                    Box::default()
                };

                c.expect(
                    TokenKind::BraceL,
                    &[TokenKind::BraceR],
                    || "struct declaration identifier",
                )?;
                let fields = self.parse_listing(
                    c,
                    TokenKind::Comma,
                    TokenKind::BraceR,
                    |p, c| {
                        let name = p.parse_ident(
                            c,
                            &[TokenKind::Comma, TokenKind::BraceR],
                            "struct fields",
                        )?;
                        c.expect(
                            TokenKind::Colon,
                            &[TokenKind::Comma, TokenKind::BraceR],
                            &|| "struct field name",
                        )?;
                        Ok((name, p.parse_ty_annot(c)?))
                    },
                    ListingKind::StructFields,
                )?;

                self.add_stmt(Stmt::StructDecl(StructContent {
                    name,
                    generics,
                    fields,
                }))
            }

            TokenKind::Eof => return Err(ParseError::ReachedEof),

            _ => {
                // try to parse expression
                c.undo();
                let expr = self.parse_expr(c, 0)?;
                // TODO: Handle malformed expressions
                // match self.get_expr(expr) {
                //     Expr::Variable(_) => {
                //         return Err(MalformedExpression(c.lex.token_pos(c.prev())));
                //     }
                //     _ => {}
                // }
                c.expect_semicolon("expression")?;
                self.add_stmt(Stmt::Expr(expr))
            }
        };

        Ok(id)
    }
}

struct ParseCursor<'a> {
    lex: &'a Lexer,
    pos: u32,
}
impl<'a> ParseCursor<'a> {
    fn new(lex: &'a Lexer) -> Self {
        Self { lex, pos: 0 }
    }

    fn peek(&self) -> &Token {
        // we always at least have the last EOF token, so this cant fail
        // be careful in eating loops
        &self.lex.tokens[self.pos as usize]
    }

    fn prev(&self) -> &Token {
        &self.lex.tokens[self.pos.saturating_sub(1) as usize]
    }

    fn check(&self, target: TokenKind) -> bool {
        self.peek().kind == target
    }

    fn advance(&mut self) -> &Token {
        self.pos += 1;
        self.prev()
    }

    fn undo(&mut self) {
        self.pos -= 1;
    }

    fn eat(&mut self, target: TokenKind) -> Option<&Token> {
        if self.check(target) {
            Some(self.advance())
        } else {
            None
        }
    }

    fn expect<S: Into<String>, M: FnOnce() -> S>(
        &mut self,
        target: TokenKind,
        anchors: &[TokenKind],
        msg: M,
    ) -> ParseResult<&Token> {
        if self.check(target) {
            Ok(self.advance())
        } else {
            let tok = self.peek().clone();

            let mut anchors = anchors.to_vec();
            anchors.push(target);

            self.recover(&anchors);
            self.eat(target); // we still check for the target after recovering

            let err = ParseError::WrongToken {
                pos: self.lex.pos_from_tok(&tok),
                expected: target,
                actual: tok.kind,
                at: msg().into(),
            };
            Err(err)
        }
    }

    fn expect_semicolon(&mut self, msg: &str) -> ParseResult<&Token> {
        if self.check(TokenKind::Semicolon) {
            Ok(self.advance())
        } else {
            let tok = self.peek().clone();
            let err = ParseError::WrongToken {
                pos: self.lex.pos_from_tok(&tok),
                expected: TokenKind::Semicolon,
                actual: tok.kind,
                at: format!("end of {msg} statement"),
            };
            Err(err)
        }
    }

    fn has_some(&self) -> bool {
        (self.pos as usize) < self.lex.tokens.len()
    }

    fn skip_while<F: FnMut(&TokenKind) -> bool>(&mut self, f: F) -> u32 {
        let len = self.lex.tokens[self.pos as usize..]
            .iter()
            .map(|t| t.kind)
            .take_while(f)
            .count();

        self.pos += len as u32;
        len as u32
    }

    fn recover(&mut self, anchors: &[TokenKind]) -> Span {
        let start = self.pos;
        let len = self.skip_while(|t| !anchors.contains(t));

        if !self.has_some() {
            // stay pointing at EOF token
            self.undo();
        }

        Span {
            start,
            end: len + start,
        }
    }
}

pub fn parse(lex: &Lexer) -> Parser {
    let mut p = Parser::default();
    let mut c = ParseCursor::new(lex);

    while c.has_some() {
        match p.parse_stmt(&mut c) {
            Ok(id) => {}
            Err(e) => p.report_error(&mut c, e),
        }
    }

    p
}
