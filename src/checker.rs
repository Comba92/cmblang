use crate::parser::*;
use crate::*;
use std::collections::HashMap;

// https://www.reddit.com/r/ProgrammingLanguages/comments/v5zwlw/resources_for_building_a_typechecker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/8kox21/help_implementing_a_type_checker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/1nhy0ef/how_do_i_make_a_real_typechecker/
// https://www.reddit.com/r/ProgrammingLanguages/comments/1gkjkz4/how_to_implement_local_type_inference/
// https://ettolrach.com/blog/bidirectional_inference.html
// https://thunderseethe.dev/posts/how-to-choose-between-hm-and-bidir/
// https://thunderseethe.dev/posts/bidirectional-constraint-generation/

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

    fn bind(&mut self, name: IdentId, ty: TypeId) {
        self.scopes.last_mut().unwrap().insert(name, ty);
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new())
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

struct TypeId(pub Id);

#[derive(Debug, Clone)]
enum Type {
    Void,
    Integer,
    Float,
    Bool,
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

const VOID_ID: u32 = 0;
const BOOL_ID: u32 = 1;
const INT_ID: u32 = 2;
const FLOAT_ID: u32 = 3;
const PRIMITIVES: &[(Type, TypeId)] = &[
    (Type::Void, VOID_ID),
    (Type::Bool, BOOL_ID),
    (Type::Integer, INT_ID),
    (Type::Float, FLOAT_ID),
];

#[derive(Debug)]
enum TypeErrorKind {
    UndeclaredVar,
    WrongAnnotation { cause: Box<TypeError> },
    WrongTypecheck { expected: Type, actual: Type },
    RequiredAnnotation,

    InvalidStmtInTopLevel,
    InvalidStmtInBlock,

    CallingWithWrongType,
    IndexingWithWrongType,
}

#[derive(Debug)]
struct TypeError {
    // span: Span,
    kind: TypeErrorKind,
}
impl TypeError {
    pub fn new(kind: TypeErrorKind) -> Self {
        Self { kind }
    }
}

type TypeResult<T> = Result<T, TypeError>;

struct Typechecker {
    env: Environment,
    types: Vec<Type>,
    errors: Vec<TypeError>,
}

impl Default for Typechecker {
    fn default() -> Self {
        Self {
            env: Environment::default(),
            types: PRIMITIVES.iter().map(|x| x.0.clone()).collect(),
            errors: vec![],
        }
    }
}

impl Typechecker {
    fn add_type(&mut self, ty: Type) -> TypeId {
        let id = self.types.len();
        self.types.push(ty);
        id as TypeId
    }

    fn get_type(&self, id: TypeId) -> &Type {
        &self.types[id as usize]
    }

    fn add_annotation_as_type(&mut self, ast: &Parser, annot: TyAnnotId) -> TypeId {
        let ty = match &ast.types[annot.0 as usize] {
            TyAnnot::Type { name, args } => {
                let str = ast.get_ident(*name);
                match str {
                    "bool" => return BOOL_ID,
                    "int" => return INT_ID,
                    "float" => return FLOAT_ID,
                    _ => todo!("user defined types not implemented"),
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

                Type::Func {
                    params: list.into(),
                    ret: ret.unwrap_or(VOID_ID),
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

    fn toplvl_pass(&mut self, ast: &Parser) {
        for stmt_id in &ast.toplvl_stmts {
            let stmt = ast.get_stmt(*stmt_id);
            match stmt {
                Stmt::FuncDecl(content) => {
                    // add fn type to types list and to global scope
                    let ret = if let Some(ret) = content.ret {
                        self.add_annotation_as_type(ast, ret)
                    } else {
                        VOID_ID
                    };

                    let ty = Type::Func {
                        params: content.params.iter().map(|p| p.1).collect(),
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
        }
    }

    fn var_decl(&mut self, ast: &Parser, var: &VarContent) -> TypeResult<()> {
        let final_ty = if let Some(ty_annot) = var.ty {
            let annot_id = self.add_annotation_as_type(ast, ty_annot);
            self.check(ast, var.rhs, annot_id).map_err(|e| {
                TypeError::new(TypeErrorKind::WrongAnnotation { cause: Box::new(e) })
            })?;
            annot_id
        } else {
            self.infer(ast, var.rhs)?
        };

        self.env.bind(var.name, final_ty);
        Ok(())
    }

    fn typecheck(&mut self, ast: &Parser, stmt: &Stmt) -> TypeResult<()> {
        match stmt {
            Stmt::VarDecl(content) => self.var_decl(ast, content)?,

            Stmt::FuncDecl(content) => {
                self.env.push_scope();
                for param in &content.params {
                    let ty = self.add_annotation_as_type(ast, param.1);
                    self.env.bind(param.0, ty);
                }

                let ret_actual = self.typecheck_block(ast, &content.code);
                self.env.pop_scope();

                let ret_expected = if let Some(ret) = content.ret {
                    self.add_annotation_as_type(ast, ret)
                } else {
                    VOID_ID
                };

                let ret_actual = ret_actual?;
                if ret_actual != ret_expected {
                    let err = TypeError::new(TypeErrorKind::WrongTypecheck {
                        expected: self.get_type(ret_expected).clone(),
                        actual: self.get_type(ret_actual).clone(),
                    });

                    Err(err)?
                }
            }

            Stmt::StructDecl(_) => {} // Do nothing, already done in toplvl_pass

            Stmt::Expr(_) => {
                let err = TypeError::new(TypeErrorKind::InvalidStmtInTopLevel);
                Err(err)?
            }
        }

        Ok(())
    }

    fn typecheck_block(&mut self, ast: &Parser, block: &Block) -> TypeResult<TypeId> {
        let mut ty = None;
        for stmt_id in block.start..block.end {
            let stmt = ast.get_stmt(stmt_id);
            match stmt {
                Stmt::VarDecl(content) => self.var_decl(ast, content)?,

                Stmt::Expr(expr_id) => {
                    let expr = ast.get_expr(*expr_id);
                    match expr {
                        Expr::Return(expr_id) => match ty {
                            None => {
                                ty = Some(self.infer(ast, *expr_id)?);
                            }
                            Some(other_ty) => {
                                self.check(ast, *expr_id, other_ty)?;
                            }
                        },

                        _ => _ = self.infer(ast, *expr_id)?,
                    }
                }

                Stmt::FuncDecl(_) | Stmt::StructDecl(_) => {
                    let err = TypeError::new(TypeErrorKind::InvalidStmtInBlock);
                    Err(err)?
                }
            };
        }

        Ok(ty.unwrap_or(VOID_ID))
    }

    fn infer(&mut self, ast: &Parser, expr: ExprId) -> TypeResult<TypeId> {
        let id = match ast.get_expr(expr) {
            Expr::BoolLiteral(_) => BOOL_ID,
            Expr::IntLiteral(_) => INT_ID,
            Expr::FloatLiteral(_) => FLOAT_ID,

            Expr::Variable(name) => self
                .env
                .lookup(*name)
                .ok_or_else(|| TypeError::new(TypeErrorKind::UndeclaredVar))?,

            Expr::Return(id) => self.infer(ast, *id)?,

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
                let func_ty = self
                    .env
                    .lookup(*callee)
                    .ok_or_else(|| TypeError::new(TypeErrorKind::UndeclaredVar))?;

                match self.get_type(func_ty).clone() {
                    // we must clone to both access params and do check
                    Type::Func { params, ret } => {
                        for (param, arg) in params.iter().zip(args.iter()) {
                            self.check(ast, *arg, *param)?;
                        }
                        ret
                    }
                    _ => Err(TypeError::new(TypeErrorKind::CallingWithWrongType))?,
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
                    _ => Err(TypeError::new(TypeErrorKind::IndexingWithWrongType))?,
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

    fn check(&mut self, ast: &Parser, expr: ExprId, target: TypeId) -> TypeResult<()> {
        let res_ty = match ast.get_expr(expr) {
            Expr::BoolLiteral(_) => BOOL_ID,
            Expr::IntLiteral(_) => INT_ID,
            Expr::FloatLiteral(_) => FLOAT_ID,

            _ => {
                let inferred_ty = self.infer(ast, expr)?;
                inferred_ty
            }
        };

        if res_ty != target {
            let err = TypeError::new(TypeErrorKind::WrongTypecheck {
                expected: self.get_type(target).clone(),
                actual: self.get_type(res_ty).clone(),
            });
            Err(err)
        } else {
            Ok(())
        }
    }
}

pub fn check(ast: Parser) {
    // first pass: gather top level symbols (struct definitions and function signatures)
    // second pass, global variables and scan funtion bodies

    let mut tc = Typechecker::default();
    tc.toplvl_pass(&ast);

    let mut iter = ast.stmts.iter();
    while let Some(stmt) = iter.next() {
        match tc.typecheck(&ast, stmt) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{e:?}");
                tc.errors.push(e);
            }
        }
    }
}
