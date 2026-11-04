use crate::{intern::*, *};
use std::collections::{HashMap, HashSet};

// https://www.reddit.com/r/ProgrammingLanguages/comments/v5zwlw/resources_for_building_a_typechecker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/8kox21/help_implementing_a_type_checker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/1nhy0ef/how_do_i_make_a_real_typechecker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/1gkjkz4/how_to_implement_local_type_inference/

// https://ettolrach.com/blog/bidirectional_inference.html
// https://thunderseethe.dev/posts/how-to-choose-between-hm-and-bidir/
// https://thunderseethe.dev/posts/bidirectional-constraint-generation/
// https://smallcultfollowing.com/babysteps/blog/2017/03/25/unification-in-chalk-part-1/

#[derive(Debug, Clone, Copy, Hash)]
struct TypeId(pub Id);
type Scope = HashMap<IdentId, TypeId>;
struct Environment {
    scopes: Vec<Scope>,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }
}
impl Environment {
    fn lookup(&self, name: IdentId) -> Option<TypeId> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(&name) {
                return Some(*ty);
            }
        }

        None
    }

    fn lookup_global(&self, name: IdentId) -> Option<TypeId> {
        self.scopes.first().unwrap().get(&name).copied()
    }

    // TODO: check if value already exists
    fn bind(&mut self, name: IdentId, ty: TypeId) {
        self.scopes.last_mut().unwrap().insert(name, ty);
    }

    // TODO: this is dangerous. Every push NEEDS a relative POP or else we risk of gertting unwanted state.
    // a push operation with a callback that automatically pops after the callback should be ideal
    // hoewever that requires mutable access to the whole environment struct, currently it is inside the typechecker struct
    // we would need to extract it from there and pass it separately to every typechker function
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new())
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

#[derive(Debug, Clone, Hash)]
pub enum Type {
    Void,
    Integer,
    Float,
    Bool,
    Generic(u32, IdentId), // TODO: change this to be func: IdentId, name: IdentId,
    TypeVar(u32),
    Array {
        inner: TypeId,
        size: u32,
    },
    Func {
        params: List<TypeId>,
        ret: TypeId,
    },
    Struct {
        name: IdentId,
        fields: List<(IdentId, TypeId)>,
    },
    Tuple(List<TypeId>),
}

const VOID_ID: TypeId = TypeId(0);
const BOOL_ID: TypeId = TypeId(1);
const INT_ID: TypeId = TypeId(2);
const FLOAT_ID: TypeId = TypeId(3);

const PRIMITIVES: &[(Type, TypeId)] = &[
    (Type::Void, VOID_ID),
    (Type::Bool, BOOL_ID),
    (Type::Integer, INT_ID),
    (Type::Float, FLOAT_ID),
];

// TODO: better errors here, should probably be SPANNED
#[derive(Debug)]
pub enum TypeErrorKind {
    UndeclaredVar,
    WrongAnnotation { cause: Box<TypeError> },
    WrongTypecheck { expected: Type, actual: Type },
    RequiredAnnotation,

    InvalidStmtInGlobal,
    InvalidStmtInBlock,

    DuplicateParamsInFunction,
    DuplicateGenericsInFunction,
    DuplicateFieldsInStruct,
    DuplicateGenericsInStruct,

    CallingWithWrongType,
    IndexingWithWrongType,
}

type TypeError = Spanned<TypeErrorKind>;
pub fn tyerr(kind: TypeErrorKind, span: &Span) -> TypeError {
    (kind, *span)
}

type TypeResult<T> = Result<T, TypeError>;

#[derive(Default)]
struct Inference {
    ty_var_count: u32,
    constraints: Vec<(TypeId, TypeId)>,
}
impl Inference {
    fn fresh_ty_var(&mut self) -> u32 {
        let res = self.ty_var_count;
        self.ty_var_count += 1;
        res
    }
}

pub struct Typechecker {
    env: Environment,
    types: Interner<Type>,
    unify: Inference,

    vars_typed: Vec<(StmtId, TypeId)>,
    pub errors: Vec<TypeError>,
}

impl Default for Typechecker {
    fn default() -> Self {
        let mut types = Interner::new();
        for primitive in PRIMITIVES {
            let id = types.add(primitive.0.clone());
            debug_assert_eq!(id, primitive.1.0);
        }

        Self {
            env: Environment::default(),
            types,
            unify: Default::default(),
            vars_typed: vec![],
            errors: vec![],
        }
    }
}

fn type_to_annotation(ty_id: TypeId, tc: &Typechecker, ast: &mut Ast) -> TyAnnotId {
    let annot = match tc.get_type(ty_id) {
        Type::Void => TyAnnot::Type {
            name: IdentId(ast.idents.add("void")),
            args: Default::default(),
        },

        Type::Integer => TyAnnot::Type {
            name: IdentId(ast.idents.add("int")),
            args: Default::default(),
        },

        Type::Float => TyAnnot::Type {
            name: IdentId(ast.idents.add("float")),
            args: Default::default(),
        },

        Type::Bool => TyAnnot::Type {
            name: IdentId(ast.idents.add("int")), // we treat bools as ints in C
            args: Default::default(),
        },

        Type::Generic(_, name) => TyAnnot::Type {
            name: *name,
            args: Default::default(),
        },

        Type::TypeVar(u32) => unreachable!("can't convert a type var to a type annotation"),

        Type::Array { inner, size } => TyAnnot::Array {
            inner: type_to_annotation(*inner, tc, ast),
            size: todo!("array types not implemented yet"),
        },

        Type::Func { params, ret } => {
            let list = Vec::new();
            for param in params {
                type_to_annotation(*param, tc, ast);
            }

            let ret = type_to_annotation(*ret, tc, ast);

            TyAnnot::Func {
                params: list.into(),
                ret: Some(ret),
            }
        }

        Type::Struct { name, fields } => todo!("structs not implemented yet"),
        Type::Tuple(list) => todo!("tuple not implemented yet"),
    };

    ast.add_ty_annot(annot)
}

impl Typechecker {
    fn add_type(&mut self, ty: Type) -> TypeId {
        // let id = self.types.len();
        // self.types.push(ty);
        let id = self.types.add(ty);
        TypeId(id as Id)
    }

    fn add_ty_var(&mut self) -> TypeId {
        let id = self
            .types
            .add(Type::Generic(self.unify.fresh_ty_var(), IdentId(0)));
        TypeId(id as Id)
    }

    fn get_type(&self, id: TypeId) -> &Type {
        // &self.types[id.0 as usize]
        self.types.get(id.0)
    }

    fn report_error(&mut self, ast: &Ast, e: TypeError) {
        eprintln!(
            "{:?} {:?} {:?} {:?}",
            ast.lex.str_from_span(&e.1),
            e.0,
            ast.lex.pos_from_offset(e.1.start),
            ast.lex.pos_from_offset(e.1.end)
        );
        self.errors.push(e);
    }

    fn add_annotation_as_type(&mut self, ast: &Ast, annot_id: TyAnnotId) -> TypeId {
        self.add_annotation_as_type_with_generics(ast, annot_id, &[])
    }

    fn add_annotation_as_type_with_generics(
        &mut self,
        ast: &Ast,
        annot_id: TyAnnotId,
        generics: &[(IdentId, TypeId)],
    ) -> TypeId {
        let (annot, span) = ast.get_annot(annot_id);
        let ty = match annot {
            TyAnnot::Type { name, args } => {
                if let Some(generic) = generics.iter().find(|g| g.0 == *name) {
                    if !args.is_empty() {
                        todo!("error, generic type variables can't have generic args")
                    }

                    return generic.1;
                } else {
                    let str = ast.get_ident(*name);
                    match str {
                        "bool" => return BOOL_ID,
                        "int" => return INT_ID,
                        "float" => return FLOAT_ID,
                        _ => todo!("user defined types not implemented"),
                    }
                }
            }

            TyAnnot::Array { inner, size } => {
                let inner_id = self.add_annotation_as_type(ast, *inner);
                // TODO: size not handled right now
                Type::Array {
                    inner: inner_id,
                    size: 0,
                }
            }

            TyAnnot::Func { params, ret } => {
                let mut list = Vec::new();
                for param in params {
                    let id = self.add_annotation_as_type(ast, *param);
                    list.push(id);
                }

                let ret = if let Some(ret) = ret {
                    self.add_annotation_as_type(ast, *ret)
                } else {
                    VOID_ID
                };

                Type::Func {
                    params: list.into(),
                    ret,
                }
            }

            TyAnnot::Tuple(elems) => {
                let mut list = Vec::new();
                for elem in elems {
                    let id = self.add_annotation_as_type(ast, *elem);
                    list.push(id);
                }

                Type::Tuple(list.into())
            }
        };

        self.add_type(ty)
    }

    fn var_decl(
        &mut self,
        ast: &Ast,
        var: &VarContent,
        stmt_id: StmtId,
        span: &Span,
    ) -> TypeResult<()> {
        let final_ty = if let Some(ty_annot) = var.ty {
            let annot_id = self.add_annotation_as_type(ast, ty_annot);
            self.check(ast, var.rhs, annot_id)
                .map_err(|e| tyerr(TypeErrorKind::WrongAnnotation { cause: Box::new(e) }, span))?;
            annot_id
        } else {
            self.infer(ast, var.rhs)?
        };

        self.env.bind(var.name, final_ty);
        self.vars_typed.push((stmt_id, final_ty));

        Ok(())
    }

    fn global_pass(&mut self, ast: &Ast, stmt_id: StmtId) -> TypeResult<()> {
        let (stmt, span) = ast.get_stmt(stmt_id);
        match stmt {
            Stmt::FuncDecl(content) => {
                // TODO: should check if fn name already exists

                if has_duplicates(&content.generics, |g| *g) {
                    return Err(tyerr(TypeErrorKind::DuplicateGenericsInFunction, span));
                }

                if has_duplicates(&content.params, |p| p.0) {
                    return Err(tyerr(TypeErrorKind::DuplicateParamsInFunction, span));
                }

                // TODO: temporary solution
                let generics: Vec<_> = content
                    .generics
                    .iter()
                    .map(|g| {
                        let ty_var = Type::Generic(self.unify.fresh_ty_var(), *g);
                        (*g, self.add_type(ty_var))
                    })
                    .collect();

                // add fn type to types list and to global scope
                let ret = if let Some(ret) = content.ret {
                    self.add_annotation_as_type_with_generics(ast, ret, &generics)
                } else {
                    VOID_ID
                };

                let ty = Type::Func {
                    params: content
                        .params
                        .iter()
                        .map(|p| self.add_annotation_as_type_with_generics(ast, p.1, &generics))
                        .collect(),
                    ret,
                };

                let ty_id = self.add_type(ty);
                self.env.bind(content.name, ty_id);
            }

            Stmt::StructDecl(content) => {
                todo!("structs not implemented: add struct type to types list")
            }
            _ => {}
        }

        Ok(())
    }

    fn typecheck(&mut self, ast: &Ast, stmt_id: StmtId) -> TypeResult<()> {
        let (stmt, span) = ast.get_stmt(stmt_id);
        match stmt {
            Stmt::VarDecl(content) => self.var_decl(ast, content, stmt_id, span)?,

            Stmt::FuncDecl(content) => {
                self.env.push_scope();

                let func = self.env.lookup(content.name).expect(
                    "function should already be in global scope after name resolution pass",
                );
                match self.get_type(func).clone() {
                    Type::Func {
                        params,
                        ret: ret_expected,
                    } => {
                        for (param_ty, param_content) in params.iter().zip(content.params.iter()) {
                            self.env.bind(param_content.0, *param_ty);
                        }

                        let ret_actual = self.typecheck_block(ast, &content.code);
                        self.env.pop_scope();

                        let ret_actual = ret_actual?;

                        if ret_actual.0 != ret_expected.0 {
                            let err = tyerr(
                                TypeErrorKind::WrongTypecheck {
                                    expected: self.get_type(ret_expected).clone(),
                                    actual: self.get_type(ret_actual).clone(),
                                },
                                span,
                            );

                            Err(err)?
                        }
                    }

                    _ => unreachable!(
                        "function type should already be in global scope after name resolution pass"
                    ),
                }
            }

            Stmt::StructDecl(_) => {} // Do nothing, already done in toplvl_pass

            Stmt::Expr(_) => {
                let err = tyerr(TypeErrorKind::InvalidStmtInGlobal, span);
                Err(err)?
            }
        }

        Ok(())
    }

    fn typecheck_block(&mut self, ast: &Ast, block: &Block) -> TypeResult<TypeId> {
        let mut ret_ty = None;

        for stmt_id in block.start.0..block.end.0 {
            let stmt_id = StmtId(stmt_id);
            let (stmt, span) = ast.get_stmt(stmt_id);
            match stmt {
                Stmt::VarDecl(content) => self.var_decl(ast, content, stmt_id, span)?,

                Stmt::Expr(expr_id) => {
                    let (expr, _) = ast.get_expr(*expr_id);

                    match expr {
                        Expr::Return(expr_id) => match ret_ty {
                            None => {
                                // we found the first return epxr.
                                ret_ty = Some(self.infer(ast, *expr_id)?);
                            }
                            Some(found_ret_ty) => {
                                // we found another return expr. all other return statement should have the same type as the first.
                                self.check(ast, *expr_id, found_ret_ty)?;
                            }
                        },

                        _ => _ = self.infer(ast, *expr_id)?,
                    }
                }

                Stmt::FuncDecl(_) | Stmt::StructDecl(_) => {
                    let err = tyerr(TypeErrorKind::InvalidStmtInBlock, span);
                    Err(err)?
                }
            };
        }

        // if we didn't find any return types, this block has type void
        Ok(ret_ty.unwrap_or(VOID_ID))
    }

    fn infer(&mut self, ast: &Ast, expr: ExprId) -> TypeResult<TypeId> {
        let (expr, span) = ast.get_expr(expr);
        let id = match expr {
            Expr::BoolLiteral(_) => BOOL_ID,
            Expr::IntLiteral(_) => INT_ID,
            Expr::FloatLiteral(_) => FLOAT_ID,

            Expr::Grouping(expr) => self.infer(ast, *expr)?,

            Expr::Variable(name) => self
                .env
                .lookup(*name)
                .ok_or_else(|| tyerr(TypeErrorKind::UndeclaredVar, span))?,

            Expr::Return(id) => self.infer(ast, *id)?,

            // TODO: right now, no way to check if operand amits the operation between types
            Expr::Unary { op, rhs } => self.infer(ast, *rhs)?,
            Expr::Binary { op, lhs, rhs } => {
                let lhs_ty = self.infer(ast, *lhs)?;
                self.check(ast, *rhs, lhs_ty)?;
                lhs_ty
            }
            Expr::Ternary {
                cond,
                first,
                second,
            } => {
                self.check(ast, *cond, BOOL_ID)?;
                let ty = self.infer(ast, *first)?;
                self.check(ast, *second, ty)?;
                ty
            }

            Expr::Call { callee, args } => {
                let ty_id = self.infer(ast, *callee)?;
                match self.get_type(ty_id).clone() {
                    Type::Func { params, ret } => {
                        // we must clone to both access params and do check

                        for (param, arg) in params.iter().zip(args.iter()) {
                            match self.get_type(*param) {
                                Type::Generic(_, _) => {
                                    let ty_var = self.add_ty_var();
                                    self.check(ast, *arg, ty_var)?
                                }
                                _ => self.check(ast, *arg, *param)?,
                            }
                        }
                        ret
                    }

                    _ => Err(tyerr(TypeErrorKind::CallingWithWrongType, span))?,
                }
            }

            Expr::Indexing { src, idx } => {
                self.check(ast, *idx, INT_ID)?;

                let ty = self.infer(ast, *src)?;
                match self.get_type(ty) {
                    Type::Array { inner, size } => {
                        // TODO: we might even do constant bound checking here
                        *inner
                    }
                    _ => Err(tyerr(TypeErrorKind::IndexingWithWrongType, span))?,
                }
            }

            Expr::Member { src, member } => {
                todo!("structs not implemented yet")
            }

            Expr::Block(block) => {
                self.env.push_scope();
                let res = self.typecheck_block(ast, block);
                self.env.pop_scope();
                res?
            }
        };

        Ok(id)
    }

    fn check(&mut self, ast: &Ast, expr_id: ExprId, expected_ty: TypeId) -> TypeResult<()> {
        let (expr, span) = ast.get_expr(expr_id);

        let inferred_ty = match expr {
            Expr::BoolLiteral(_) => BOOL_ID,
            Expr::IntLiteral(_) => INT_ID,
            Expr::FloatLiteral(_) => FLOAT_ID,
            _ => {
                let inferred_ty = self.infer(ast, expr_id)?;
                inferred_ty
            }
        };

        // types are interned so hopefully this is enough to check them
        if inferred_ty.0 != expected_ty.0 {
            self.unify.constraints.push((inferred_ty, expected_ty));

            // let err = tyerr(
            //     TypeErrorKind::WrongTypecheck {
            //         expected: self.get_type(expected_ty_id).clone(),
            //         actual: self.get_type(inferred_ty).clone(),
            //     },
            //     span,
            // );
            // Err(err)
            Ok(())
        } else {
            Ok(())
        }
    }
}

pub fn typecheck(ast: &mut Ast) -> Typechecker {
    let mut tc = Typechecker::default();

    // first pass: gather top level symbols (struct definitions and function signatures)
    for stmt in &ast.global_stmts {
        match tc.global_pass(ast, *stmt) {
            Ok(_) => {}
            Err(e) => tc.report_error(ast, e),
        }
    }

    // second pass, global variables and scan funtion bodies
    // typecheck expression blocks
    for stmt in &ast.global_stmts {
        match tc.typecheck(ast, *stmt) {
            Ok(_) => {}
            Err(e) => tc.report_error(ast, e),
        }
    }

    println!("{:?}", tc.types);
    println!("{:?}", tc.unify.constraints);

    if !tc.errors.is_empty() {
        return tc;
    }

    // final pass to assign final types to variables declarations
    for (stmt_id, ty_id) in &tc.vars_typed {
        let annot = type_to_annotation(*ty_id, &tc, ast);
        let (stmt, _) = ast.get_stmt_mut(*stmt_id);
        match stmt {
            Stmt::VarDecl(content) => {
                content.ty = Some(annot);
            }
            _ => unreachable!(),
        }
    }

    tc
}
