module ast;

import lexer;
import parser;

import std::io;

enum StmtKind { EXPR, BLOCK, IF, WHILE }
struct Stmt {
    StmtKind kind;
    union {
        Expr* expr;
        BlockStmt* block;
        IfStmt* ifs;
        WhileStmt* whiles;
    }
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

enum ExprKind { LITERAL, GROUPING, BINARY, UNARY, ASSIGN }
struct Expr {
    ExprKind kind;
    union {
        Token* literal;
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