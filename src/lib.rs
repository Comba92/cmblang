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
pub type Spanned<T> = (T, Span);

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub line: u32,
    pub col: u32,
}
pub type Positioned<T> = (T, Position);

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
