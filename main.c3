import lexer;
import parser;
import eval;
import std::io;

fn void! repl() {
    Env env = {};

    while (true) {
        io::print("> ");
        String! line = io::readline();
        if (catch line) { continue; }
        if (line.len == 0) { continue; }
        
        interpret(line, &env);
    }
}

fn void interpret(String input, Env* env) {
    DynamicArenaAllocator arena;
    arena.init(2048, allocator::heap());
    
    TokenList tokens = lexer::scan(input);
    StmtList stmts = parser::parse(tokens);
    env.run(stmts);
}

fn void main() {
    String header = string::new_format("Hello!\n");
    io::printn(header);
    repl()!!;
}