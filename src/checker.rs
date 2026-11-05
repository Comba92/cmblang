use crate::{UnionFind::UnionFind, intern::*, *};
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
    Generic(IdentId), // TODO: this should be scoped man
    TypeVar(u32),
    // Array {
    //     inner: TypeId,
    //     size: u32,
    // },
    // Tuple(List<TypeId>),
    Func {
        generics: List<IdentId>, // TODO: is this needed?
        params: List<TypeId>,
        ret: TypeId,
    },
    Struct {
        generics: List<IdentId>, // TODO: is this needed?
        name: IdentId,           // TODO: is name needed?
        fields: List<(IdentId, TypeId)>,
    },
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

#[derive(Debug)]
pub enum TypeErrorKind {
    UndeclaredVar,
    WrongAnnotation { cause: Box<TypeError> },
    TypeMismatch { a: Type, b: Type },
    InfiniteType { source: Type, contains: Type },
    RequiredAnnotation,

    InvalidStmtInGlobal,
    InvalidStmtInBlock,

    WrongCallArgumentsCount,
    WrongStructLiteralCount,
    WrongStructLiteralField,
    DuplicateParamsInFunction,
    DuplicateGenericsInFunction,
    DuplicateFieldsInStruct,
    DuplicateGenericsInStruct,

    CallingWithWrongType,
    IndexingWithWrongType,
    ValueNotAssignable,
    UndeclaredType,
}

type TypeError = Spanned<TypeErrorKind>;
pub fn tyerr(kind: TypeErrorKind, span: &Span) -> TypeError {
    (kind, *span)
}

type TypeResult<T> = Result<T, TypeError>;

// TODO: temporary solution
// Basically a simple Union-find, not optimized
#[derive(Default)]
struct Substs(Vec<Option<TypeId>>);
impl Substs {
    pub fn fresh(&mut self) -> u32 {
        let id = self.0.len() as u32;
        self.0.push(None);
        id
    }
}

pub struct Typechecker {
    env: Environment,
    pub types: Interner<Type>,

    pub funcs: HashMap<IdentId, TypeId>,
    pub structs: HashMap<IdentId, TypeId>,

    generics_pool: HashMap<IdentId, TypeId>, // here just so we dont have to pass it around and allocate a new one each time

    substs: Substs,

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
            env: Environment::default(),
            types,

            funcs: Default::default(),
            structs: Default::default(),

            generics_pool: Default::default(),
            substs: Default::default(),

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

        Type::TypeVar(_) => unreachable!("can't convert a type var to a type annotation"),

        // we're supposed to get an instanciated function here??
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
        Type::Struct {
            name,
            generics,
            fields,
        } => {
            todo!("struct to annotation not implemented");
            //             let mut list = Vec::new();
            //             for (name, ty) in fields {
            //                 list.push(type_to_annotation(*ty, tc, ast));
            //             }
            //
            //             // we're supposed to get back the generics here?
            //             TyAnnot::Type {
            //                 name: *name,
            //                 args: (),
            //             }
        } // Type::Array { inner, size } => TyAnnot::Array {
          //     inner: type_to_annotation(*inner, tc, ast),
          //     size: todo!("array types not implemented yet"),
          // },
          // Type::Tuple(list) => todo!("tuple not implemented yet"),
    };

    ast.add_ty_annot(annot)
}

impl Typechecker {
    fn add_type(&mut self, ty: Type) -> TypeId {
        let id = self.types.add(ty);
        TypeId(id as Id)
    }

    fn add_ty_var(&mut self) -> TypeId {
        let id = self.substs.fresh();
        self.add_type(Type::TypeVar(id))
    }

    fn get_type(&self, id: TypeId) -> &Type {
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

    // usually called "resolve"
    fn add_type_from_annotation(&mut self, ast: &Ast, annot_id: TyAnnotId) -> TypeId {
        let (annot, _) = ast.get_annot(annot_id);
        let ty = match annot {
            TyAnnot::Type { name, args } => {
                if let Some(generic) = self.generics_pool.get(name) {
                    if !args.is_empty() {
                        panic!("error, generic type variables can't have generic args")
                    }

                    return *generic;
                } else {
                    let str = ast.get_ident(*name);
                    match str {
                        "bool" => return BOOL_ID,
                        "int" => return INT_ID,
                        "float" => return FLOAT_ID,
                        _ => todo!("user defined types"),
                    }
                }
            }

            TyAnnot::Array { inner, size } => {
                // let inner_id = self.add_annotation_as_type(ast, *inner);
                // // TODO: size not handled right now
                // Type::Array {
                //     inner: inner_id,
                //     size: 0,
                // }
                todo!("array not implemented")
            }

            TyAnnot::Func { params, ret } => {
                let mut list = Vec::new();
                for param in params {
                    let id = self.add_type_from_annotation(ast, *param);
                    list.push(id);
                }

                let ret = if let Some(ret) = ret {
                    self.add_type_from_annotation(ast, *ret)
                } else {
                    VOID_ID
                };

                Type::Func {
                    generics: Default::default(),
                    params: list.into(),
                    ret,
                }
            }

            TyAnnot::Tuple(elems) => {
                let mut list = Vec::new();
                for elem in elems {
                    let id = self.add_type_from_annotation(ast, *elem);
                    list.push(id);
                }
                todo!(" tuple not implemented")
                // Type::Tuple(list.into())
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
            let annot_id = self.add_type_from_annotation(ast, ty_annot);
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
                // TODO: should check if fn name already exists or else the ones declared earlier get shadowed globally

                // TODO: should we check if generic names collides with primitive name?
                if has_duplicates(&content.generics, |g| *g) {
                    return Err(tyerr(TypeErrorKind::DuplicateGenericsInFunction, span));
                }

                if has_duplicates(&content.params, |p| p.0) {
                    return Err(tyerr(TypeErrorKind::DuplicateParamsInFunction, span));
                }

                self.generics_pool.clear();
                for generic in &content.generics {
                    let ty = self.add_type(Type::Generic(*generic));
                    self.generics_pool.insert(*generic, ty);
                }

                // add fn type to types list and to global scope
                let ret = if let Some(ret) = content.ret {
                    self.add_type_from_annotation(ast, ret)
                } else {
                    VOID_ID
                };

                let ty = Type::Func {
                    generics: content.generics.clone(),
                    params: content
                        .params
                        .iter()
                        .map(|p| self.add_type_from_annotation(ast, p.1))
                        .collect(),
                    ret,
                };

                let ty_id = self.add_type(ty);
                self.funcs.insert(content.name, ty_id); // TODO: can we shadow function names?
                self.env.bind(content.name, ty_id);
            }

            Stmt::StructDecl(content) => {
                if has_duplicates(&content.generics, |g| *g) {
                    return Err(tyerr(TypeErrorKind::DuplicateGenericsInStruct, span));
                }

                if has_duplicates(&content.fields, |f| f.0) {
                    return Err(tyerr(TypeErrorKind::DuplicateFieldsInStruct, span));
                }

                self.generics_pool.clear();
                for generic in &content.generics {
                    let ty = self.add_type(Type::Generic(*generic));
                    self.generics_pool.insert(*generic, ty);
                }

                let ty = Type::Struct {
                    generics: content.generics.clone(),
                    name: content.name,
                    fields: content
                        .fields
                        .iter()
                        .map(|f| (f.0, self.add_type_from_annotation(ast, f.1)))
                        .collect(),
                };

                let id = self.add_type(ty);
                self.structs.insert(content.name, id);
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
                        ..
                    } => {
                        for (param_ty, param_content) in params.iter().zip(content.params.iter()) {
                            self.env.bind(param_content.0, *param_ty);
                        }

                        let ret_actual = self.infer_block(ast, &content.code);
                        self.env.pop_scope();

                        let ret_actual = ret_actual?;

                        self.unify(ret_actual, ret_expected, span)?;
                    }

                    _ => unreachable!(
                        "function type should already be in global scope after name resolution pass"
                    ),
                }
            }

            Stmt::StructDecl(_) => {} // Do nothing, already done in top level pass

            Stmt::Expr(_) => {
                let err = tyerr(TypeErrorKind::InvalidStmtInGlobal, span);
                Err(err)?
            }
        }

        Ok(())
    }

    fn infer_block(&mut self, ast: &Ast, block: &Block) -> TypeResult<TypeId> {
        let mut ret_ty = None;

        let mut stmt_iter = block.start.0;
        while stmt_iter < block.end.0 {
            let stmt_id = StmtId(stmt_iter);
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

                        // TODO: we're visiting inner blocks more than once doing useless work
                        // TODO: this loops program
                        // Expr::Block(inner_block) => {
                        //     self.infer_block(ast, inner_block)?;
                        //     // WE continue visiting nodes from the last of this child block, or else we'll visit them more than once
                        //     stmt_iter = inner_block.end.0;
                        // }
                        _ => _ = self.infer(ast, *expr_id)?,
                    }
                }

                Stmt::FuncDecl(_) | Stmt::StructDecl(_) => {
                    let err = tyerr(TypeErrorKind::InvalidStmtInBlock, span);
                    Err(err)?
                }
            };

            stmt_iter += 1;
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

            // TODO: be careful. we can assign type variables to a variable (just think of a generic function)
            // A generic function is not instantiated, so what do we really assign to the variable?
            // We could infer the acctual function type by successive calls, however, that function can be only ONE thing (monomorphic) and not be polymorphic
            Expr::Variable(name) => {
                let ty = self
                    .env
                    .lookup(*name)
                    .ok_or_else(|| tyerr(TypeErrorKind::UndeclaredVar, span))?;

                self.ty_instantiate(ty, &mut HashMap::new())
            }

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
                // TODO: problem arises if the callee is a type variable.
                // Example: fn id<T>(T x) -> T { return x; }
                // let a = id;
                // As id is a generic function, it is not an instantiated type, so calling a returns a type variable

                let ty_id = self.infer(ast, *callee)?;
                match self.get_type(ty_id) {
                    Type::Func { .. } => {
                        let instance_id = self.ty_instantiate(ty_id, &mut HashMap::new());
                        let Type::Func { params, ret, .. } = self.get_type(instance_id).clone()
                        else {
                            unreachable!()
                        };

                        // we must clone to both access params and do check
                        for (param, arg) in params.iter().zip(args.iter()) {
                            self.check(ast, *arg, *param)?;
                        }

                        if params.len() != args.len() {
                            return Err(tyerr(TypeErrorKind::WrongCallArgumentsCount, span));
                        }

                        // return value might be a type variable, we have to normalize and get the potential solved type
                        self.ty_normalize(ret)
                    }

                    _ => Err(tyerr(TypeErrorKind::CallingWithWrongType, span))?,
                }
            }

            Expr::StructLiteral(name, args) => {
                if let Some(ty) = self.structs.get(name) {
                    let instance_id = self.ty_instantiate(*ty, &mut HashMap::new());

                    let Type::Struct { fields, .. } = self.get_type(instance_id).clone() else {
                        unreachable!()
                    };

                    // we must clone to both access fields and do check
                    for ((field_name, field_ty), (value_name, value)) in
                        fields.iter().zip(args.iter())
                    {
                        if field_name != value_name {
                            return Err(tyerr(TypeErrorKind::WrongStructLiteralField, span));
                        }
                        self.check(ast, *value, *field_ty)?;
                    }

                    if fields.len() != args.len() {
                        return Err(tyerr(TypeErrorKind::WrongStructLiteralCount, span));
                    }

                    instance_id
                } else {
                    Err(tyerr(TypeErrorKind::UndeclaredType, span))?
                }
            }

            Expr::Indexing { src, idx } => {
                //                 self.check(ast, *idx, INT_ID)?;
                //
                //                 let ty = self.infer(ast, *src)?;
                //                 match self.get_type(ty) {
                //                     Type::Array { inner, size } => {
                //                         // TODO: we might even do constant bound checking here
                //                         *inner
                //                     }
                //                     _ => Err(tyerr(TypeErrorKind::IndexingWithWrongType, span))?,
                //                 }
                todo!("array not implemented")
            }

            Expr::Member { src, member } => {
                todo!("structs not implemented yet")
            }

            Expr::Block(block) => {
                self.env.push_scope();
                let res = self.infer_block(ast, block);
                self.env.pop_scope();
                res?
            }
        };

        Ok(id)
    }

    fn check(&mut self, ast: &Ast, expr_id: ExprId, expected_ty: TypeId) -> TypeResult<()> {
        let (expr, span) = ast.get_expr(expr_id);

        match expr {
            // Expr::BoolLiteral(_) => BOOL_ID,
            // Expr::IntLiteral(_) => INT_ID,
            // Expr::FloatLiteral(_) => FLOAT_ID,
            _ => {
                let inferred_ty = self.infer(ast, expr_id)?;
                self.unify(expected_ty, inferred_ty, span)?;
            }
        };

        Ok(())
    }

    fn unify(&mut self, ty_id_a: TypeId, ty_id_b: TypeId, span: &Span) -> TypeResult<()> {
        let ty_id_a = self.ty_normalize(ty_id_a);
        let ty_id_b = self.ty_normalize(ty_id_b);

        let ty_a = self.get_type(ty_id_a).clone();
        let ty_b = self.get_type(ty_id_b).clone();

        match (ty_a, ty_b) {
            (Type::TypeVar(x), Type::TypeVar(y)) => {
                if x != y {
                    return Err(tyerr(
                        TypeErrorKind::TypeMismatch {
                            a: self.get_type(ty_id_a).clone(),
                            b: self.get_type(ty_id_b).clone(),
                        },
                        span,
                    ));
                }
            }

            (Type::TypeVar(x), _) => {
                if self.ty_occurs_check(ty_id_b, x) {
                    return Err(tyerr(
                        TypeErrorKind::InfiniteType {
                            source: self.get_type(ty_id_b).clone(),
                            contains: self.get_type(ty_id_a).clone(),
                        },
                        span,
                    ));
                } else {
                    self.ty_substitute(x, ty_id_b);
                }
            }
            (_, Type::TypeVar(x)) => {
                if self.ty_occurs_check(ty_id_a, x) {
                    return Err(tyerr(
                        TypeErrorKind::InfiniteType {
                            source: self.get_type(ty_id_a).clone(),
                            contains: self.get_type(ty_id_b).clone(),
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
                            a: self.get_type(ty_id_a).clone(),
                            b: self.get_type(ty_id_b).clone(),
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
                    self.unify(*param_l, *param_r, span)?;
                }

                self.unify(ret_l, ret_r, span)?;
            }

            (
                Type::Struct {
                    fields: fields_l, ..
                },
                Type::Struct {
                    fields: fields_r, ..
                },
            ) => {
                for (field_l, field_r) in fields_l.iter().zip(fields_r.iter()) {
                    self.unify(field_l.1, field_r.1, span)?;
                }
            }

            _ => {
                return Err(tyerr(
                    TypeErrorKind::TypeMismatch {
                        a: self.get_type(ty_id_a).clone(),
                        b: self.get_type(ty_id_b).clone(),
                    },
                    span,
                ));
            }
        }

        Ok(())
    }

    // get the solved type from a type variable
    fn ty_normalize(&mut self, ty: TypeId) -> TypeId {
        match self.get_type(ty) {
            Type::TypeVar(x) => {
                if let Some(parent) = self.substs.0[*x as usize] {
                    self.ty_normalize(parent)
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
    fn ty_instantiate(&mut self, ty: TypeId, generics: &mut HashMap<IdentId, TypeId>) -> TypeId {
        match self.get_type(ty).clone() {
            Type::Func { params, ret, .. } => {
                let instantiated = params
                    .iter()
                    .map(|p| self.ty_instantiate(*p, generics))
                    .collect();
                let ret_instantiated = self.ty_instantiate(ret, generics);

                self.add_type(Type::Func {
                    generics: Default::default(),
                    params: instantiated,
                    ret: ret_instantiated,
                })
            }

            Type::Struct { name, fields, .. } => {
                let instantiated = fields
                    .iter()
                    .map(|f| (f.0, self.ty_instantiate(f.1, generics)))
                    .collect();

                self.add_type(Type::Struct {
                    generics: Default::default(),
                    name,
                    fields: instantiated,
                })
            }

            Type::Generic(name) => {
                if let Some(fresh) = generics.get(&name) {
                    *fresh
                } else {
                    let fresh = self.add_ty_var();
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
            Type::Func { params, ret, .. } => {
                params.iter().any(|p| self.ty_occurs_check(*p, ty_var))
                    || self.ty_occurs_check(*ret, ty_var)
            }
            Type::Struct { fields, .. } => fields.iter().any(|f| self.ty_occurs_check(f.1, ty_var)),
            Type::TypeVar(inner_ty_var) => *inner_ty_var == ty_var,
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
