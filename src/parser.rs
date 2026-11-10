use crate::intern::StringInterner;
use crate::*;

// OPT:
// https://jhwlr.io/super-flat-ast/
// https://thunderseethe.dev/posts/parser-base/

pub type Id = u32;
#[derive(Debug, Clone, Copy)]
pub struct ExprId(pub Id);
#[derive(Debug, Clone, Copy)]
pub struct StmtId(pub Id);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdentId(pub Id);
#[derive(Debug, Clone, Copy)]
pub struct TyAnnotId(pub Id);

pub type List<T> = Box<[T]>;

#[derive(Debug, Clone)]
// Statements indexes are [start, end)
pub struct Block {
    // pub start: StmtId,
    // pub end: StmtId,
    pub list: List<StmtId>,
}

// TODO: expressions are not added in order. no way to do superflat opt right now
#[derive(Debug, Clone)]
pub enum Expr {
    IntLiteral(u32),
    FloatLiteral(f32),
    BoolLiteral(bool),
    // ArrayLiteral(List<ExprId>),
    // TupleLiteral(List<ExprId>),
    StructLiteral(IdentId, List<(IdentId, ExprId)>),

    Variable(IdentId),
    Return(ExprId),
    // Break,
    // Continue,
    Grouping(ExprId),
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

    IfElse {
        cond: ExprId,
        first: ExprId,
        second: Option<ExprId>,
    },
    While {
        cond: ExprId,
        code: ExprId,
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

#[derive(Debug)]
pub enum TyAnnot {
    // Void,
    // Hole,
    Type {
        name: IdentId,
        args: List<TyAnnotId>,
    },
    Array {
        inner: TyAnnotId,
        size: ExprId,
    },
    Func {
        params: List<TyAnnotId>,
        ret: Option<TyAnnotId>,
    },
    Tuple(List<TyAnnotId>),
}

#[derive(Debug, Clone)]
pub struct VarContent {
    pub name: IdentId,
    pub ty: Option<TyAnnotId>,
    pub rhs: ExprId, // TODO: do we want optional definition?
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
    Break,
    Continue,
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
    InvalidStmt,
    MalformedExpression,
    MalformedTypeAnnotation,

    IntParseError(std::num::ParseIntError),
    FloatParseError(std::num::ParseFloatError),

    UnclosedBlock,
    UnclosedListing(ListingKind),

    ReachedEof,
}

fn err_span(err: ParseErrorKind, span: Span) -> ParseError {
    (err, span)
}
fn err_tok(err: ParseErrorKind, tok: Token) -> ParseError {
    err_span(err, tok.to_span())
}

// TODO: update these as new statements and expressions are added
// TODO: these are getting big. maybe better to turn em into sets?
const INNER_ANCHORS: &[TokenKind] = &[
    TokenKind::Let,
    TokenKind::Return,
    TokenKind::If,
    TokenKind::While,
    TokenKind::For,
    TokenKind::Equal,
    // we add these closing tokens and recovers automatically will skip em
    TokenKind::Semicolon,
    TokenKind::BraceR,
];
const FNDECL_ANCHORS: &[TokenKind] = &[TokenKind::BraceL, TokenKind::Struct, TokenKind::Let];
const TOPLVL_ANCHORS: &[TokenKind] = &[TokenKind::Fn, TokenKind::Struct, TokenKind::Let];

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
    fn add_ident(&mut self, c: &ParseCursor, start: Token) -> IdentId {
        IdentId(self.idents.add(c.lex.str_from_tok(start)))
    }

    fn add_ty_annot(&mut self, ty: TyAnnot, c: &ParseCursor, start: Token) -> TyAnnotId {
        let id = self.types.len();
        self.types.push((ty, c.span(start)));
        TyAnnotId(id as Id)
    }

    fn add_stmt(&mut self, stmt: Stmt, c: &ParseCursor, start: Token) -> StmtId {
        let id = self.stmts.len();
        self.stmts.push((stmt, c.span(start)));
        StmtId(id as Id)
    }

    fn add_expr(&mut self, expr: Expr, c: &ParseCursor, start: Token) -> ExprId {
        let id = self.exprs.len();
        self.exprs.push((expr, c.span(start)));
        ExprId(id as Id)
    }

    fn add_err(&mut self, _c: &mut ParseCursor, e: ParseError) {
        self.errors.push(e);
    }

    fn parse_ident(
        &mut self,
        c: &mut ParseCursor,
        anchors: &[TokenKind],
        after: &str,
    ) -> ParseResult<IdentId> {
        let ident_tok = c.expect(TokenKind::Ident, anchors, || after)?;
        Ok(self.add_ident(c, ident_tok))
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

        let start = c.advance();

        let mut lhs = match start.kind {
            TokenKind::IntNumber => {
                let num = c
                    .lex
                    .str_from_tok(start)
                    .parse()
                    .map_err(|e| err_tok(ParseErrorKind::IntParseError(e), start))?;
                self.add_expr(Expr::IntLiteral(num), c, start)
            }

            TokenKind::FloatNumber => {
                let num = c
                    .lex
                    .str_from_tok(start)
                    .parse()
                    .map_err(|e| err_tok(ParseErrorKind::FloatParseError(e), start))?;
                self.add_expr(Expr::FloatLiteral(num), c, start)
            }

            TokenKind::True => self.add_expr(Expr::BoolLiteral(true), c, start),
            TokenKind::False => self.add_expr(Expr::BoolLiteral(false), c, start),

            TokenKind::If => {
                let cond = self.parse_expr(c, 0)?;
                let first = self.parse_expr_block(c, start, || "if expr")?;

                let second = if c.eat(TokenKind::Else).is_some() {
                    Some(self.parse_expr_block(c, start, || "else expr")?)
                } else {
                    None
                };

                self.add_expr(
                    Expr::IfElse {
                        cond,
                        first,
                        second,
                    },
                    c,
                    start,
                )
            }

            TokenKind::While => {
                let cond = self.parse_expr(c, 0)?;
                let code = self.parse_expr_block(c, start, || "while block")?;

                self.add_expr(Expr::While { cond, code }, c, start)
            }

            TokenKind::Ident => {
                let ident = self.add_ident(c, start);

                if c.eat(TokenKind::BraceL).is_some() {
                    // struct literal
                    let fields = self.parse_listing(
                        c,
                        TokenKind::Comma,
                        TokenKind::BraceR,
                        |p, c| {
                            let name = p.parse_ident(c, INNER_ANCHORS, "struct literal")?;
                            c.expect(TokenKind::Colon, INNER_ANCHORS, || "struct literal values")?;

                            Ok((name, p.parse_expr(c, 0)?))
                        },
                        ListingKind::StructFields,
                    )?;

                    // we sort the fields because they might come in any order
                    // fields.sort_by_key(|f| f.0);

                    self.add_expr(Expr::StructLiteral(ident, fields), c, start)
                } else {
                    self.add_expr(Expr::Variable(ident), c, start)
                }
            }

            // prefix op
            TokenKind::Minus | TokenKind::Bang => {
                let lvl = prefix_lvl(start.kind);
                let op = start.kind;
                let rhs = self.parse_expr(c, lvl)?;
                self.add_expr(Expr::Unary { op, rhs }, c, start)
            }

            TokenKind::ParenL => {
                let lhs = self.parse_expr(c, 0)?;
                c.expect(
                    TokenKind::ParenR,
                    INNER_ANCHORS,
                    || "expression parenthesis grouping",
                )?;
                self.add_expr(Expr::Grouping(lhs), c, start)
            }

            TokenKind::BraceL => {
                let block = self.parse_block(c)?;
                self.add_expr(Expr::Block(block), c, start)
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
            TokenKind::Invalid => return Err(err_tok(ParseErrorKind::InvalidCharacter, start)),
            _ => {
                // TODO: handle malformed expression better?
                return Err(err_tok(ParseErrorKind::MalformedExpression, start));
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
                        self.add_expr(Expr::Call { callee: lhs, args }, c, start)
                    }

                    TokenKind::SquareL => {
                        let idx = self.parse_expr(c, 0)?;
                        self.add_expr(Expr::Indexing { src: lhs, idx }, c, start)
                    }

                    TokenKind::Dot => {
                        let member = self.parse_ident(c, INNER_ANCHORS, "struct member access")?;
                        self.add_expr(Expr::Member { src: lhs, member }, c, start)
                    }
                    _ => unreachable!(), // this shouldnt happen! precondition: postfix_lvl returns only these variants
                };

                continue;
            }

            if let Some((left_lvl, right_lvl)) = infix_lvl(op.kind) {
                if left_lvl < prec_lvl {
                    break;
                }
                c.advance();

                if c.eat(TokenKind::Question).is_some() {
                    let first = self.parse_expr(c, 0)?;
                    c.expect(
                        TokenKind::Colon,
                        INNER_ANCHORS,
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
                        start,
                    );
                // } else if c.eat(TokenKind::Equal).is_some() {
                //     // TODO: not sure about this. this permits to chain multiple assignments
                //     // assignment should probably just be a statement
                //     // check if lhs is assignable is done at typechecking level
                //     let rhs = self.parse_expr(c, 0)?;
                //     lhs = self.add_expr(Expr::Assign { lhs, rhs }, c, tok);
                } else {
                    let rhs = self.parse_expr(c, right_lvl)?;

                    lhs = self.add_expr(
                        Expr::Binary {
                            op: op.kind,
                            lhs,
                            rhs,
                        },
                        c,
                        start,
                    );
                }

                continue;
            }

            break;
        }

        Ok(lhs)
    }

    fn parse_ty_annot(&mut self, c: &mut ParseCursor) -> ParseResult<TyAnnotId> {
        let tok = c.advance();

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
                    let size = self.parse_expr(c, 0)?; // expression is checked for constness at typechecker level
                    c.expect(TokenKind::SquareR, INNER_ANCHORS, || "array size value")?;

                    TyAnnot::Array { inner, size }
                } else {
                    TyAnnot::Type { name, args }
                };

                self.add_ty_annot(ty, c, tok)
            }

            TokenKind::Fn => {
                c.expect(TokenKind::ParenL, INNER_ANCHORS, || "function annotation")?;

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
        let start = c.peek();

        while c.has_some() {
            if c.eat(terminator).is_some() {
                break;
            }

            // TODO: this doesn't handle errors well, because right now except ALWAYS recovers with anchors
            // with our current system, getting an error here means always getting a unclosed listing error too..
            // match getter(self, c) {
            //     Ok(item) => list.push(item),
            //     Err(e) => {
            //         dbg!(c.peek());
            //         self.report_error(c, e);
            //         // c.recover(&[separator, terminator]);
            //     }
            // }

            list.push(getter(self, c)?);

            if c.eat(separator).is_none() {
                // if we don't find a separator, we are expecting a terminator
                // if we don't get a terminator, it is an error

                if c.eat(terminator).is_none() {
                    let span = c.span(start);
                    return Err(err_span(ParseErrorKind::UnclosedListing(what), span));
                }

                break;
            }
        }

        Ok(list.into())
    }

    fn parse_block(&mut self, c: &mut ParseCursor) -> ParseResult<Block> {
        // let start = StmtId(self.stmts.len() as Id);
        let start_tok = c.prev();

        let mut list = Vec::new();
        let mut is_closed = false;
        while c.has_some() {
            if c.eat(TokenKind::BraceR).is_some() {
                is_closed = true;
                break;
            }

            match self.parse_stmt(c) {
                Ok(id) => list.push(id),
                Err(e) => {
                    self.add_err(c, e);
                    // c.recover(INNER_ANCHORS);
                }
            }

            if c.eat(TokenKind::Semicolon).is_none() {
                // only last statements can miss the semicolon
                c.expect(
                    TokenKind::BraceR,
                    &[],
                    || "only last statement in block can miss the semicolon",
                )?;
                is_closed = true;
                break;
            }
        }

        if !is_closed {
            return Err(err_tok(ParseErrorKind::UnclosedBlock, start_tok));
        }

        // let end = StmtId(self.stmts.len() as Id);
        Ok(Block { list: list.into() })
    }

    fn parse_expr_block<S: Into<String>, M: FnOnce() -> S>(
        &mut self,
        c: &mut ParseCursor,
        start: Token,
        msg: M,
    ) -> ParseResult<ExprId> {
        c.expect(TokenKind::BraceL, INNER_ANCHORS, msg)?;
        let block = self.parse_block(c)?;
        Ok(self.add_expr(Expr::Block(block), c, start))
    }

    fn parse_var_decl(&mut self, c: &mut ParseCursor, start: Token) -> ParseResult<StmtId> {
        let name = self.parse_ident(c, INNER_ANCHORS, "let token")?;

        let ty = if c.eat(TokenKind::Colon).is_some() {
            Some(self.parse_ty_annot(c)?)
        } else {
            None
        };

        c.expect(
            TokenKind::Equal,
            INNER_ANCHORS,
            || "variable declaration identifier",
        )?;

        let expr = self.parse_expr(c, 0)?;

        let stmt = Stmt::VarDecl(VarContent {
            name,
            ty,
            rhs: expr,
        });

        Ok(self.add_stmt(stmt, c, start))
    }

    fn parse_stmt(&mut self, c: &mut ParseCursor) -> ParseResult<StmtId> {
        let start = c.advance();

        let id = match start.kind {
            TokenKind::Let => self.parse_var_decl(c, start)?,

            TokenKind::Return => {
                let expr = self.parse_expr(c, 0)?;
                let ret = self.add_expr(Expr::Return(expr), c, start);
                self.add_stmt(Stmt::Expr(ret), c, start)
            }

            TokenKind::Continue => {
              self.add_stmt(Stmt::Continue, c, start)
            }

            TokenKind::Break => {
              self.add_stmt(Stmt::Break, c, start)
            }

            TokenKind::Fn => {
                let name_tok = c.expect(TokenKind::Ident, FNDECL_ANCHORS, || "fn token")?;
                let name = self.add_ident(c, name_tok);

                let generics = if c.eat(TokenKind::Less).is_some() {
                    self.parse_listing(
                        c,
                        TokenKind::Comma,
                        TokenKind::Great,
                        |p, c| p.parse_ident(c, FNDECL_ANCHORS, "function generics list"),
                        ListingKind::TypeGenericsAnnot,
                    )?
                } else {
                    Box::default()
                };

                c.expect(
                    TokenKind::ParenL,
                    FNDECL_ANCHORS,
                    || "function identifier or generics list",
                )?;

                let params = self.parse_listing(
                    c,
                    TokenKind::Comma,
                    TokenKind::ParenR,
                    |p, c| {
                        let name = p.parse_ident(c, FNDECL_ANCHORS, "function parameters start")?;
                        c.expect(
                            TokenKind::Colon,
                            FNDECL_ANCHORS,
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
                    FNDECL_ANCHORS,
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
                    start,
                )
            }

            TokenKind::Struct => {
                let name = self.parse_ident(c, TOPLVL_ANCHORS, "struct token")?;

                let generics = if c.eat(TokenKind::Less).is_some() {
                    self.parse_listing(
                        c,
                        TokenKind::Comma,
                        TokenKind::Great,
                        |p, c| p.parse_ident(c, TOPLVL_ANCHORS, "struct generics list"),
                        ListingKind::TypeGenericsAnnot,
                    )?
                } else {
                    Box::default()
                };

                c.expect(
                    TokenKind::BraceL,
                    TOPLVL_ANCHORS,
                    || "struct declaration identifier",
                )?;
                let fields = self.parse_listing(
                    c,
                    TokenKind::Comma,
                    TokenKind::BraceR,
                    |p, c| {
                        let name = p.parse_ident(c, TOPLVL_ANCHORS, "struct fields")?;
                        c.expect(TokenKind::Colon, TOPLVL_ANCHORS, &|| "struct field name")?;
                        Ok((name, p.parse_ty_annot(c)?))
                    },
                    ListingKind::StructFields,
                )?;

                // we sort the fields because literals might list them in any order
                // fields.sort_by_key(|f| f.0);

                self.add_stmt(
                    Stmt::StructDecl(StructContent {
                        name,
                        generics,
                        fields,
                    }),
                    c,
                    start,
                )
            }

            TokenKind::Invalid => return Err(err_tok(ParseErrorKind::InvalidCharacter, start)),
            TokenKind::Eof => return Err(err_tok(ParseErrorKind::ReachedEof, start)),

            _ => {
                // try to parse expression
                c.undo();
                let expr = self.parse_expr(c, 0)?;
                self.add_stmt(Stmt::Expr(expr), c, start)
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

    fn peek(&self) -> Token {
        // we always at least have the last EOF token, so this cant fail
        // be careful in eating loops
        self.lex.tokens[self.pos as usize]
    }

    fn prev(&self) -> Token {
        self.lex.tokens[self.pos.saturating_sub(1) as usize]
    }

    fn check(&self, target: TokenKind) -> bool {
        self.peek().kind == target
    }

    fn advance(&mut self) -> Token {
        self.pos += 1;
        self.prev()
    }

    fn undo(&mut self) {
        self.pos -= 1;
    }

    fn eat(&mut self, target: TokenKind) -> Option<Token> {
        if self.check(target) {
            Some(self.advance())
        } else {
            None
        }
    }

    fn span(&self, start_tok: Token) -> Span {
        start_tok.to_span().merge(&self.peek().to_span())
    }

    fn expect<S: Into<String>, M: FnOnce() -> S>(
        &mut self,
        target: TokenKind,
        anchors: &[TokenKind],
        msg: M,
    ) -> ParseResult<Token> {
        if self.check(target) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            self.recover(&anchors);

            let err = err_span(
                ParseErrorKind::WrongToken {
                    expected: target,
                    actual: tok.kind,
                    at: msg().into(),
                },
                tok.to_span(),
            );
            Err(err)
        }
    }

    fn expect_semicolon(&mut self) -> ParseResult<Token> {
        if self.check(TokenKind::Semicolon) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            let err = err_tok(
                ParseErrorKind::WrongToken {
                    expected: TokenKind::Semicolon,
                    actual: tok.kind,
                    at: "end of statement".into(),
                },
                tok,
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
        self.skip_while(|t| *t == TokenKind::Semicolon || *t == TokenKind::BraceR); // also skip any trailing semicolon just in case

        if !self.has_some() {
            // stay pointing at EOF token, or else next peek will go out of bounds
            self.undo();
        }
    }
}

pub fn parse(lex: &Lexer) -> Parser {
    let mut p = Parser::default();
    let mut c = ParseCursor::new(lex);

    while c.has_some() {
        match p.parse_stmt(&mut c).and_then(|stmt_id| {
            // if we have an closing brace, ignore the trailing semicolon;
            // otherwise, expect it
            // TODO: wouldnt it be a lot better if we allowed empty statements...?

            if c.prev().kind != TokenKind::BraceR {
                c.expect_semicolon()?;
            } else {
                c.eat(TokenKind::Semicolon);
            }
            Ok(stmt_id)
        }) {
            Ok(id) => p.global_stmts.push(id),
            Err(e) => {
                p.add_err(&mut c, e);
                c.recover(TOPLVL_ANCHORS);
            }
        }
    }

    p
}
