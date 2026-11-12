use crate::*;
use std::collections::HashSet;

#[derive(Default)]
struct Emitter {
    out: String,
    counter: usize,
    block_exprs: HashSet<ExprId>,
}
impl Emitter {
    fn push<S: AsRef<str>>(&mut self, s: S) {
        self.out.push_str(s.as_ref());
    }

    fn pop(&mut self) {
        self.out.pop();
    }

    fn name(&mut self, id: usize, ident: IdentId, ast: &Ast) {
        self.name_new(id, ast.get_str(ident));
    }

    fn name_new(&mut self, id: usize, s: &str) {
        self.push("_cmb_");
        self.push(s);
        self.push(id.to_string());
    }

    fn name_tmp(&mut self) {
        self.push("_cmb__tmp");
        self.push(self.counter.to_string());
        self.counter += 1;
    }

    fn emit_type(&mut self, annot_id: TyAnnotId, name: Option<IdentId>, ast: &Ast) {
        match ast.get_annot(annot_id) {
            TyAnnot::Type { name, .. } => {
                self.push(ast.get_str(*name));
            }

            // TODO: arrays are a problem.
            // if it is a variable declaration, the type goes before the name, and the square brackets go after the name.
            // we have to emit the type and the name together!
            TyAnnot::Array { inner, size } => {
                todo!("arrays not implemented")
            }

            // expecting this to be a function pointer, needs the name like arrays too!
            // remember: ret_ty (*name) (params)
            TyAnnot::Func { params, ret } => {
                todo!("function pointers not implemented")
            }

            TyAnnot::Tuple(list) => todo!("tuples not supported"),
        }
    }

    fn emit_stmt(&mut self, id: StmtId, ast: &Ast) {
        match ast.get_stmt(id) {
            Stmt::VarDecl(content) => {
                self.emit_type(
                    content
                        .ty
                        .expect("variable type annotation should have been filled by typechecker"),
                    Some(content.name),
                    ast,
                );
                self.push(" ");
                self.name(id.0 as usize, content.name, ast);
                self.push("=");
                self.emit_expr(content.rhs, ast);
            }

            Stmt::FuncDecl(_) => {}
            Stmt::StructDecl(_) => {}

            Stmt::Break => self.push("break"),
            Stmt::Continue => self.push("continue"),

            Stmt::Expr(e) => self.emit_expr(*e, ast),
        }

        self.push(";\n");
    }

    fn emit_block(&mut self, block: &Block, ast: &Ast) {
        self.push('{');
        for stmt_id in block.start.0..block.end.0 {
            emit_stmt(StmtId(stmt_id), ast, output);
        }
        self.push('}');
    }

    fn emit_tmp_block(&mut self, block: &Block, ast: &Ast) {}

    fn emit_blocks_if_any(&mut self, id: ExprId, ast: &Ast) {
        match ast.get_expr(id) {
            Expr::Empty => {}
            Expr::BoolLiteral(_) => {}
            Expr::IntLiteral(_) => {}
            Expr::FloatLiteral(_) => {}
            Expr::Variable(_) => {}

            Expr::Grouping(expr) => self.emit_blocks_if_any(*expr, ast),

            Expr::Unary { rhs, .. } => self.emit_blocks_if_any(*rhs, ast),

            Expr::Binary { lhs, rhs, .. } => {
                self.emit_blocks_if_any(*lhs, ast);
                self.emit_blocks_if_any(*rhs, ast);
            }
            Expr::Ternary {
                cond,
                first,
                second,
            } => {
                self.emit_blocks_if_any(*cond, ast);
                self.emit_blocks_if_any(*first, ast);
                self.emit_blocks_if_any(*second, ast);
            }

            Expr::Call { callee, args } => {
                self.emit_blocks_if_any(*callee, ast);
                for arg in args {
                    self.emit_blocks_if_any(*arg, ast);
                }
            }

            Expr::StructLiteral(name, fields) => {
                for field in fields {
                    self.emit_blocks_if_any(field.1, ast);
                }
            }

            Expr::Indexing { src, idx } => {
                self.emit_blocks_if_any(*src, ast);
                self.emit_blocks_if_any(*idx, ast);
            }

            Expr::Member { src, member } => self.emit_blocks_if_any(*src, ast),

            Expr::Return(e) => self.emit_blocks_if_any(*e, ast),

            // TODO: C doesn't allow block expressions.
            // it's easy to deal with, tho.
            // declare the variable earlier
            // emit the block statement
            // assign the variable on the last expression of the block
            // only problem is the outstide variable can be shadowed inside the block; just add some prefix to the outside variable.
            Expr::Block(block) => {}

            Expr::IfElse {
                cond,
                first,
                second,
            } => {}

            Expr::While { cond, code } => {}
        }
    }

    fn emit_expr(&mut self, id: ExprId, ast: &Ast) {
        self.emit_blocks_if_any(id, ast);
        self.emit_expr_rec(id, ast);
    }

    fn emit_expr_rec(&mut self, id: ExprId, ast: &Ast) {
        match ast.get_expr(id) {
            Expr::Empty => {}
            Expr::BoolLiteral(b) => self.push(if *b { "1" } else { "0" }),
            Expr::IntLiteral(n) => self.push(&n.to_string()),
            Expr::FloatLiteral(f) => self.push(&f.to_string()),

            Expr::Variable(v) => self.name(id.0 as usize, *v, ast),

            Expr::Grouping(expr) => {
                self.push('(');
                emit_expr(*expr, ast, output);
                self.push(')');
            }

            Expr::Unary { op, rhs } => {
                self.push_str(op.to_c_op().expect("illegal C operator"));
                emit_expr(*rhs, ast, output);
            }

            Expr::Binary { op, lhs, rhs } => {
                emit_expr(*lhs, ast, output);
                self.push_str(op.to_c_op().expect("illegal C operator"));
                emit_expr(*rhs, ast, output);
            }
            Expr::Ternary {
                cond,
                first,
                second,
            } => {
                emit_expr(*cond, ast, output);
                self.push('?');
                emit_expr(*first, ast, output);
                self.push(':');
                emit_expr(*second, ast, output);
            }

            Expr::Call { callee, args } => {
                emit_expr(*callee, ast, output);

                self.push('(');
                for arg in args {
                    emit_expr(*arg, ast, output);
                    self.push(',');
                }
                self.push(')');
            }

            Expr::Indexing { src, idx } => {
                emit_expr(*src, ast, output);
                self.push('[');
                emit_expr(*idx, ast, output);
                self.push(']');
            }

            Expr::Member { src, member } => todo!("structs not implemented"),

            Expr::Return(e) => {
                self.push_str("return ");
                emit_expr(*e, ast, output);
            }

            // TODO: C doesn't allow block expressions.
            // it's easy to deal with, tho.
            // declare the variable earlier
            // emit the block statement
            // assign the variable on the last expression of the block
            // only problem is the outstide variable can be shadowed inside the block; just add some prefix to the outside variable.
            Expr::Block(block) => emit_block(block, ast, output),
        }
    }
}

pub fn emit(checker: &Typechecker, ast: &Ast) -> String {
    let mut e = Emitter::default();

    // first, forward declare all structs
    for (id, strct) in checker.structs_instances.iter().enumerate() {
        e.push("struct ");
        e.name(id, strct.name, ast);
        e.push(";\n");
    }

    // second, define all structs
    // TODO: this needs topological sorting
    for (id, strct) in checker.structs_instances.iter().enumerate() {
        e.push("struct ");
        e.name(id, strct.name, ast);

        e.push("{");

        for field in strct.fields {
            e.emit_type(field.1, Some(field.0), ast);
            e.push(";");
        }
        if strct.fields.len() > 0 {
            e.pop();
        }

        e.push("};\n");
    }

    // third, forward declare all funcs
    for (id, func) in checker.funcs_instances.iter().enumerate() {
        emit_type(func.ret, None, ast);
        e.push(" ");
        e.name_new(id, "func");
        e.push("(");

        for param in func.params {
            e.emit_type(param, None, ast);
            e.push(",");
        }
        if func.params.len() > 0 {
            e.pop();
        }

        e.push(");\n");
    }

    // fourth, forward declare all global vars
    // TODO: define them as the first instructions of main function
    // or else put them in another function thats called by main
    for stmt_id in &ast.global_stmts {
        match ast.get_stmt(*stmt_id) {
            Stmt::VarDecl(content) => {
                e.emit_type(content.ty.unwrap(), Some(content.name), ast);
                e.push(";\n");
            }

            _ => {}
        }
    }

    // fifth, define all funcs
    for (id, func) in checker.funcs_instances.iter().enumerate() {
        e.emit_type(func.ret, None, ast);
        e.name_new(id, "func");
        e.push("(");

        for param in func.params {
            e.emit_type(param, None, ast);
            e.push(",");
        }
        if func.params.len() > 0 {
            e.out.pop();
        }

        e.push(");\n");
        e.push("{");

        for stmt_id in func.body.list {
            e.emit_stmt(stmt_id, ast);
        }

        e.push("}\n");
    }

    e.out
}
