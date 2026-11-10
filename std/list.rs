struct List<T> {
  data: *T;
  u32 size;
  u32 cap;

  fn new() -> List<T> {
    return with_cap(16);
  }

  fn with_cap(cap: u32) -> List<T> {
    List {
      data: @malloc(16),
      size: 0,
      cap: 16,
    };
  }

  fn push(T val) {
    data[size] = val;
    size += 1;
  }

  fn pop() -> Maybe<T> {
    if size == 0 { return None };
    let res = data[size];
    size -= 1;
    return Maybe(res);
  }

  fn clear() {
    size = 0;
  }
}
