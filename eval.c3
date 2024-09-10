module eval;

import lexer;
import parser;
import ast;

import std::io;
import std::collections::list;
import std::collections::map;

def Scope = HashMap(<char[], Value>);
def ScopeList = List(<Scope*>);

struct Env {
  ScopeList scopes;
  bool panicked;
}

fault RuntimeErr {
  INVALID_OPERAND_TYPE,
  UNDEFINED_IDENTIFIER
}

fn void Env.add_scope(Env* self) {
  Scope* scope = mem::new(Scope, Scope {});
  self.scopes.push(scope);
}
fn void Env.remove_scope(Env* self) {
  self.scopes.pop() ?? null;
}
fn Scope* Env.current_scope(Env self) {
  return self.scopes.last() ?? null;
}
fn Value Env.get_identifier_value(Env* self, char[] id) {
  int current_scope = (int)self.scopes.len()-1;
  while (current_scope >= 0) {
    Scope* scope = self.scopes.get(current_scope);
    Value! val = scope.get(id);
    if (try val) return val;
    current_scope -= 1;
  }

  self.panic(RuntimeErr.UNDEFINED_IDENTIFIER);
  return {};
}

enum ValueKind { NUMBER, BOOL, NULL }
struct Value {
  ValueKind kind;
  union {
    float number;
    bool boolean;
  }
}
fn void Value.print(Value self) {
  switch (self.kind) {
    case NULL: { io::printn("null"); }
    case NUMBER: { io::printfn("%f", self.number); }
    case BOOL: { io::printn(self.boolean); }
  }
}

fn void Env.panic(Env* self, RuntimeErr err) {
  // TODO: better panicking? Panic checks are all over the place
  String msg;
  msg = string::new_format("Runtime error %s", err);
  
  io::eprintn(msg);
  self.panicked = true;
}

fn void Env.run(Env* self, StmtList stmts) {
  self.panicked = false;
  if (self.scopes.len() == 0) self.add_scope();
  
  foreach(stmt : stmts) {
    self.execute(*stmt);
    if (self.panicked) break;
  }

    // io::printn("Scope: ");
    // Scope.@each(*scope; char[] key, Value val) {
    //   io::printn(key);
    //   val.print();
    // };
}

fn void Env.execute(Env* self, Stmt stmt) {
  if (self.panicked) return;
  
  switch (stmt.kind) {
    case EXPR: {
      Value res = self.evaluate(stmt.expr);
      if (!self.panicked) res.print();
    }
    case BLOCK: {
      self.add_scope();
      
      foreach (s : stmt.block.body) {
        self.execute(*s);
      }

      self.remove_scope();
    }
  }
}

fn Value Env.evaluate(Env* self, Expr expr) {  
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
        case BOOL: {
          Value result;
          result.kind = ValueKind.BOOL;
          switch (expr.literal.lexeme) {
            case "true": result.boolean = true;
            case "false": result.boolean = false;
            default: unreachable();
          }
          return result;
        }
        case NULL: {
          return Value {.kind = ValueKind.NULL, .number = 0};
        }
        case IDENTIFIER: {
          return self.get_identifier_value(expr.literal.lexeme);
        }
        default: unreachable();
      }
    }
    case ASSIGN: {
      char[] name = expr.assign.identifier.literal.lexeme;
      Value result = self.evaluate(*expr.assign.value);
      self.current_scope().set(name, result);
      return result;
    }
    case GROUPING: {
      return self.evaluate(*expr.grouping.expr);
    }
    case BINARY: {
      Value left = self.evaluate(*expr.binary.left);
      Value right = self.evaluate(*expr.binary.right);

      if (left.kind != ValueKind.NUMBER || right.kind != ValueKind.NUMBER) {
        self.panic(RuntimeErr.INVALID_OPERAND_TYPE);
        return {};
      }

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
      Value right = self.evaluate(*expr.unary.right);
      Value result;
      
      switch (expr.unary.op.kind) {
        case BANG: {
          if (right.kind != ValueKind.BOOL) {
            self.panic(RuntimeErr.INVALID_OPERAND_TYPE);
            return {};
          }

          result.kind = ValueKind.BOOL;
          result.boolean = !right.boolean;
        }
        case SUB: {
          if (right.kind != ValueKind.NUMBER) {
            self.panic(RuntimeErr.INVALID_OPERAND_TYPE);
            return {};
          }

          result.kind = ValueKind.NUMBER;
          result.number = -right.number;
        }
        default: unreachable();
      }

      return result;
    }
  }
}