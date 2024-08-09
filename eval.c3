module eval;

import lexer;
import parser;
import ast;

import std::io;
import std::collections::list;
import std::collections::map;
def Scope = HashMap(<char[], Value>);
def ScopeList = List(<Scope*>);

struct Environment {
  StmtList statements;
  ScopeList scopes;
}
fn void Environment.add_scope(Environment* self) {
  Scope* scope = mem::new(Scope, Scope {});
  self.scopes.push(scope);
}
fn void Environment.remove_scope(Environment* self) {
  self.scopes.pop() ?? null;
}
fn Scope* Environment.current_scope(Environment self) {
  return self.scopes.last() ?? null;
}

Environment env = Environment {};

enum ValueKind { NUMBER, NULL }
struct Value {
  ValueKind kind;
  union {
    float number;
  }
}
fn void Value.print(Value self) {
  switch (self.kind) {
    case NULL: { io::printn("null"); }
    case NUMBER: { io::printfn("%f", self.number); }
  }
}

fn void run(StmtList stmts) {
  if (env.scopes.len() == 0) env.add_scope();
  
  foreach(stmt : stmts) {
    execute(*stmt);
  }
}

fn void execute(Stmt stmt) {
  switch (stmt.kind) {
    case EXPR: {
      Value res = evaluate(stmt.expr);
      res.print();
    }
    case BLOCK: {
      env.add_scope();
      
      foreach (s : stmt.block.body) {
        execute(*s);
      }

      env.remove_scope();
    }
  }
}

fn Value evaluate(Expr expr) {  
  switch (expr.kind) {
    case LITERAL: {
      switch (expr.literal.kind) {
        case NUMBER: {
          Value result;
          result.kind = ValueKind.NUMBER;
          String temp = string::tformat("%s", expr.literal.lexeme);
          result.number = temp.to_float()!!;
          return result;
        }
        case IDENTIFIER: {
          //TODO: search in upper scope if not found
          return env.current_scope().get(expr.literal.lexeme) ?? Value {.kind = ValueKind.NULL};
        }
        default: unreachable();
      }
    }
    case ASSIGN: {
      char[] name = expr.assign.identifier.literal.lexeme;
      Value result = evaluate(*expr.assign.value);
      env.current_scope().set(name, result);
      return result;
    }
    case GROUPING: {
      return evaluate(*expr.grouping.expr);
    }
    case BINARY: {
      Value left = evaluate(*expr.binary.left);
      Value right = evaluate(*expr.binary.right);

      Value result;
      result.kind = ValueKind.NUMBER;
      switch (expr.binary.op.kind) {
        case ADD: result.number = left.number + right.number;
        case SUB: result.number = left.number - right.number;
        case MUL: result.number = left.number * right.number;
        case DIV: result.number = left.number / right.number;
        default: unreachable();
      }
      return result;
    }
    case UNARY: {
      Value right = evaluate(*expr.unary.right);
      Value result;
      result.kind = ValueKind.NUMBER;
      //TODO:
      unreachable("UNARY OP TO BE IMPLEMENTED");

      // return result;
    }
  }
}