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
    println!("{:?}", parser.stmts);
    println!("{:?}", parser.exprs);
    println!("{:?}", parser.types);
    println!("{:?}", parser.idents);

    // let checker = check(parser);

    // for line in io::stdin().lines() {
    //   match line {
    //     Ok(line) => println!("{line}"),
    //     Err(e) => eprintln!("{e}"),
    //   }
    // }
}
