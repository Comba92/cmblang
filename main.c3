import lexer;
import parser;
import eval;
import ast;

import std::io;
import std::collections::list;

def StrList = List(<String>);

fn void! repl() {
    Env env = {};
    StrList history;

    while (true) {
        io::print("> ");
        String! line = io::readline();
        if (catch line) { continue; }
        if (line.len == 0) { continue; }
        
        history.push(line);
        interpret(line, &env);
    }
}

fn void interpret(String input, Env* env) {
    TokenList tokens = lexer::scan(input);
    // foreach(token : tokens) {
    //     io::printn(token.kind);
    // }
    StmtList stmts = parser::parse(tokens);
    eval::run(env, stmts);
}

fn void main() {
    String header = string::new_format("Hello!\n");
    io::printn(header);
    repl()!!;
}