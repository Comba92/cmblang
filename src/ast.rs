use crate::*;
use intern::*;

pub struct Ast {
    pub input: String,
    pub idents: StringInterner,
    pub exprs: Vec<Spanned<Expr>>,
    pub stmts: Vec<Spanned<Stmt>>,
    pub annots: Vec<Spanned<TyAnnot>>,
    pub global_stmts: Vec<StmtId>,
}

impl Ast {
    pub fn new(lex: Lexer, parser: Parser) -> Self {
        Self {
            input: lex.input,
            idents: parser.idents,
            exprs: parser.exprs,
            stmts: parser.stmts,
            annots: parser.types,
            global_stmts: parser.global_stmts,
        }
    }

    pub fn get_expr(&self, expr: ExprId) -> &Expr {
        &self.exprs[expr.0 as usize].0
    }

    pub fn get_stmt(&self, stmt: StmtId) -> &Stmt {
        &self.stmts[stmt.0 as usize].0
    }

    pub fn get_stmt_mut(&mut self, id: StmtId) -> &mut Stmt {
        &mut self.stmts[id.0 as usize].0
    }

    pub fn get_ident(&self, ident: IdentId) -> &str {
        self.idents.get(ident.0)
    }

    pub fn get_annot(&self, annot: TyAnnotId) -> &TyAnnot {
        &self.annots[annot.0 as usize].0
    }

    pub fn add_ty_annot(&mut self, ty: TyAnnot) -> TyAnnotId {
        let id = self.annots.len();
        self.annots.push((ty, Span { start: 0, end: 0 }));
        TyAnnotId(id as Id)
    }
}
