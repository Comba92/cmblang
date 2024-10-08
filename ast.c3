module ast;

import lexer;
import parser;

import std::io;
import std::collections::list;
def ExprList = List(<Expr*>);
def StmtList = List(<Stmt*>);

enum StmtKind { EXPR, PRINT, BLOCK, VAR, IF, WHILE }
struct Stmt {
    StmtKind kind;
    union {
        Expr* expr;
        VarStmt* vars;
        PrintStmt* print;
        BlockStmt* block;
        IfStmt* ifs;
        WhileStmt* whiles;
    }
}

struct PrintStmt {
    Expr* expr;
}

struct VarStmt {
    Token* name;
    Expr* value;
}

struct BlockStmt {
    Token* left, right;
    StmtList body;
}

struct IfStmt {
    Expr* condition;
    Stmt* if_branch;
    Stmt* else_branch;
}

struct WhileStmt {
    Expr* condition;
    Stmt* body;
}

struct GroupingExpr {
    Token* left, right;
    Expr* expr;
}
struct BinaryExpr {
    Token* op;
    Expr* left, right;
}
struct UnaryExpr {
    Token* op;
    Expr* right;
}
struct AssignExpr {
  Expr* identifier;
  Token* equal;
  Expr* value;
}
struct ListExpr {
    Token* start;
    ExprList exprs;
    Token* end;
}
struct IndexingExpr {
    Token* identifier;
    Expr* index;
}

enum ExprKind { LITERAL, LIST, INDEXING, GROUPING, BINARY, UNARY, ASSIGN }
struct Expr {
    ExprKind kind;
    union {
        Token* literal;
        ListExpr list;
        IndexingExpr indexing;
        GroupingExpr grouping;
        BinaryExpr binary;
        UnaryExpr unary;
        AssignExpr assign;
    }
}

fn void Expr.debug(Expr self) {
    switch (self.kind) {
      case LITERAL: io::printf("LITERAL: %s", self.literal.lexeme);
      case GROUPING: io::print("("); self.grouping.expr.debug(); io::print(")");
      case UNARY: io::printf("UNARY: %s", self.unary.op.lexeme); self.unary.right.debug();
      case BINARY: io::print("BINARY: "); self.binary.left.debug(); io::printf("%s", self.binary.op.lexeme); self.binary.right.debug();
      default:
    }
}