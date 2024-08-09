import lexer;
import parser;
import eval;
import std::io;

fn void! repl() {
    while (true) {
        io::print("> ");
        String! line = io::readline();
        if (catch line) { continue; }
        if (line.len == 0) { continue; }
        
        interpret(line);
    }
}

fn void interpret(String input) {
    DynamicArenaAllocator arena;
    arena.init(2048, allocator::heap());
    
    mem::@scoped(&arena) {
        TokenList tokens = lexer::scan(input);
        StmtList stmts = parser::parse(tokens);
        eval::run(stmts);
    };

    arena.free();
}

fn void main() {
    String header = string::new_format("Hello!\n");
    io::printn(header);

    repl()!!;
}