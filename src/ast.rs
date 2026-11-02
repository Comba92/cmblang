pub struct Ast {
    pub input: String,
    pub idents: StringInterner,
    pub exprs: Vec<Expr>,
    pub stmts: Vec<Stmt>,
    pub global_stmts: Vec<StmtId>,
    pub types: Vec<TyAnnot>,
}
impl Ast {
    pub fn new() -> Self {}
}
