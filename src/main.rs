use cmblang::*;
use std::{
    fs,
    io::{self, Write},
};

fn main() {
    let mut out = String::new();

    let test = fs::read_to_string("test.cmb").unwrap();
    let lex = scan(test).unwrap();

    let parser = parse(&lex);
    for err in &parser.errors {
        out += &render_err(&lex, &err.0, err.1, ErrSource::Parser);
        out += "\n";
    }

    let mut ast = Ast::new(lex, parser);
    let checker = typecheck(&mut ast);

    for err in &checker.errors {
        out += &render_err(&ast.lex, &err.0, err.1, ErrSource::Checker);
        out += "\n";
    }

    io::stdout().lock().write_all(out.as_bytes()).unwrap();
    //     println!("{:?}", checker.types);
    //     println!("{:?}", checker.funcs);
    //     println!("{:?}", checker.structs);

    // let output = emit(&ast);
    // println!("{output}");

    // for line in io::stdin().lines() {
    //   match line {
    //     Ok(line) => println!("{line}"),
    //     Err(e) => eprintln!("{e}"),
    //   }
    // }
}
