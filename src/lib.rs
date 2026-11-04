pub mod ast;
pub mod checker;
pub mod emitter;
mod intern;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use checker::*;
pub use emitter::*;
pub use lexer::*;
pub use parser::*;

fn has_duplicates<A, B, M>(list: &[A], map: M) -> bool
where
    B: Eq + std::hash::Hash,
    M: FnMut(&A) -> B,
{
    let set: std::collections::HashSet<_> = list.iter().map(map).collect();
    set.len() != list.len()
}

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}
impl Span {
    pub fn len(&self) -> u32 {
        self.end - self.start
    }
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
    // pub fn get_str<'a, 'b>(&'a self, src: &'b str) -> &'b str { &src[self.start as usize .. self.end as usize] }
}
pub type Spanned<T> = (T, Span);

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub line: u32,
    pub col: u32,
}
pub type Positioned<T> = (T, Position);

// trait CursorIter<Inner> {
//   fn peek(&self) -> Inner { self.peek_nth(0) }
//   fn peek_nth(&self, nth: usize) -> Inner;
//   fn advance(&mut self) { self.advance_nth(1); }
//   fn advance_nth(&mut self, n: usize) { *self.curr_mut() += n as u32; }
//   fn undo(&mut self) { *self.curr_mut() = self.curr().saturating_sub(1); }
//
//   fn eat(&mut self) -> Inner {
//     let res = self.peek();
//     self.advance();
//     res
//   }
//
//   fn start(&self) -> &[Inner];
//   fn slice(&self) -> &[Inner] { &self.start()[self.curr() as usize..] }
//   fn curr(&self) -> u32;
//   fn curr_mut(&mut self) -> &mut u32;
//   fn has_some(&self) -> bool { self.curr() < self.start().len() as u32 }
// }
