use crate::{TypeErrorKind::TypeAlreadyDeclared, intern::*, *};
use std::collections::{HashMap, HashSet};

// https://www.reddit.com/r/ProgrammingLanguages/comments/v5zwlw/resources_for_building_a_typechecker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/8kox21/help_implementing_a_type_checker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/1nhy0ef/how_do_i_make_a_real_typechecker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/1gkjkz4/how_to_implement_local_type_inference/

// https://ettolrach.com/blog/bidirectional_inference.html
// https://thunderseethe.dev/posts/how-to-choose-between-hm-and-bidir/
// https://thunderseethe.dev/posts/bidirectional-constraint-generation/
// https://smallcultfollowing.com/babysteps/blog/2017/03/25/unification-in-chalk-part-1/

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default)]
pub struct TypeId(pub Id);

pub struct Environment {
    vars_scopes: Vec<HashMap<IdentId, TypeId>>,
    ty_scopes: Vec<HashSet<TypeId>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            vars_scopes: vec![HashMap::new()],
            ty_scopes: vec![PRIMITIVES.iter().map(|p| p.1).collect()],
        }
    }
}

impl Environment {
    fn lookup_var(&self, name: IdentId) -> Option<TypeId> {
        for scope in self.vars_scopes.iter().rev() {
            if let Some(ty) = scope.get(&name) {
                return Some(*ty);
            }
        }
        None
    }

    fn lookup_global_var(&self, name: IdentId) -> Option<TypeId> {
        self.vars_scopes.first().unwrap().get(&name).copied()
    }

    fn bind_var(&mut self, name: IdentId, ty: TypeId, span: Span) -> TypeResult<()> {
        match self.vars_scopes.last_mut().unwrap().insert(name, ty) {
            Some(_) => Err(tyerr(TypeErrorKind::VarAlreadyDeclared(name), &span)),
            None => Ok(()),
        }
    }

    // TODO: we can shadow primitve names with types scopes. Thats a big problem
    fn ty_exists(&self, ty: TypeId) -> bool {
        self.ty_scopes.iter().rev().any(|scope| scope.contains(&ty))
    }

    fn with_var_scope<T>(&mut self, f: impl FnOnce(&mut Environment) -> T) -> T {
        self.vars_scopes.push(HashMap::new());
        let res = f(self);
        self.vars_scopes.pop();
        res
    }

    fn with_ty_scope<T>(&mut self, f: impl FnOnce(&mut Environment) -> T) -> T {
        self.ty_scopes.push(HashSet::new());
        let res = f(self);
        self.ty_scopes.pop();
        res
    }
}

#[derive(Debug, Clone, Hash)]
pub enum Type {
    Void,
    // Never,
    // Hole
    Int,
    Float,
    Bool,
    Generic(IdentId),
    Unknown(u32),
    Func { params: List<TypeId>, ret: TypeId },
    UserDef { args: List<TypeId>, name: IdentId },
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    pub generics: List<IdentId>,
    pub name: IdentId,
    pub fields: List<(IdentId, TypeId)>,
}

#[derive(Debug, Clone)]
pub struct FuncInfo {
    pub generics: List<IdentId>,
    pub params: List<TypeId>,
    pub ret: TypeId,
    pub body: Block,
}

const VOID_ID: TypeId = TypeId(0);
const BOOL_ID: TypeId = TypeId(1);
const INT_ID: TypeId = TypeId(2);
const FLOAT_ID: TypeId = TypeId(3);
const HOLE_ID: TypeId = TypeId(4);

const PRIMITIVES: &[(Type, TypeId)] = &[
    (Type::Void, VOID_ID),
    (Type::Bool, BOOL_ID),
    (Type::Int, INT_ID),
    (Type::Float, FLOAT_ID),
];

// TODO: better errors...
#[derive(Debug)]
pub enum TypeErrorKind {
    UndeclaredVar(IdentId),
    UndeclaredType(IdentId),
    WrongAnnotation { cause: Box<TypeError> },
    TypeMismatch { expected: TypeId, actual: TypeId },
    InfiniteType { source: TypeId, contains: TypeId },
    RequiredAnnotation,
    UnresolvedType,

    InvalidStmtInGlobal,
    InvalidStmtInBlock,

    WrongCallArgumentsCount { expected: usize, actual: usize },

    WrongTypeArgumentsCount { expected: usize, actual: usize },

    WrongStructLiteralCount { expected: usize, actual: usize },
    WrongStructLiteralField(IdentId),
    TypeAlreadyDeclared(IdentId),
    VarAlreadyDeclared(IdentId),

    DuplicateParamsInFunction(IdentId),
    DuplicateGenericsInFunction(IdentId),
    DuplicateFieldsInStruct(IdentId),
    DuplicateGenericsInStruct(IdentId),

    CallingWithWrongType(TypeId),
    IndexingWithWrongType,

    ValueNotAssignable,
    ArgumentsInGenericParameter,
    InvalidControlStmtInBlock,
    NoReturnInFunctionBlock,
    InvalidLastStmtInBlock,

    SelfReferentialStruct(IdentId),
    MissingMainFunction,
}
impl TypeErrorKind {}

type TypeError = Spanned<TypeErrorKind>;
pub fn tyerr(kind: TypeErrorKind, span: &Span) -> TypeError {
    (kind, *span)
}

type TypeResult<T> = Result<T, TypeError>;

// TODO: temporary solution
// Basically a simple Union-find, not optimized
#[derive(Default, Debug)]
struct Unification(Vec<TypeId>);
impl Unification {
    // pub fn fresh(&mut self) -> u32 {
    //     let id = self.0.len() as u32;
    //     self.0.push(None);
    //     id
    // }
}

pub struct Typechecker {
    pub funcs: HashMap<TypeId, FuncInfo>,
    pub structs: HashMap<IdentId, StructInfo>,

    pub funcs_instances: Vec<FuncInfo>, // todo: make these another kind of struct
    pub structs_instances: Vec<StructInfo>, // todo: make these another kind of struct

    pub types: Interner<Type>,
    substs: Unification,

    loop_depth: u32,
    func_ret: Option<TypeId>,

    untyped_vars: Vec<(StmtId, TypeId)>,
    pub stmts_types: HashMap<StmtId, TypeId>, // TODO: temporary solution

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
            types,

            funcs: Default::default(),
            structs: Default::default(),

            funcs_instances: Default::default(),
            structs_instances: Default::default(),

            substs: Default::default(),

            loop_depth: 0,
            func_ret: None,

            untyped_vars: vec![],
            stmts_types: Default::default(),

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

        Type::Int | Type::Bool => TyAnnot::Type {
            // we treat bools as ints in C
            name: IdentId(ast.idents.add("int")),
            args: Default::default(),
        },

        Type::Float => TyAnnot::Type {
            name: IdentId(ast.idents.add("float")),
            args: Default::default(),
        },

        Type::Generic(ident) => TyAnnot::Type {
            name: *ident,
            args: Default::default(),
        },

        Type::Unknown(_) => {
            unreachable!("can't convert a type var to a type annotation")
        }

        Type::Func { params, ret, .. } => {
            let mut list = Vec::new();
            for param in params {
                list.push(type_to_annotation(*param, tc, ast));
            }

            let ret = type_to_annotation(*ret, tc, ast);

            TyAnnot::Func {
                params: list.into(),
                ret: Some(ret),
            }
        }
        Type::UserDef { name, args } => TyAnnot::Type {
            name: *name,
            args: args
                .iter()
                .map(|a| type_to_annotation(*a, tc, ast))
                .collect(),
        },
    };

    ast.add_ty_annot(annot)
}

impl Typechecker {
    fn add_type(&mut self, env: &mut Environment, ty: Type) -> TypeId {
        let id = self.types.add(ty);
        let id = TypeId(id as Id);
        env.ty_scopes.last_mut().unwrap().insert(id);
        id
    }

    fn add_unknown_ty(&mut self, env: &mut Environment) -> TypeId {
        // let id = self.substs.fresh();
        // self.add_type(env, Type::Unknown(id))

        let tyvar = self.substs.0.len() as u32;
        let id = self.add_type(env, Type::Unknown(tyvar));
        self.substs.0.push(id);
        id
    }

    fn get_type(&self, id: TypeId) -> &Type {
        self.types.get(id.0)
    }

    // TODO: is this even needed? if we really want scoped types, shouldn't we assign them and unique id or hash so that they dont collide with others?
    fn lookup_ty(&self, env: &Environment, ty: &Type) -> Option<TypeId> {
        self.types
            .lookup(ty)
            .map(|id| TypeId(id))
            .filter(|id| env.ty_exists(*id))
    }

    fn report_error(&mut self, e: TypeError) {
        self.errors.push(e);
    }

    // usually called "resolve"
    fn add_type_from_annotation(
        &mut self,
        env: &mut Environment,
        ast: &Ast,
        annot_id: TyAnnotId,
    ) -> TypeResult<TypeId> {
        let (annot, span) = ast.get_annot_n_span(annot_id);
        let ty = match annot {
            TyAnnot::Type { name, args } => {
                let str = ast.get_str(*name);
                match str {
                    "bool" => return Ok(BOOL_ID),
                    "int" => return Ok(INT_ID),
                    "float" => return Ok(FLOAT_ID),
                    _ => {
                        // it is either a userdef or generic parameter
                        // TODO: this should be scoped, do a type lookup
                        if let Some(struct_info) = self.structs.get(name) {
                            if struct_info.generics.len() != args.len() {
                                Err(tyerr(
                                    TypeErrorKind::WrongTypeArgumentsCount {
                                        expected: struct_info.generics.len(),
                                        actual: args.len(),
                                    },
                                    span,
                                ))?
                            }

                            let mut list = Vec::new();
                            for arg in args {
                                let ty_id = self.add_type_from_annotation(env, ast, *arg)?;
                                list.push(ty_id);
                            }

                            Type::UserDef {
                                args: list.into(),
                                name: *name,
                            }
                        } else {
                            // it has to be a generic parameter
                            if let Some(id) = self.lookup_ty(env, &Type::Generic(*name)) {
                                return Ok(id);
                            } else {
                                Err(tyerr(TypeErrorKind::UndeclaredType(*name), span))?
                            }
                        }
                    }
                }
            }

            TyAnnot::Array { inner, size } => {
                todo!("array not implemented")
            }

            TyAnnot::Func { params, ret } => {
                let mut list = Vec::new();
                for param in params {
                    let id = self.add_type_from_annotation(env, ast, *param)?;
                    list.push(id);
                }

                let ret = if let Some(ret) = ret {
                    self.add_type_from_annotation(env, ast, *ret)?
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
                    let id = self.add_type_from_annotation(env, ast, *elem)?;
                    list.push(id);
                }
                todo!(" tuple not implemented")
            }
        };

        Ok(self.add_type(env, ty))
    }

    fn var_decl(
        &mut self,
        env: &mut Environment,
        ast: &Ast,
        var: &VarContent,
        stmt_id: StmtId,
        span: Span,
    ) -> TypeResult<()> {
        let final_ty = if let Some(ty_annot) = var.ty {
            let annot_ty_id = self.add_type_from_annotation(env, ast, ty_annot)?;
            self.check(env, ast, var.rhs, annot_ty_id)
                .map_err(|e| tyerr(TypeErrorKind::WrongAnnotation { cause: Box::new(e) }, &span))?;

            annot_ty_id
        } else {
            let ty = self.infer(env, ast, var.rhs)?;
            self.untyped_vars.push((stmt_id, ty));
            ty
        };

        env.bind_var(var.name, final_ty, span)?;
        Ok(())
    }

    fn first_userdefs_pass(
        &mut self,
        _env: &mut Environment,
        ast: &Ast,
        stmt_id: StmtId,
    ) -> TypeResult<()> {
        // this pass only collects the structs names and arguments
        // a struct can reference another struct which might be not yet declared

        let (stmt, span) = ast.get_stmt_n_span(stmt_id);
        match stmt {
            Stmt::StructDecl(content) => {
                if self.structs.contains_key(&content.name) {
                    return Err(tyerr(
                        TypeErrorKind::TypeAlreadyDeclared(content.name),
                        span,
                    ));
                }

                if has_duplicates(&content.generics, |g| *g) {
                    return Err(tyerr(
                        TypeErrorKind::DuplicateGenericsInStruct(content.name),
                        span,
                    ));
                }

                self.structs.insert(
                    content.name,
                    StructInfo {
                        generics: content.generics.clone(),
                        name: content.name,
                        fields: Default::default(),
                    },
                );
            }

            _ => {}
        }

        Ok(())
    }

    fn second_decl_pass(
        &mut self,
        env: &mut Environment,
        ast: &Ast,
        stmt_id: StmtId,
    ) -> TypeResult<()> {
        let (stmt, span) = ast.get_stmt_n_span(stmt_id);
        match stmt {
            Stmt::FuncDecl(content) => {
                if env.lookup_global_var(content.name).is_some() {
                    return Err(tyerr(TypeErrorKind::VarAlreadyDeclared(content.name), span));
                }

                // TODO: should we check if generic names collides with primitive name?
                if has_duplicates(&content.generics, |g| *g) {
                    return Err(tyerr(
                        TypeErrorKind::DuplicateGenericsInFunction(content.name),
                        span,
                    ));
                }

                // // TODO: probably not needed anymore. bind already checks for this
                // if has_duplicates(&content.params, |p| p.0) {
                //     return Err(tyerr(
                //         TypeErrorKind::DuplicateParamsInFunction(content.name),
                //         span,
                //     ));
                // }

                let (params, ret) = env.with_ty_scope(|env| -> TypeResult<_> {
                    for generic in &content.generics {
                        self.add_type(env, Type::Generic(*generic));
                    }

                    let ret = if let Some(ret) = content.ret {
                        self.add_type_from_annotation(env, ast, ret)?
                    } else {
                        VOID_ID
                    };

                    let mut params = Vec::new();
                    for param in &content.params {
                        params.push(self.add_type_from_annotation(env, ast, param.1)?);
                    }

                    Ok((params, ret))
                })?;

                let ty = Type::Func {
                    params: params.clone().into(),
                    ret,
                };

                let ty_id = self.add_type(env, ty);

                self.funcs.insert(
                    ty_id,
                    FuncInfo {
                        generics: content.generics.clone(),
                        params: params.into(),
                        ret,
                        body: content.code.clone(),
                    },
                );

                // we cant shadow global function names because we checked for it earlier
                env.bind_var(content.name, ty_id, *span)?;
            }

            Stmt::StructDecl(content) => {
                if has_duplicates(&content.fields, |f| f.0) {
                    return Err(tyerr(
                        TypeErrorKind::DuplicateFieldsInStruct(content.name),
                        span,
                    ));
                }

                let fields = env.with_ty_scope(|env| -> TypeResult<_> {
                    for generic in &content.generics {
                        self.add_type(env, Type::Generic(*generic));
                    }

                    let mut fields = Vec::new();
                    for field in &content.fields {
                        let ty_id = self.add_type_from_annotation(env, ast, field.1)?;
                        self.ty_self_referential_check(ty_id, content.name)
                            .map_err(|_| {
                                tyerr(TypeErrorKind::SelfReferentialStruct(content.name), span)
                            })?;

                        fields.push((field.0, ty_id));
                    }

                    Ok(fields)
                });

                let info = self
                    .structs
                    .get_mut(&content.name)
                    .ok_or_else(|| tyerr(TypeErrorKind::UndeclaredType(content.name), span))?;

                info.fields = fields?.into();
            }
            _ => {}
        }

        Ok(())
    }

    fn third_global_pass(
        &mut self,
        env: &mut Environment,
        ast: &Ast,
        stmt_id: StmtId,
    ) -> TypeResult<()> {
        let (stmt, span) = ast.get_stmt_n_span(stmt_id);
        match stmt {
            Stmt::VarDecl(content) => self.var_decl(env, ast, content, stmt_id, *span)?,

            Stmt::FuncDecl(content) => {
                let ty_id = env
                    .lookup_global_var(content.name)
                    .ok_or_else(|| tyerr(TypeErrorKind::UndeclaredVar(content.name), span))?;

                let info = self
                    .funcs
                    .get(&ty_id)
                    .expect("function should have been checked during the previous pass");

                let ret_expected = info.ret;
                self.func_ret = Some(ret_expected);

                let params = info.params.clone();
                let generics = info.generics.clone();
                let ret_actual = env.with_ty_scope(|env| {
                    env.with_var_scope(|env| {
                        for generic in generics {
                            self.add_type(env, Type::Generic(generic));
                        }

                        for (param_ty, param_content) in params.iter().zip(content.params.iter()) {
                            env.bind_var(param_content.0, *param_ty, *span)?;
                        }

                        self.infer_block(env, ast, &content.code)
                    })
                });

                self.func_ret = None;
                ret_actual?;
            }

            Stmt::StructDecl(_) => {} // Do nothing, already done in previous pass
            Stmt::Continue | Stmt::Break => {
                Err(tyerr(TypeErrorKind::InvalidControlStmtInBlock, span))?
            }

            Stmt::Expr(_) => Err(tyerr(TypeErrorKind::InvalidStmtInGlobal, span))?,
        }

        Ok(())
    }

    // TODO: this fails IMMEDIATELY, add statement level granularity for errors
    // TODO: please refactor this motherfucker
    fn infer_block(
        &mut self,
        env: &mut Environment,
        ast: &Ast,
        block: &Block,
    ) -> TypeResult<TypeId> {
        // iter over all except the last
        // this is important because otherwise we would add two different type variables for the last expression and incurr in a type error
        for stmt_id in block.list.iter().rev().skip(1).rev().copied() {
            let (stmt, span) = ast.get_stmt_n_span(stmt_id);
            match stmt {
                Stmt::VarDecl(content) => self.var_decl(env, ast, content, stmt_id, *span)?,

                Stmt::Expr(expr_id) => {
                    let (expr, _) = ast.get_expr_n_span(*expr_id);

                    match expr {
                        Expr::Return(expr_id) => match &self.func_ret {
                            None => {
                                // this block isn't inside a function but is doing a return
                                return Err(tyerr(TypeErrorKind::InvalidControlStmtInBlock, span));
                            }
                            Some(current_func_ret) => {
                                // all return statements should have the same type as the function's.
                                self.check(env, ast, *expr_id, *current_func_ret)?;
                            }
                        },

                        _ => _ = self.infer(env, ast, *expr_id)?,
                    }
                }

                Stmt::Break | Stmt::Continue => {
                    if self.loop_depth == 0 {
                        Err(tyerr(TypeErrorKind::InvalidControlStmtInBlock, span))?
                    }
                }

                Stmt::FuncDecl(_) | Stmt::StructDecl(_) => {
                    Err(tyerr(TypeErrorKind::InvalidStmtInBlock, span))?
                }
            };
        }

        // check last expression
        // if block has no elements, its just void

        let mut ret_ty = None;
        if let Some(last_id) = block.list.last() {
            let (stmt, span) = ast.get_stmt_n_span(*last_id);

            match stmt {
                Stmt::Expr(expr_id) => match &self.func_ret {
                    Some(current_func_ret) => {
                        // we dont care if return type is void
                        if *current_func_ret != VOID_ID {
                            self.check(env, ast, *expr_id, *current_func_ret)?;
                        } else {
                            self.infer(env, ast, *expr_id)?;
                        }
                    }
                    None => ret_ty = Some(self.infer(env, ast, *expr_id)?),
                },

                _ => {
                    if let Some(current_func_ret) = &self.func_ret {
                        // we dont care if return type is void
                        if *current_func_ret != VOID_ID {
                            return Err(tyerr(TypeErrorKind::NoReturnInFunctionBlock, span));
                        } else {
                            match stmt {
                                Stmt::VarDecl(content) => {
                                    self.var_decl(env, ast, content, *last_id, *span)?
                                }

                                Stmt::Expr(expr_id) => {
                                    let (expr, _) = ast.get_expr_n_span(*expr_id);

                                    match expr {
                                        Expr::Return(expr_id) => match &self.func_ret {
                                            None => {
                                                // this block isn't inside a function but is doing a return
                                                return Err(tyerr(
                                                    TypeErrorKind::InvalidControlStmtInBlock,
                                                    span,
                                                ));
                                            }
                                            Some(current_func_ret) => {
                                                // all return statements should have the same type as the function's.
                                                self.check(env, ast, *expr_id, *current_func_ret)?;
                                            }
                                        },

                                        _ => _ = self.infer(env, ast, *expr_id)?,
                                    }
                                }

                                Stmt::Break | Stmt::Continue => {
                                    if self.loop_depth == 0 {
                                        Err(tyerr(TypeErrorKind::InvalidControlStmtInBlock, span))?
                                    }
                                }

                                Stmt::FuncDecl(_) | Stmt::StructDecl(_) => {
                                    Err(tyerr(TypeErrorKind::InvalidStmtInBlock, span))?
                                }
                            };
                        }
                    } else {
                        return Err(tyerr(TypeErrorKind::InvalidLastStmtInBlock, span));
                    }
                }
            }
        }

        // if we didn't find any return types, this block has type void
        Ok(ret_ty.unwrap_or(VOID_ID))
    }

    fn infer(&mut self, env: &mut Environment, ast: &Ast, expr: ExprId) -> TypeResult<TypeId> {
        let (expr, span) = ast.get_expr_n_span(expr);
        let id = match expr {
            Expr::Empty => VOID_ID,
            Expr::BoolLiteral(_) => BOOL_ID,
            Expr::IntLiteral(_) => INT_ID,
            Expr::FloatLiteral(_) => FLOAT_ID,

            Expr::Grouping(expr) => self.infer(env, ast, *expr)?,

            // TODO: be careful. we can assign type variables to a variable (just think of a generic function)
            // A generic function is not instantiated, so what do we really assign to the variable?
            Expr::Variable(name) => {
                let ty = env
                    .lookup_var(*name)
                    .ok_or_else(|| tyerr(TypeErrorKind::UndeclaredVar(*name), span))?;

                match self.get_type(ty) {
                    // if we're getting a generic from a variable, we are inside a function declaration. WE DON'T HAVE to instantiate it
                    Type::Generic(_) => ty,
                    _ => self.ty_instantiate(env, ty, &mut HashMap::new()),
                }
            }

            Expr::Return(id) => self.infer(env, ast, *id)?,

            // TODO: right now, no way to check if operand amits the operation between types
            Expr::Unary { op, rhs } => self.infer(env, ast, *rhs)?,
            Expr::Binary { op, lhs, rhs } => {
                let lhs_ty = self.infer(env, ast, *lhs)?;
                self.check(env, ast, *rhs, lhs_ty)?;
                lhs_ty
            }
            Expr::Ternary {
                cond,
                first,
                second,
            } => {
                self.check(env, ast, *cond, BOOL_ID)?;
                let ty = self.infer(env, ast, *first)?;
                self.check(env, ast, *second, ty)?;
                ty
            }

            Expr::IfElse {
                cond,
                first,
                second,
            } => {
                self.check(env, ast, *cond, BOOL_ID)?;

                if let Some(second) = second {
                    let first_ty = self.infer(env, ast, *first)?;
                    self.check(env, ast, *second, first_ty)?;
                    first_ty
                } else {
                    self.check(env, ast, *first, VOID_ID)?;
                    VOID_ID
                }
            }

            Expr::While { cond, code } => {
                self.check(env, ast, *cond, BOOL_ID)?;
                self.loop_depth += 1;
                let res = self.check(env, ast, *code, VOID_ID);
                self.loop_depth -= 1;
                res?;
                VOID_ID
            }

            Expr::Call { callee, args } => {
                let ty_id = self.infer(env, ast, *callee)?;
                match self.get_type(ty_id) {
                    Type::Func { params, ret } => {
                        if params.len() != args.len() {
                            return Err(tyerr(
                                TypeErrorKind::WrongCallArgumentsCount {
                                    expected: params.len(),
                                    actual: args.len(),
                                },
                                span,
                            ));
                        }

                        let ret = *ret;
                        for (param, arg) in params.clone().iter().zip(args.iter()) {
                            self.check(env, ast, *arg, *param)?;
                        }

                        ret
                    }

                    _ => Err(tyerr(TypeErrorKind::CallingWithWrongType(ty_id), span))?,
                }
            }

            Expr::StructLiteral(name, args) => {
                if let Some(info) = self.structs.get(name).cloned() {
                    if info.fields.len() != args.len() {
                        return Err(tyerr(
                            TypeErrorKind::WrongStructLiteralCount {
                                expected: info.fields.len(),
                                actual: args.len(),
                            },
                            span,
                        ));
                    }

                    let mut generics = Vec::new();
                    for ((field_name, field_ty), (value_name, value)) in
                        info.fields.iter().zip(args.iter())
                    {
                        if field_name != value_name {
                            return Err(tyerr(
                                TypeErrorKind::WrongStructLiteralField(*value_name),
                                span,
                            ));
                        }

                        match self.get_type(*field_ty) {
                            Type::Generic(_) => {
                                let value_ty = self.infer(env, ast, *value)?;
                                generics.push(value_ty);
                            }
                            _ => {
                                self.check(env, ast, *value, *field_ty)?;
                            }
                        };
                    }

                    self.add_type(
                        env,
                        Type::UserDef {
                            args: generics.into(),
                            name: *name,
                        },
                    )
                } else {
                    Err(tyerr(TypeErrorKind::UndeclaredType(*name), span))?
                }
            }

            Expr::Indexing { src, idx } => {
                todo!("array not implemented")
            }

            Expr::Member { src, member } => {
                todo!("structs not implemented yet")
            }

            Expr::Block(block) => env.with_var_scope(|env| self.infer_block(env, ast, block))?,
        };

        Ok(id)
    }

    // TODO: might need to add an erorr variant util so we know which kind of type mismatch weve vot
    fn check(
        &mut self,
        env: &mut Environment,
        ast: &Ast,
        expr_id: ExprId,
        expected_ty: TypeId,
    ) -> TypeResult<TypeId> {
        let (_, span) = ast.get_expr_n_span(expr_id);

        let inferred_ty = self.infer(env, ast, expr_id)?;
        self.unify(env, expected_ty, inferred_ty, span)?;

        Ok(inferred_ty)
    }

    fn unify(
        &mut self,
        env: &Environment,
        ty_id_a: TypeId,
        ty_id_b: TypeId,
        span: &Span,
    ) -> TypeResult<()> {
        let ty_id_a = self.ty_normalize(ty_id_a);
        let ty_id_b = self.ty_normalize(ty_id_b);

        // TODO: moving the types interner to environment would not require cloning here anymore, but thats a design choice
        let ty_a = self.get_type(ty_id_a).clone();
        let ty_b = self.get_type(ty_id_b).clone();

        match (ty_a, ty_b) {
            (Type::Unknown(x), _) => {
                if self.ty_occurs_check(ty_id_b, x) {
                    return Err(tyerr(
                        TypeErrorKind::InfiniteType {
                            source: ty_id_b,
                            contains: ty_id_a,
                        },
                        span,
                    ));
                } else {
                    self.ty_substitute(x, ty_id_b);
                }
            }
            (_, Type::Unknown(x)) => {
                if self.ty_occurs_check(ty_id_a, x) {
                    return Err(tyerr(
                        TypeErrorKind::InfiniteType {
                            source: ty_id_a,
                            contains: ty_id_b,
                        },
                        span,
                    ));
                } else {
                    self.ty_substitute(x, ty_id_a);
                }
            }

            (Type::Void, Type::Void)
            | (Type::Int, Type::Int)
            | (Type::Bool, Type::Bool)
            | (Type::Float, Type::Float) => {}

            (Type::Generic(x), Type::Generic(y)) => {
                if x != y {
                    return Err(tyerr(
                        TypeErrorKind::TypeMismatch {
                            expected: ty_id_a,
                            actual: ty_id_b,
                        },
                        span,
                    ));
                }
            }

            (
                Type::Func {
                    params: params_l,
                    ret: ret_l,
                    ..
                },
                Type::Func {
                    params: params_r,
                    ret: ret_r,
                    ..
                },
            ) => {
                for (param_l, param_r) in params_l.iter().zip(params_r.iter()) {
                    self.unify(env, *param_l, *param_r, span)?;
                }

                self.unify(env, ret_l, ret_r, span)?;
            }

            (
                Type::UserDef {
                    name: name_l,
                    args: args_l,
                    ..
                },
                Type::UserDef {
                    name: name_r,
                    args: args_r,
                    ..
                },
            ) => {
                if name_l != name_r {
                    return Err(tyerr(
                        TypeErrorKind::TypeMismatch {
                            expected: ty_id_a,
                            actual: ty_id_b,
                        },
                        span,
                    ));
                }

                for (arg_l, arg_r) in args_l.iter().zip(args_r.iter()) {
                    self.unify(env, *arg_l, *arg_r, span)?;
                }
            }

            _ => {
                return Err(tyerr(
                    TypeErrorKind::TypeMismatch {
                        expected: ty_id_a,
                        actual: ty_id_b,
                    },
                    span,
                ));
            }
        }

        Ok(())
    }

    // get the solved type from a type variable
    fn ty_normalize(&self, ty: TypeId) -> TypeId {
        match self.get_type(ty) {
            Type::Unknown(x) => {
                // if let Some(parent) = self.substs.0[*x as usize] {
                //     self.ty_normalize(parent)
                // } else {
                //     ty
                // }
                let next = self.substs.0[*x as usize];
                if next == ty {
                    ty
                } else {
                    self.ty_normalize(next)
                }
            }

            _ => ty,
        }
    }

    // substitute an equality constraint between type variable and monomorphic type
    fn ty_substitute(&mut self, type_var: u32, ty: TypeId) {
        // self.substs.0[type_var as usize] = Some(ty);
        self.substs.0[type_var as usize] = ty;
    }

    fn ty_solve(&self, ty: TypeId) -> TypeId {
        let ty = self.ty_normalize(ty);
        match self.get_type(ty) {
            Type::Void | Type::Bool | Type::Int | Type::Float => ty,
            Type::Func { params, ret } => {
                params.iter().for_each(|p| {
                    self.ty_solve(*p);
                });
                self.ty_solve(*ret)
            }
            Type::UserDef { args, .. } => {
                args.iter().for_each(|a| {
                    self.ty_solve(*a);
                });
                ty
            }
            Type::Generic(_) => ty,
            Type::Unknown(_) => ty,
        }
    }

    // instantiate a generic type to a concrete one
    // generic arguments not solved are converted to type variables
    fn ty_instantiate(
        &mut self,
        env: &mut Environment,
        ty: TypeId,
        generics: &mut HashMap<IdentId, TypeId>,
    ) -> TypeId {
        match self.get_type(ty).clone() {
            Type::Func { params, ret, .. } => {
                let instantiated = params
                    .iter()
                    .map(|p| self.ty_instantiate(env, *p, generics))
                    .collect();
                let ret_instantiated = self.ty_instantiate(env, ret, generics);

                self.add_type(
                    env,
                    Type::Func {
                        params: instantiated,
                        ret: ret_instantiated,
                    },
                )
            }

            Type::UserDef { name, args } => {
                let instanciated = args
                    .iter()
                    .map(|a| self.ty_instantiate(env, *a, generics))
                    .collect();

                self.add_type(
                    env,
                    Type::UserDef {
                        args: instanciated,
                        name,
                    },
                )
            }

            Type::Generic(name) => {
                if let Some(concrete) = generics.get(&name) {
                    *concrete
                } else {
                    let fresh = self.add_unknown_ty(env);
                    generics.insert(name, fresh);
                    fresh
                }
            }

            _ => ty,
        }
    }

    // check for self-referencing type variables
    fn ty_occurs_check(&self, ty: TypeId, ty_var: u32) -> bool {
        match self.get_type(ty) {
            Type::Void | Type::Bool | Type::Int | Type::Float => false,
            Type::Generic(_) => false,
            Type::Func { params, ret } => {
                params.iter().any(|p| self.ty_occurs_check(*p, ty_var))
                    || self.ty_occurs_check(*ret, ty_var)
            }
            Type::UserDef { args, .. } => args.iter().any(|a| self.ty_occurs_check(*a, ty_var)),
            Type::Unknown(inner_ty_var) => *inner_ty_var == ty_var,
        }
    }

    fn ty_self_referential_check(&self, curr: TypeId, parent: IdentId) -> Result<(), ()> {
        match self.get_type(curr) {
            Type::Void | Type::Bool | Type::Int | Type::Float => Ok(()),
            Type::Generic(_) => Ok(()),
            Type::Unknown(_) => Ok(()),

            Type::Func { params, ret } => {
                for param in params {
                    self.ty_self_referential_check(*param, parent)?;
                }

                self.ty_self_referential_check(*ret, parent)
            }
            Type::UserDef { args, name } => {
                if *name == parent {
                    return Err(());
                }

                for arg in args {
                    self.ty_self_referential_check(*arg, parent)?;
                }

                Ok(())
            }
        }
    }

    fn ty_is_generic(&self, ty: TypeId) -> bool {
        match self.get_type(ty) {
            Type::Func { params, ret } => {
                params.iter().any(|p| self.ty_is_generic(*p)) || self.ty_is_generic(*ret)
            }
            Type::UserDef { args, .. } => args.iter().any(|a| self.ty_is_generic(*a)),

            Type::Unknown(_) | Type::Generic(_) => true,

            Type::Void | Type::Bool | Type::Int | Type::Float => false,
        }
    }
}

pub fn typecheck(ast: &mut Ast) -> Typechecker {
    let mut tc = Typechecker::default();
    let mut env = Environment::default();

    // first pass: gather userdefs names and arguments only
    for stmt in &ast.global_stmts {
        if let Err(e) = tc.first_userdefs_pass(&mut env, ast, *stmt) {
            tc.report_error(e);
        }
    }

    // second pass: gather top level symbols (struct fields and function signatures)
    for stmt in &ast.global_stmts {
        if let Err(e) = tc.second_decl_pass(&mut env, ast, *stmt) {
            tc.report_error(e);
        }
    }

    // third pass, global variables and scan function bodies
    for stmt in &ast.global_stmts {
        if let Err(e) = tc.third_global_pass(&mut env, ast, *stmt) {
            tc.report_error(e);
        }
    }

    // TODO: check for any unresolved type
    if tc
        .substs
        .0
        .iter()
        .any(|t| matches!(tc.get_type(*t), Type::Unknown(_)))
    {
        tc.report_error(tyerr(
            TypeErrorKind::UnresolvedType,
            &Span { start: 0, end: 0 },
        ));
    }

    if !tc.errors.is_empty() {
        return tc;
    }

    // zonking
    for (stmt_id, ty_id) in &tc.untyped_vars {
        let annot = type_to_annotation(tc.ty_solve(*ty_id), &tc, ast);

        let Stmt::VarDecl(content) = ast.get_stmt_mut(*stmt_id) else {
            unreachable!()
        };

        content.ty = Some(annot);
    }

    //     for stmt_id in (0..ast.stmts.len()).map(|i| StmtId(i as Id)) {
    //         match ast.get_stmt(stmt_id) {
    //             Stmt::VarDecl(content) => {}
    //             Stmt::Expr(expr) => {}
    //
    //             _ => {}
    //         }
    //     }

    // funcs monomorphization
    for id in (0..tc.types.len()).map(|i| TypeId(i as Id)) {
        let ty = tc.get_type(id);
        match ty {
            Type::Func { params, ret } => {
                let params = params.clone();
                let ret = *ret;

                if params.iter().any(|t| tc.ty_is_generic(*t)) || tc.ty_is_generic(ret) {
                    continue;
                }

                let info = tc.funcs.get(&id).unwrap().clone();
                debug_assert_eq!(params.len(), info.params.len());
                if info.generics.is_empty() {
                    tc.funcs_instances.push(info);
                    continue;
                }

                let substs: HashMap<_, _> = info
                    .params
                    .iter()
                    .zip(params.iter())
                    .filter_map(|(decl_param, call_param)| match tc.get_type(*decl_param) {
                        Type::Generic(x) => Some((*x, *call_param)),
                        _ => None,
                    })
                    .collect();

                let mut body = Vec::new();
                for stmt_id in &info.body.list {
                    match ast.get_stmt(*stmt_id) {
                        Stmt::VarDecl(content) => {
                            let annot_id =
                                content.ty.expect("zoking should have already been done");

                            match ast.get_annot(annot_id) {
                                // if the type is named...
                                TyAnnot::Type { name, .. } => {
                                    // ... and it turns out its a generic ...
                                    let new_stmt = if let Some(sub) = substs.get(name) {
                                        let content = content.clone();
                                        let annot = type_to_annotation(*sub, &tc, ast);

                                        // ... make the substitution!
                                        ast.add_stmt(Stmt::VarDecl(VarContent {
                                            ty: Some(annot),
                                            ..content
                                        }))
                                    } else {
                                        *stmt_id
                                    };

                                    body.push(new_stmt);
                                }

                                _ => body.push(*stmt_id),
                            }
                        }

                        _ => body.push(*stmt_id), // TODO: this makes statements not conescutive anymore, here a new statements should be cloned and added again
                    }
                }

                let instance = FuncInfo {
                    body: Block { list: body.into() },
                    ..info
                };
                tc.funcs_instances.push(instance);
            }

            _ => {}
        }
    }

    // structs monomorphization
    for id in 0..tc.types.len() {
        match tc.get_type(TypeId(id as Id)) {
            Type::UserDef { name, args } => {
                if let Some(info) = tc.structs.get(&name) {
                    debug_assert_eq!(args.len(), info.generics.len());

                    if info.generics.is_empty() {
                        tc.structs_instances.push(info.clone());
                        continue;
                    }

                    let mut substs: HashMap<_, _> = info
                        .generics
                        .iter()
                        .copied()
                        .zip(args.iter().copied())
                        .collect();
                    let mut concrete_fields = info.fields.clone();

                    let name = info.name;
                    for field in concrete_fields.iter_mut() {
                        field.1 = tc.ty_instantiate(&mut env, field.1, &mut substs);
                    }

                    let instance = StructInfo {
                        generics: Default::default(),
                        name,
                        fields: concrete_fields,
                    };

                    tc.structs_instances.push(instance);
                }
            }
            _ => {}
        }
    }

    println!("{:?}", tc.funcs_instances);
    println!("{:?}", tc.structs_instances);

    tc
}
