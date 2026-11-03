use cmblang::*;
use std::{fs, io};

fn main() {
    println!("Hello!");

    let test = fs::read_to_string("test.cmb").unwrap();
    let lex = scan(test).unwrap();

    for tok in &lex.tokens {
        println!("{tok}");
    }

    let parser = parse(&lex);
    for (i, stmt) in parser.stmts.iter().enumerate() {
        println!("{i} == {:?}", stmt);
    }

    println!();

    for (i, expr) in parser.exprs.iter().enumerate() {
        println!("{i} == {:?}", expr);
    }

    println!();

    let mut ast = Ast::new(lex, parser);
    let checker = typecheck(&mut ast);

    let output = emit(&ast);
    println!("{output}");

    // for line in io::stdin().lines() {
    //   match line {
    //     Ok(line) => println!("{line}"),
    //     Err(e) => eprintln!("{e}"),
    //   }
    // }
}
