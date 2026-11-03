use crate::*;

pub fn emit(ast: &Ast) -> String {
    let mut output = String::new();

    // forward declare everything on top
    for stmt_id in &ast.global_stmts {
        emit_decl(*stmt_id, ast, &mut output);
    }

    for stmt_id in &ast.global_stmts {
        emit_stmt(*stmt_id, ast, &mut output);
    }

    output
}

fn emit_type(annot_id: TyAnnotId, ast: &Ast, output: &mut String) {
    let (annot, _) = ast.get_annot(annot_id);

    match annot {
        TyAnnot::Type { name, args } => {
            output.push_str(ast.get_ident(*name));
            // TODO: generics not supported yet
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

fn emit_decl(stmt_id: StmtId, ast: &Ast, output: &mut String) {
    let (stmt, _) = ast.get_stmt(stmt_id);

    match stmt {
        Stmt::VarDecl(content) => {
            emit_type(
                content
                    .ty
                    .expect("variable type annotation should have been filled by typechecker"),
                ast,
                output,
            );
            output.push(' ');
            output.push_str(ast.get_ident(content.name));
            output.push(';');
        }
        Stmt::FuncDecl(content) => {
            if let Some(ret) = content.ret {
                emit_type(ret, ast, output);
            } else {
                output.push_str("void");
            }
            output.push(' ');
            output.push_str(ast.get_ident(content.name));
            output.push('(');
            for param in &content.params {
                emit_type(param.1, ast, output);
                output.push(',');
            }
            output.pop(); // pop last ','
            output.push_str(");");
        }
        Stmt::StructDecl(content) => todo!(),
        Stmt::Expr(_) => {}
    }
}

fn emit_stmt(stmt_id: StmtId, ast: &Ast, output: &mut String) {
    let (stmt, _) = ast.get_stmt(stmt_id);

    match stmt {
        Stmt::VarDecl(content) => {
            emit_type(
                content
                    .ty
                    .expect("variable type annotation should have been filled by typechecker"),
                ast,
                output,
            );
            output.push(' ');
            output.push_str(ast.get_ident(content.name));
            output.push('=');
            emit_expr(content.rhs, ast, output);
            output.push(';');
        }

        Stmt::FuncDecl(content) => {
            if let Some(ret) = content.ret {
                emit_type(ret, ast, output);
            } else {
                output.push_str("void");
            }
            output.push(' ');
            output.push_str(ast.get_ident(content.name));
            output.push('(');
            for param in &content.params {
                emit_type(param.1, ast, output);
                output.push(' ');
                output.push_str(ast.get_ident(param.0));
                output.push(',');
            }
            output.pop(); // pop last ','
            output.push(')');

            emit_block(&content.code, ast, output);
        }

        Stmt::StructDecl(content) => todo!("structs not implemented"),

        Stmt::Expr(e) => {
            emit_expr(*e, ast, output);
            output.push(';');
        }
    }
}

fn emit_block(block: &Block, ast: &Ast, output: &mut String) {
    output.push('{');
    for stmt_id in block.start.0..block.end.0 {
        emit_stmt(StmtId(stmt_id), ast, output);
    }
    output.push('}');
}

fn emit_expr(expr_id: ExprId, ast: &Ast, output: &mut String) {
    let (expr, _) = ast.get_expr(expr_id);

    match expr {
        Expr::BoolLiteral(b) => output.push(if *b { '1' } else { '0' }),
        Expr::IntLiteral(n) => output.push_str(&n.to_string()),
        Expr::FloatLiteral(f) => output.push_str(&f.to_string()),

        Expr::Variable(v) => output.push_str(ast.get_ident(*v)),

        Expr::Grouping(expr) => {
            output.push('(');
            emit_expr(*expr, ast, output);
            output.push(')');
        }

        Expr::Unary { op, rhs } => {
            output.push_str(op.to_c_op().expect("illegal C operator"));
            emit_expr(*rhs, ast, output);
        }

        Expr::Binary { op, lhs, rhs } => {
            emit_expr(*lhs, ast, output);
            output.push_str(op.to_c_op().expect("illegal C operator"));
            emit_expr(*rhs, ast, output);
        }
        Expr::Ternary {
            cond,
            first,
            second,
        } => {
            emit_expr(*cond, ast, output);
            output.push('?');
            emit_expr(*first, ast, output);
            output.push(':');
            emit_expr(*second, ast, output);
        }

        Expr::Call { callee, args } => {
            emit_expr(*callee, ast, output);

            output.push('(');
            for arg in args {
                emit_expr(*arg, ast, output);
                output.push(',');
            }
            output.push(')');
        }

        Expr::Indexing { src, idx } => {
            emit_expr(*src, ast, output);
            output.push('[');
            emit_expr(*idx, ast, output);
            output.push(']');
        }

        Expr::Member { src, member } => todo!("structs not implemented"),

        Expr::Return(e) => {
            output.push_str("return ");
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
