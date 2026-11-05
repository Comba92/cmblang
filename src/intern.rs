use std::{collections::HashMap, hash};

fn hash<T: hash::Hash>(val: &T) -> u64 {
    use hash::Hasher;
    let mut hasher = hash::DefaultHasher::new();
    val.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default, Debug)]
pub struct StringInterner {
    hash_to_id: HashMap<u64, u32>,
    id_to_value: Vec<(usize, usize)>,
    buf: String,
}

impl StringInterner {
    pub fn add(&mut self, val: &str) -> u32 {
        let hash = hash(&val);
        if let Some(id) = self.hash_to_id.get(&hash) {
            return *id;
        }

        let id = self.id_to_value.len() as u32;
        self.hash_to_id.insert(hash, id);

        let start = self.buf.len();
        self.id_to_value.push((start, start + val.len()));

        self.buf.push_str(val);
        id
    }

    pub fn get(&self, id: u32) -> &str {
        let span = self.id_to_value[id as usize];
        &self.buf[span.0..span.1]
    }
}

#[derive(Debug)]
pub struct Interner<T> {
    hash_to_id: HashMap<u64, u32>,
    id_to_value: Vec<T>,
}

impl<T: hash::Hash> Interner<T> {
    pub fn new() -> Self {
        Self {
            hash_to_id: HashMap::new(),
            id_to_value: Vec::new(),
        }
    }

    pub fn is_interned(&self, val: &T) -> bool {
        let hash = hash(&val);
        self.hash_to_id.get(&hash).is_some()
    }

    pub fn add(&mut self, val: T) -> u32 {
        let hash = hash(&val);
        if let Some(id) = self.hash_to_id.get(&hash) {
            return *id;
        }

        let id = self.id_to_value.len() as u32;
        self.hash_to_id.insert(hash, id);
        self.id_to_value.push(val);
        id
    }

    pub fn get(&self, id: u32) -> &T {
        &self.id_to_value[id as usize]
    }
}
