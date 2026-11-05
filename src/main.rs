use cmblang::*;
use std::{fs, io};

fn main() {
    println!("Hello!");

    let test = fs::read_to_string("test.cmb").unwrap();
    let lex = scan(test).unwrap();

    let parser = parse(&lex);

    let mut ast = Ast::new(lex, parser);
    let checker = typecheck(&mut ast);

    println!("{:?}", checker.types);
    println!("{:?}", checker.funcs);
    println!("{:?}", checker.structs);

    // let output = emit(&ast);
    // println!("{output}");

    // for line in io::stdin().lines() {
    //   match line {
    //     Ok(line) => println!("{line}"),
    //     Err(e) => eprintln!("{e}"),
    //   }
    // }
}
