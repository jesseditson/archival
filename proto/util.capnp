@0x92f1d8c1dd43e3dc;

struct Map(Key, Value) {
  entries @0 :List(Entry);
  struct Entry {
    key @0 :Key;
    value @1 :Value;
  }
}

struct Option(Value) {
  union {
    none @0 :Void;
    some @1 :Value;
  }
}
