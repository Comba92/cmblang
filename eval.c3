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
  INVALID_OPERATION,
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

  return self.panic(RuntimeErr.UNDEFINED_IDENTIFIER);
}

enum ValueKind { NUMBER, BOOL, NULL }
struct Value {
  ValueKind kind;
  union {
    float number;
    bool boolean;
    bool nulled;
  }
}
fn void Value.print(Value self) {
  switch (self.kind) {
    case NULL: { io::printn("null"); }
    case NUMBER: { io::printfn("%f", self.number); }
    case BOOL: { io::printn(self.boolean); }
  }
}

fn Value Env.panic(Env* self, RuntimeErr err) {
  // TODO: better panicking? Panic checks are all over the place
  String msg;
  msg = string::new_format("Runtime error %s", err);
  
  io::eprintn(msg);
  self.panicked = true;
  return Value {};
}

fn void run(Env* env, StmtList stmts) {
  env.panicked = false;
  if (env.scopes.len() == 0) env.add_scope();
  
  foreach(stmt : stmts) {
    env.execute(*stmt);
    if (env.panicked) break;
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
      Value res = self.evaluate(*stmt.expr);
      if (!self.panicked) res.print();
    }
    case VAR: {
      Value res = self.evaluate(*stmt.vars.value);
      self.current_scope().set(stmt.vars.name.lexeme, res);
      res.print();
    }
    case BLOCK: {
      self.add_scope();
      
      foreach (s : stmt.block.body) {
        self.execute(*s);
      }

      self.remove_scope();
    }
    case IF: {
      Value cond = self.evaluate(*stmt.ifs.condition);
      if (cond.boolean) {
        self.execute(*stmt.ifs.if_branch);
      } else {
        self.execute(*stmt.ifs.else_branch);
      }
    }
    case WHILE: {
      io::printn("executing while...");
      Value cond = self.evaluate(*stmt.whiles.condition);
      while(cond.boolean) {
        self.execute(*stmt.whiles.body);
        cond = self.evaluate(*stmt.whiles.condition);
      }
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
          return Value {.kind = ValueKind.NULL, .nulled = true};
        }
        case IDENTIFIER: {
          return self.get_identifier_value(expr.literal.lexeme);
        }
        default: unreachable();
      }
    }
    case ASSIGN: {
      // FIXME: currently assignment only updates a variable in its current scope (if it doesnt exists it creates one)
      char[] name = expr.assign.identifier.literal.lexeme;
      self.get_identifier_value(name);
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

      Value result;

      switch (left.kind) {
        case NUMBER: {
          if (right.kind != ValueKind.NUMBER) {
            return self.panic(RuntimeErr.INVALID_OPERAND_TYPE);
          }

          switch (expr.binary.op.kind) {
            case ADD: result.number = left.number + right.number;
            case SUB: result.number = left.number - right.number;
            case MUL: result.number = left.number * right.number;
            case DIV: result.number = left.number / right.number;

            case EQUALEQUAL: result.boolean = left.number == right.number;
            case BANGEQUAL: result.boolean = left.number != right.number;
            case GREATER:    result.boolean = left.number > right.number;
            case GREATEREQUAL: result.boolean = left.number <= right.number;
            case LESS:       result.boolean = left.number < right.number;
            case LESSEQUAL:  result.boolean = left.number >= right.number;

            default: return self.panic(RuntimeErr.INVALID_OPERATION);
          }

          // We have to infer what type of result we've got: operations returns a number, relations returns a boolean!
          switch (expr.binary.op.kind) {
            case ADD: nextcase;
            case SUB: nextcase;
            case MUL: nextcase;
            case DIV: result.kind = ValueKind.NUMBER;

            case EQUALEQUAL: nextcase;
            case BANGEQUAL: nextcase;
            case GREATER: nextcase;
            case GREATEREQUAL: nextcase;
            case LESS: nextcase;
            case LESSEQUAL: result.kind = ValueKind.BOOL;

            default: unreachable();
          }
        }
        case BOOL: {
          if (right.kind != ValueKind.BOOL) {
            return self.panic(RuntimeErr.INVALID_OPERAND_TYPE);
          }
          result.kind = ValueKind.BOOL;
          
          switch (expr.binary.op.kind) {
            case AND: result.boolean = left.boolean && right.boolean;
            case OR: result.boolean = left.boolean || right.boolean;

            case EQUALEQUAL: result.boolean = left.boolean == right.boolean;
            case BANGEQUAL: result.boolean = left.boolean != right.boolean;
            default: return self.panic(INVALID_OPERAND_TYPE);
          }
        }
        case NULL: {
          result.kind = ValueKind.BOOL;

          switch(expr.binary.op.kind) {
            case EQUALEQUAL: result.boolean = left.nulled == right.nulled;
            case BANGEQUAL: result.boolean = left.nulled != right.nulled;
            default: return self.panic(INVALID_OPERAND_TYPE);
          }
        }
        
        default: unreachable();
      }

      return result;
    }
    case UNARY: {
      Value right = self.evaluate(*expr.unary.right);
      Value result;
      
      switch (expr.unary.op.kind) {
        case NOT: nextcase;
        case BANG: {
          if (right.kind != ValueKind.BOOL) {
            return self.panic(RuntimeErr.INVALID_OPERAND_TYPE);
          }

          result.kind = ValueKind.BOOL;
          result.boolean = !right.boolean;
        }
        case SUB: {
          if (right.kind != ValueKind.NUMBER) {
            return self.panic(RuntimeErr.INVALID_OPERAND_TYPE);
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