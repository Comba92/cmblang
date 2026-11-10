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

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default)]
pub struct TypeId(pub Id);

pub struct Environment {
    vars_scopes: Vec<HashMap<IdentId, TypeId>>,
    ty_scopes: Vec<HashSet<TypeId>>,
}

impl Default for Environment {
    fn default() -> Self {
        let mut types = Interner::new();
        for primitive in PRIMITIVES {
            let id = types.add(primitive.0.clone());
            debug_assert_eq!(id, primitive.1.0);
        }

        Self {
            vars_scopes: vec![HashMap::new()],
            ty_scopes: vec![PRIMITIVES.iter().map(|p| p.1).collect()],
        }
    }
}

impl Environment {
    // ---- variables ----
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

    // TODO: check if value already exists
    fn bind_var(&mut self, name: IdentId, ty: TypeId) {
        self.vars_scopes.last_mut().unwrap().insert(name, ty);
    }

    fn ty_exists(&self, ty: TypeId) -> bool {
        self.ty_scopes.iter().rev().any(|scope| scope.contains(&ty))
    }

    // ---- scopes ----
    // push/pop are only reachable through these, so they can't get unbalanced.
    // the callback receives the environment back, since the caller's closure
    // can't capture it while we hold &mut self.
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
    Integer,
    Float,
    Bool,
    Generic(IdentId),
    Unknown(u32),
    Func { params: List<TypeId>, ret: TypeId },
    UserDef { args: List<TypeId>, name: IdentId },
}

#[derive(Debug, Clone)]
pub struct StructInfo {
    generics: List<IdentId>,
    name: IdentId,
    fields: List<(IdentId, TypeId)>,
}

#[derive(Debug, Clone)]
pub struct FuncInfo {
    generics: List<IdentId>,
    params: List<TypeId>,
    ret: TypeId,
}

const VOID_ID: TypeId = TypeId(0);
const BOOL_ID: TypeId = TypeId(1);
const INT_ID: TypeId = TypeId(2);
const FLOAT_ID: TypeId = TypeId(3);
const HOLE_ID: TypeId = TypeId(4);

const PRIMITIVES: &[(Type, TypeId)] = &[
    (Type::Void, VOID_ID),
    (Type::Bool, BOOL_ID),
    (Type::Integer, INT_ID),
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

    InvalidStmtInGlobal,
    InvalidStmtInBlock,

    WrongCallArgumentsCount { expected: usize, actual: usize },

    WrongTypeArgumentsCount { expected: usize, actual: usize },

    WrongStructLiteralCount { expected: usize, actual: usize },
    WrongStructLiteralField(IdentId),

    DuplicateParamsInFunction(IdentId),
    DuplicateGenericsInFunction(IdentId),
    DuplicateFieldsInStruct(IdentId),
    DuplicateGenericsInStruct(IdentId),

    CallingWithWrongType(TypeId),
    IndexingWithWrongType,

    ValueNotAssignable,
    ArgumentsInGenericParameter,
    BlockWithoutReturnExpr(Option<TypeId>),
    InvalidControlStmtInBlock,

    SelfReferentialStruct(IdentId),
}
impl TypeErrorKind {}

type TypeError = Spanned<TypeErrorKind>;
pub fn tyerr(kind: TypeErrorKind, span: &Span) -> TypeError {
    (kind, *span)
}

type TypeResult<T> = Result<T, TypeError>;

// TODO: temporary solution
// Basically a simple Union-find, not optimized
#[derive(Default)]
struct Unification(Vec<Option<TypeId>>);
impl Unification {
    pub fn fresh(&mut self) -> u32 {
        let id = self.0.len() as u32;
        self.0.push(None);
        id
    }
}

pub struct Typechecker {
    pub funcs: HashMap<IdentId, FuncInfo>,
    pub structs: HashMap<IdentId, StructInfo>,
    pub types: Interner<Type>,

    substs: Unification,

    loop_depth: u32,

    vars_typed: Vec<(StmtId, TypeId)>, // TODO: temporary solution, find fix
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

            substs: Default::default(),
            loop_depth: 0,

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

        Type::Integer | Type::Bool => TyAnnot::Type {
            // we treat bools as ints in C
            name: IdentId(ast.idents.add("int")),
            args: Default::default(),
        },

        Type::Float => TyAnnot::Type {
            name: IdentId(ast.idents.add("float")),
            args: Default::default(),
        },

        Type::Generic(name) => TyAnnot::Type {
            name: *name,
            args: Default::default(),
        },

        Type::Unknown(_) => unreachable!("can't convert a type var to a type annotation"),

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
        let id = self.substs.fresh();
        self.add_type(env, Type::Unknown(id))
    }

    fn get_type(&self, id: TypeId) -> &Type {
        self.types.get(id.0)
    }

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
        let (annot, span) = ast.get_annot(annot_id);
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
        span: &Span,
    ) -> TypeResult<()> {
        let final_ty = if let Some(ty_annot) = var.ty {
            let annot_id = self.add_type_from_annotation(env, ast, ty_annot)?;
            self.check(env, ast, var.rhs, annot_id)
                .map_err(|e| tyerr(TypeErrorKind::WrongAnnotation { cause: Box::new(e) }, span))?;

            annot_id
        } else {
            self.infer(env, ast, var.rhs)?
        };

        env.bind_var(var.name, final_ty);
        self.vars_typed.push((stmt_id, final_ty));

        Ok(())
    }

    fn first_pass(&mut self, _env: &mut Environment, ast: &Ast, stmt_id: StmtId) -> TypeResult<()> {
        // this pass only collects the structs names and arguments
        // a struct can reference another struct which might be not yet declared

        let (stmt, span) = ast.get_stmt(stmt_id);
        match stmt {
            Stmt::StructDecl(content) => {
                if has_duplicates(&content.generics, |g| *g) {
                    return Err(tyerr(
                        TypeErrorKind::DuplicateGenericsInStruct(content.name),
                        span,
                    ));
                }

                // TODO: what if we have colliding names?
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

    fn second_pass(&mut self, env: &mut Environment, ast: &Ast, stmt_id: StmtId) -> TypeResult<()> {
        let (stmt, span) = ast.get_stmt(stmt_id);
        match stmt {
            Stmt::FuncDecl(content) => {
                // TODO: should check if fn name already exists or else the ones declared earlier get shadowed globally

                // TODO: should we check if generic names collides with primitive name?
                if has_duplicates(&content.generics, |g| *g) {
                    return Err(tyerr(
                        TypeErrorKind::DuplicateGenericsInFunction(content.name),
                        span,
                    ));
                }

                if has_duplicates(&content.params, |p| p.0) {
                    return Err(tyerr(
                        TypeErrorKind::DuplicateParamsInFunction(content.name),
                        span,
                    ));
                }

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
                    content.name,
                    FuncInfo {
                        generics: content.generics.clone(),
                        params: params.into(),
                        ret,
                    },
                );

                // TODO: can we shadow function names?
                env.bind_var(content.name, ty_id);
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
                        self.ty_self_referential_check(ty_id, content.name)?;

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

    fn typecheck(&mut self, env: &mut Environment, ast: &Ast, stmt_id: StmtId) -> TypeResult<()> {
        let (stmt, span) = ast.get_stmt(stmt_id);
        match stmt {
            Stmt::VarDecl(content) => self.var_decl(env, ast, content, stmt_id, span)?,

            Stmt::FuncDecl(content) => {
                let info = self
                    .funcs
                    .get(&content.name)
                    .ok_or_else(|| tyerr(TypeErrorKind::UndeclaredVar(content.name), span))?;

                let ret_expected = info.ret;
                let params = info.params.clone();

                let generics = info.generics.clone();
                let ret_actual = env.with_ty_scope(|env| env.with_var_scope(|env| {
                    for generic in generics {
                        self.add_type(env, Type::Generic(generic));
                    }

                    for (param_ty, param_content) in params.iter().zip(content.params.iter()) {
                        env.bind_var(param_content.0, *param_ty);
                    }

                    self.infer_block(env, ast, &content.code)
                }));

                let ret_actual = ret_actual?;
                self.unify(env, ret_actual, ret_expected, span)?;
            }

            Stmt::StructDecl(_) => {} // Do nothing, already done in top level pass
            Stmt::Continue | Stmt::Break => {
              Err(tyerr(TypeErrorKind::InvalidControlStmtInBlock, span))?
            }

            Stmt::Expr(_) => {
                Err(tyerr(TypeErrorKind::InvalidStmtInGlobal, span))?
            }
        }

        Ok(())
    }

    // TODO: this fails IMMEDIATELY, add statement level granularity for errors
    fn infer_block(
        &mut self,
        env: &mut Environment,
        ast: &Ast,
        block: &Block,
    ) -> TypeResult<TypeId> {
        let mut ret_ty = None;

        // iter over all except the last
        // this is important because otherwise we would add two different type variables for the last expression and incurr in a type error
        for stmt_id in block.list.iter().rev().skip(1).rev().copied() {
            let (stmt, span) = ast.get_stmt(stmt_id);
            match stmt {
                Stmt::VarDecl(content) => self.var_decl(env, ast, content, stmt_id, span)?,

                Stmt::Expr(expr_id) => {
                    let (expr, _) = ast.get_expr(*expr_id);

                    match expr {
                        Expr::Return(expr_id) => match ret_ty {
                            None => {
                                // we found the first return epxr.
                                ret_ty = Some(self.infer(env, ast, *expr_id)?);
                            }
                            Some(found_ret_ty) => {
                                // all other return statements should have the same type as the first.
                                self.check(env, ast, *expr_id, found_ret_ty)?;
                            }
                        },

                        _ => _ = self.infer(env, ast, *expr_id)?,
                    }
                }

                Stmt::Break | Stmt::Continue => if self.loop_depth == 0 {
                  Err(tyerr(TypeErrorKind::InvalidControlStmtInBlock, span))?
                }

                Stmt::FuncDecl(_) | Stmt::StructDecl(_) => {
                    let err = tyerr(TypeErrorKind::InvalidStmtInBlock, span);
                    Err(err)?
                }
            };
        }

        // check last expression
        // if block has no elements, its just void
        if let Some(last) = block.list.last() {
            let (stmt, span) = ast.get_stmt(*last);

            match stmt {
                Stmt::Expr(expr) => match ret_ty {
                    None => {
                        ret_ty = Some(self.infer(env, ast, *expr)?);
                    }
                    Some(found_ret_ty) => {
                        self.check(env, ast, *expr, found_ret_ty)?;
                    }
                },

                _ => return Err(tyerr(TypeErrorKind::BlockWithoutReturnExpr(ret_ty), span)),
            }
        }

        // if we didn't find any return types, this block has type void
        Ok(ret_ty.unwrap_or(VOID_ID))
    }

    fn infer(&mut self, env: &mut Environment, ast: &Ast, expr: ExprId) -> TypeResult<TypeId> {
        let (expr, span) = ast.get_expr(expr);
        let id = match expr {
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
                let first_ty = self.infer(env, ast, *first)?;
                if let Some(second) = second {
                    self.check(env, ast, *second, first_ty)?;
                }

                first_ty
            }

            Expr::While { cond, code } => {
                self.check(env, ast, *cond, BOOL_ID)?;
                self.loop_depth += 1;
                let res = self.infer(env, ast, *code);
                self.loop_depth -= 1;
                res?;
                VOID_ID
            }

            Expr::Call { callee, args } => {
                let ty_id = self.infer(env, ast, *callee)?;
                match self.get_type(ty_id).clone() {
                    Type::Unknown(id) => {
                        todo!("figure this one out");
                    }

                    Type::Func { params, ret } => {
                        for (param, arg) in params.iter().zip(args.iter()) {
                            self.check(env, ast, *arg, *param)?;
                        }

                        if params.len() != args.len() {
                            return Err(tyerr(
                                TypeErrorKind::WrongCallArgumentsCount {
                                    expected: params.len(),
                                    actual: args.len(),
                                },
                                span,
                            ));
                        }

                        // return value might be a type variable, we have to normalize and get the potential solved type
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
                            _ => self.check(env, ast, *value, *field_ty)?,
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

    // TODO: might need to add an erorr variant util so we know which kind of type mismatch wege vot
    fn check(
        &mut self,
        env: &mut Environment,
        ast: &Ast,
        expr_id: ExprId,
        expected_ty: TypeId,
    ) -> TypeResult<()> {
        let (_, span) = ast.get_expr(expr_id);

        let inferred_ty = self.infer(env, ast, expr_id)?;
        self.unify(env, expected_ty, inferred_ty, span)?;

        Ok(())
    }

    fn unify(
        &mut self,
        env: &Environment,
        ty_id_a: TypeId,
        ty_id_b: TypeId,
        span: &Span,
    ) -> TypeResult<()> {
        let ty_id_a = self.ty_normalize(env, ty_id_a);
        let ty_id_b = self.ty_normalize(env, ty_id_b);

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
            | (Type::Integer, Type::Integer)
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
    fn ty_normalize(&self, env: &Environment, ty: TypeId) -> TypeId {
        match self.get_type(ty) {
            Type::Unknown(x) => {
                if let Some(parent) = self.substs.0[*x as usize] {
                    self.ty_normalize(env, parent)
                } else {
                    ty
                }
            }

            _ => ty,
        }
    }

    // substitute an equality constraint between type variable and monomorphic type
    fn ty_substitute(&mut self, type_var: u32, ty: TypeId) {
        self.substs.0[type_var as usize] = Some(ty);
    }

    // instantiate a generic type to a concrete one
    // generic arguments are converted to type variables
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
                if let Some(fresh) = generics.get(&name) {
                    *fresh
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
            Type::Void | Type::Bool | Type::Integer | Type::Float => false,
            Type::Generic(_) => false,
            Type::Func { params, ret } => {
                params.iter().any(|p| self.ty_occurs_check(*p, ty_var))
                    || self.ty_occurs_check(*ret, ty_var)
            }
            Type::UserDef { args, .. } => args.iter().any(|a| self.ty_occurs_check(*a, ty_var)),
            Type::Unknown(inner_ty_var) => *inner_ty_var == ty_var,
        }
    }

    fn ty_self_referential_check(&self, curr: TypeId, parent: IdentId) -> TypeResult<()> {
        match self.get_type(curr) {
            Type::Void | Type::Bool | Type::Integer | Type::Float => Ok(()),
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
                    return Err(tyerr(
                        TypeErrorKind::SelfReferentialStruct(parent),
                        &Span { start: 0, end: 0 }, // TODO: no span here
                    ));
                }

                for arg in args {
                    self.ty_self_referential_check(*arg, parent)?;
                }

                Ok(())
            }
        }
    }
}

pub fn typecheck(ast: &mut Ast) -> Typechecker {
    let mut tc = Typechecker::default();
    let mut env = Environment::default();

    // first pass: gather userdefs names and arguments only
    for stmt in &ast.global_stmts {
        if let Err(e) = tc.first_pass(&mut env, ast, *stmt) {
            tc.report_error(e);
        }
    }

    // second pass: gather top level symbols (struct fields and function signatures)
    for stmt in &ast.global_stmts {
        if let Err(e) = tc.second_pass(&mut env, ast, *stmt) {
            tc.report_error(e);
        }
    }

    // third pass, global variables and scan function bodies
    for stmt in &ast.global_stmts {
        if let Err(e) = tc.typecheck(&mut env, ast, *stmt) {
            tc.report_error(e);
        }
    }

    // TODO: should check for any unresolved variable?

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
