pub mod ast;
pub mod checker;
// pub mod emitter;
mod intern;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use checker::*;
// pub use emitter::*;
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

// TODO: this datatype is retarted
// T and Span/Position should stay in different arrays.
// we only care about spans when errors occurs after all
pub type Spanned<T> = (T, Span);
pub type Positioned<T> = (T, Position);

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub line_start: u32,
    pub line_end: u32,
    pub col_start: u32,
    pub col_end: u32,
}

#[derive(Debug)]
pub enum ErrSource {
    Parser,
    Checker,
}
pub fn render_err<E: std::fmt::Debug>(lex: &Lexer, e: &E, span: Span, src: ErrSource) -> String {
    // TODO: line offsets might be retarded, just get the current line until the newline?

    //     let line_start = lex
    //         .input
    //         .char_indices()
    //         .take(e.1.start as usize)
    //         .find_map(|(i, c)| (c == '\n').then(|| i as u32))
    //         .unwrap_or(0);
    //
    //     let line_end = lex
    //         .input
    //         .char_indices()
    //         .skip(e.1.start as usize)
    //         .find_map(|(i, c)| (c == '\n').then(|| i as u32))
    //         .unwrap_or(lex.input.len() as u32);

    // let curr_line = lex
    //     .str_from_span(Span {
    //         start: line_start,
    //         end: line_end,
    //     })
    //     .trim();

    let pos = lex.pos_from_span(span);
    let curr_line = lex.str_line_from_pos(&pos);
    let filename = "test.cmb"; // TODO: add filename here

    let mut str = format!("error ({:?}): {:?} ", src, e);
    str += &format!("at {filename}:{}:{}\n", pos.line_start, pos.col_start);
    str += &format!("\t{curr_line}\n");

    let max_underline_len = (curr_line.len() as u32).saturating_sub(pos.col_start) + 1;
    let left = " ".repeat(pos.col_start.saturating_sub(1) as usize);
    let underline = "^".repeat(span.len().min(max_underline_len) as usize);
    str += &format!("\t{left}{underline}");

    str
}

pub mod UnionFind {
    #[derive(Default)]
    pub struct UfNode<T> {
        pub payload: T,
        parent: u32,
        size: u32,
    }

    #[derive(Default)]
    pub struct UnionFind<T> {
        forest: Vec<UfNode<T>>,
    }
    impl<T: Eq> UnionFind<T> {
        pub fn new() -> Self {
            Self { forest: vec![] }
        }

        fn search_idx(&self, val: &T) -> Option<usize> {
            self.forest.iter().position(|x| x.payload == *val)
        }

        pub fn make_set(&mut self, val: T) -> u32 {
            match self.search_idx(&val) {
                Some(idx) => idx as u32,
                None => {
                    let id = self.forest.len() as u32;
                    self.forest.push(UfNode {
                        parent: id,
                        payload: val,
                        size: 1,
                    });
                    id
                }
            }
        }

        pub fn find(&mut self, val: &T) -> Option<&UfNode<T>> {
            if let Some(node_id) = self.search_idx(val) {
                let id = self.find_idx(node_id as u32);
                Some(&self.forest[id as usize])
            } else {
                None
            }
        }

        fn get(&self, idx: u32) -> &UfNode<T> {
            &self.forest[idx as usize]
        }

        fn get_mut(&mut self, idx: u32) -> &mut UfNode<T> {
            &mut self.forest[idx as usize]
        }

        // implemented with path compression
        fn find_idx(&mut self, node_id: u32) -> u32 {
            let parent_id = self.get(node_id).parent;

            if parent_id == node_id {
                // if its representative return
                node_id
            } else {
                // else go up to the parent recursively
                let new_parent_id = self.find_idx(parent_id);
                let node = self.get_mut(node_id);
                node.parent = new_parent_id;
                node_id
            }
        }

        pub fn union(&mut self, x: &T, y: &T) -> bool {
            let x_idx = self.search_idx(x);
            let y_idx = self.search_idx(y);

            match (x_idx, y_idx) {
                (Some(x), Some(y)) => {
                    self.union_idx(x as u32, y as u32);
                    true
                }
                _ => false,
            }
        }

        fn union_idx(&mut self, x: u32, y: u32) {
            // replace nodes by representatives
            let mut x = self.find_idx(x);
            let mut y = self.find_idx(y);

            if x == y {
                // x and y are already in the same set
                return;
            }

            // size is used to prevent unbalanced trees
            // use the node with less descendants
            if self.get(x).size < self.get(y).size {
                std::mem::swap(&mut x, &mut y);
            }

            // make x the new root
            self.get_mut(y).parent = x;

            // update size of x
            let new_size = self.get(x).size + self.get(y).size;
            self.get_mut(x).size = new_size;
        }
    }
}
