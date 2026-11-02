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

// TODO: expressions are not added in order. no way to do superflat opt right now
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
pub enum ParseErrorKind {
    WrongToken {
        expected: TokenKind,
        actual: TokenKind,
        at: String,
    },

    InvalidCharacter,

    InvalidStmtInGlobal,
    InvalidStmtInBlock,

    IntParseError(std::num::ParseIntError),
    FloatParseError(std::num::ParseFloatError),

    UnclosedBlock,
    UnclosedListing(ListingKind),

    MalformedExpression,
    MalformedTypeAnnotation,

    ReachedEof,
}

fn err_span(err: ParseErrorKind, span: Span) -> ParseError {
    (err, span)
}
fn err_tok(err: ParseErrorKind, tok: &Token) -> ParseError {
    err_span(err, tok.span)
}

type ParseError = Spanned<ParseErrorKind>;
type ParseResult<T> = Result<T, ParseError>;

#[derive(Default)]
pub struct Parser {
    pub errors: Vec<ParseError>,

    pub idents: StringInterner,
    pub exprs: Vec<Spanned<Expr>>,
    pub stmts: Vec<Spanned<Stmt>>,
    pub types: Vec<Spanned<TyAnnot>>,
    pub global_stmts: Vec<StmtId>,
}

impl Parser {
    fn add_ident(&mut self, c: &ParseCursor, tok: &Token) -> IdentId {
        IdentId(self.idents.add(c.lex.string_tok(tok)))
    }

    fn add_ty_annot(&mut self, ty: TyAnnot, c: &ParseCursor, start: &Token) -> TyAnnotId {
        let id = self.types.len();
        self.types.push((ty, c.span(start)));
        TyAnnotId(id as Id)
    }

    fn add_stmt(&mut self, stmt: Stmt, c: &ParseCursor, start: &Token) -> StmtId {
        let id = self.stmts.len();
        self.stmts.push((stmt, c.span(start)));
        StmtId(id as Id)
    }

    fn add_expr(&mut self, expr: Expr, c: &ParseCursor, start: &Token) -> ExprId {
        let id = self.exprs.len();
        self.exprs.push((expr, c.span(start)));
        ExprId(id as Id)
    }

    pub fn get_expr(&self, expr: ExprId) -> &Expr {
        &self.exprs[expr.0 as usize].0
    }

    pub fn get_stmt(&self, stmt: StmtId) -> &Stmt {
        &self.stmts[stmt.0 as usize].0
    }

    pub fn get_ident(&self, ident: IdentId) -> &str {
        self.idents.get(ident.0)
    }

    // TODO: do recover inside here
    fn report_error(&mut self, c: &mut ParseCursor, e: ParseError) {
        eprintln!(
            "{:?} {:?} {:?} {:?}",
            c.lex.string_span(&e.1),
            e.0,
            c.lex.pos_from_offset(e.1.start),
            c.lex.pos_from_offset(e.1.end)
        );
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

        let mut lhs = match tok.kind {
            TokenKind::IntNumber => {
                let num = c
                    .lex
                    .string_tok(tok)
                    .parse()
                    .map_err(|e| err_tok(ParseErrorKind::IntParseError(e), tok))?;
                self.add_expr(Expr::IntLiteral(num), c, tok)
            }

            TokenKind::FloatNumber => {
                let num = c
                    .lex
                    .string_tok(tok)
                    .parse()
                    .map_err(|e| err_tok(ParseErrorKind::FloatParseError(e), tok))?;
                self.add_expr(Expr::FloatLiteral(num), c, tok)
            }

            TokenKind::True => self.add_expr(Expr::BoolLiteral(true), c, tok),
            TokenKind::False => self.add_expr(Expr::BoolLiteral(false), c, tok),

            TokenKind::Ident => {
                let ident = self.add_ident(c, tok);
                self.add_expr(Expr::Variable(ident), c, tok)
            }

            // prefix op
            TokenKind::Minus | TokenKind::Bang => {
                let lvl = prefix_lvl(tok.kind);
                let op = tok.kind;
                let rhs = self.parse_expr(c, lvl)?;
                self.add_expr(Expr::Unary { op, rhs }, c, tok)
            }

            TokenKind::ParenL => {
                let lhs = self.parse_expr(c, 0)?;
                c.expect(
                    TokenKind::ParenR,
                    &[TokenKind::Semicolon],
                    || "expression parenthesis grouping",
                )?;
                lhs
            }

            TokenKind::BraceL => {
                let block = self.parse_block(c)?;
                self.add_expr(Expr::Block(block), c, tok)
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
            TokenKind::Invalid => return Err(err_tok(ParseErrorKind::InvalidCharacter, tok)),
            _ => {
                return Err(err_tok(ParseErrorKind::MalformedExpression, tok));
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
                        self.add_expr(Expr::Call { callee: lhs, args }, c, tok)
                    }

                    TokenKind::SquareL => {
                        let idx = self.parse_expr(c, 0)?;
                        self.add_expr(Expr::Indexing { src: lhs, idx }, c, tok)
                    }

                    TokenKind::Dot => {
                        let member = self.parse_ident(
                            c,
                            &[TokenKind::Semicolon, TokenKind::Equal],
                            "struct member access",
                        )?;
                        self.add_expr(Expr::Member { src: lhs, member }, c, tok)
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
                    lhs = self.add_expr(
                        Expr::Ternary {
                            cond: lhs,
                            first,
                            second,
                        },
                        c,
                        tok,
                    );
                } else {
                    // TODO: check for assignment here

                    let rhs = self.parse_expr(c, right_lvl)?;

                    lhs = self.add_expr(
                        Expr::Binary {
                            op: op.kind,
                            lhs,
                            rhs,
                        },
                        c,
                        tok,
                    );
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
                    let inner = self.add_ty_annot(TyAnnot::Type { name, args }, c, tok);
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

                self.add_ty_annot(ty, c, tok)
            }

            TokenKind::Fn => {
                c.expect(
                    TokenKind::ParenL,
                    &[TokenKind::Semicolon, TokenKind::Equal],
                    || "function annotation",
                )?;

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

                self.add_ty_annot(TyAnnot::Func { params, ret }, c, tok)
            }

            TokenKind::ParenL => {
                let inner = self.parse_listing(
                    c,
                    TokenKind::Comma,
                    TokenKind::ParenR,
                    |p, c| p.parse_ty_annot(c),
                    ListingKind::TupleAnnot,
                )?;

                self.add_ty_annot(TyAnnot::Tuple(inner), c, tok)
            }

            TokenKind::Invalid => return Err(err_tok(ParseErrorKind::InvalidCharacter, tok)),
            _ => {
                return Err(err_tok(ParseErrorKind::MalformedTypeAnnotation, tok));
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

        while c.has_some() {
            if c.eat(terminator).is_some() {
                break;
            }

            match getter(self, c) {
                Ok(item) => list.push(item),
                Err(e) => {
                    self.report_error(c, e);
                    c.recover(&[separator, terminator]);
                }
            }

            if c.eat(separator).is_none() {
                // if we don't find a separator, we are expecting a terminator
                // if we don't get a terminator, it is an error

                if c.eat(terminator).is_none() {
                    let span = c.span(&start);
                    return Err(err_span(ParseErrorKind::UnclosedListing(what), span));
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

            match self.parse_inner_stmt(c) {
                Ok(_) => {}
                Err(e) => {
                    self.report_error(c, e);
                    c.recover(&[TokenKind::Semicolon, TokenKind::BraceR]);
                }
            }
        }

        if !is_closed {
            return Err(err_tok(ParseErrorKind::UnclosedBlock, &start_tok));
        }

        let end = StmtId(self.stmts.len() as Id);
        Ok(Block { start, end })
    }

    fn parse_var_decl(&mut self, c: &mut ParseCursor, start: &Token) -> ParseResult<StmtId> {
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

        Ok(self.add_stmt(stmt, c, start))
    }

    fn parse_inner_stmt(&mut self, c: &mut ParseCursor) -> ParseResult<StmtId> {
        let tok = &c.advance().clone();

        let id = match tok.kind {
            TokenKind::Let => self.parse_var_decl(c, tok)?,

            TokenKind::Return => {
                let expr = self.parse_expr(c, 0)?;
                c.expect_semicolon("return")?;
                let ret = self.add_expr(Expr::Return(expr), c, tok);
                self.add_stmt(Stmt::Expr(ret), c, tok)
            }

            TokenKind::Invalid => return Err(err_tok(ParseErrorKind::InvalidCharacter, tok)),
            TokenKind::Eof => return Err(err_tok(ParseErrorKind::ReachedEof, tok)),

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
                self.add_stmt(Stmt::Expr(expr), c, tok)
            }
        };

        Ok(id)
    }

    fn parse_top_stmt(&mut self, c: &mut ParseCursor) -> ParseResult<StmtId> {
        let tok = &c.advance().clone();

        let id = match tok.kind {
            TokenKind::Let => self.parse_var_decl(c, tok)?,
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

                self.add_stmt(
                    Stmt::FuncDecl(FuncContent {
                        name,
                        generics,
                        params,
                        ret,
                        code,
                    }),
                    c,
                    tok,
                )
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

                self.add_stmt(
                    Stmt::StructDecl(StructContent {
                        name,
                        generics,
                        fields,
                    }),
                    c,
                    tok,
                )
            }

            TokenKind::Invalid => return Err(err_tok(ParseErrorKind::InvalidCharacter, tok)),
            TokenKind::Eof => return Err(err_tok(ParseErrorKind::ReachedEof, tok)),
            _ => return Err(err_tok(ParseErrorKind::InvalidStmtInGlobal, tok)),
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

    fn span(&self, start_tok: &Token) -> Span {
        start_tok.span.merge(&self.prev().span)
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

            let err = err_span(
                ParseErrorKind::WrongToken {
                    expected: target,
                    actual: tok.kind,
                    at: msg().into(),
                },
                self.span(&tok),
            );
            Err(err)
        }
    }

    fn expect_semicolon(&mut self, msg: &str) -> ParseResult<&Token> {
        if self.check(TokenKind::Semicolon) {
            Ok(self.advance())
        } else {
            let tok = self.peek().clone();
            let err = err_tok(
                ParseErrorKind::WrongToken {
                    expected: TokenKind::Semicolon,
                    actual: tok.kind,
                    at: format!("end of {msg} statement"),
                },
                &tok,
            );
            Err(err)
        }
    }

    fn has_some(&self) -> bool {
        // do not count EOF token
        (self.pos as usize) < self.lex.tokens.len() - 1
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

    fn recover(&mut self, anchors: &[TokenKind]) {
        self.skip_while(|t| !anchors.contains(t));

        if !self.has_some() {
            // stay pointing at EOF token
            self.undo();
        }
    }
}

pub fn parse(lex: &Lexer) -> Parser {
    let mut p = Parser::default();
    let mut c = ParseCursor::new(lex);

    while c.has_some() {
        match p.parse_top_stmt(&mut c) {
            Ok(id) => p.global_stmts.push(id),
            Err(e) => p.report_error(&mut c, e),
        }
    }

    p
}
