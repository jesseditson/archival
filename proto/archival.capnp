@0xaf6782dafa40b446;
using import "util.capnp".Map;
using import "util.capnp".Option;

# Objects

struct DisplayType {
    union {
        image @0 :Void;
        video @1 :Void;
        audio @2 :Void;
        download @3 :Void;
    }
}

struct MetaValue {
    union {
        string @0 :Text;
        number @1 :Float64;
        boolean @2 :Bool;
        dateString @3 :Text;
        array @4 :List(MetaValue);
        map @5 :List(Meta);
    }
}

struct Meta {
  entries @0 :List(Entry);
  struct Entry {
    name @0 :Text;
    value @1 :MetaValue;
  }
}

struct File {
    displayType @0 :DisplayType;
    filename @1 :Text;
    sha @2 :Text;
    mime @3 :Text;
    name @4 :Option(Text);
    description @5 :Option(Text);
}

struct FieldValue {
  union {
    string @0 :Text;
    enum @1 :Text;
    markdown @2 :Text;
    number @3 :Float64;
    dateString @4 :Text;
    objects @5 :List(ObjectValues);
    boolean @6 :Bool;
    file @7 :File;
    meta @8 :Meta;
    null @9 :Void;
  }
}

struct ObjectValues {
  fields @0 :List(Field);
  struct Field {
    name @0 :Text;
    value @1 :FieldValue;
  }
}

struct Object {
  filename @0 :Text;
  objectName @1 :Text;
  order :union {
    none @2 :Void;
    some @3 :Float64;
  }
  path @4 :Text;
  values @5 :ObjectValues;
}

struct ObjectEntry {
  union {
    list @0 :List(Object);
    object @1 :Object;
  }
}

struct ObjectMap {
  entries @0 :List(Entry);
  struct Entry {
    name @0 :Text;
    object @1 :ObjectEntry;
  }
}

# Object Definitions

struct AliasType {
    type @0 :FieldType;
    name @1 :Text;
}

struct FieldType {
    union {
      string @0 :Void;
      number @1 :Void;
      date @2 :Void;
      enum @3 :List(Text);
      markdown @4 :Void;
      boolean @5 :Void;
      image @6 :Void;
      video @7 :Void;
      upload @8 :Void;
      audio @9 :Void;
      meta @10 :Void;
      alias @11 : AliasType;
    }
}

struct FieldsMap {
  fields @0 :List(Field);
  struct Field {
    name @0 :Text;
    type @1 :FieldType;
  }
}

struct ChildDefinitions {
  children @0 :List(Child);
  struct Child {
    name @0 :Text;
    definition @1 :ObjectDefinition;
  }
}

struct ObjectDefinition {
    name @0 :Text;
    fields @1 :FieldsMap;
    template @2 :Option(Text);
    children @3 :ChildDefinitions;
}

# UI

struct ValuePathComponent {
    union {
        key @0 :Text;
        index @1 :UInt32;
    }
}

struct ValuePath {
    path @0: List(ValuePathComponent);
}

# Events

struct AddObjectValue {
    path @0 :ValuePath;
    value @1 :FieldValue;
}

struct RenameObjectEvent {
    object @0 :Text;
    from @1 :Text;
    to @2 :Text;
}
struct AddObjectEvent {
    object @0 :Text;
    filename @1 :Text;
    order :union {
        none @2 :Void;
        some @3 :Float64;
    }
    values @4 :List(AddObjectValue);
}
struct AddRootObjectEvent {
    object @0 :Text;
    values @1 :List(AddObjectValue);
}
struct DeleteObjectEvent {
    object @0 :Text;
    filename @1 :Text;
    source @2 :Option(Text);
}
struct EditFieldEvent {
    object @0 :Text;
    filename @1 :Text;
    path @2 :ValuePath;
    field @3 :Text;
    value @4 :Option(FieldValue);
    source @5 :Option(Text);
}
struct EditOrderEvent {
    object @0 :Text;
    filename @1 :Text;
    order :union {
        none @2 :Void;
        some @3 :Float64;
    }
    source @4 :Option(Text);
}
struct AddChildEvent {
    object @0 :Text;
    filename @1 :Text;
    path @2 :ValuePath;
    values @3 :List(AddObjectValue);
    index :union {
        none @4 :Void;
        some @5 :UInt32;
    }
}
struct RemoveChildEvent {
    object @0 :Text;
    filename @1 :Text;
    path @2 :ValuePath;
    source @3 :Option(Text);
}

struct ArchivalEvent {
    union {
        renameObject @0 :RenameObjectEvent;
        addObject @1 :AddObjectEvent;
        addRootObject @2 :AddRootObjectEvent;
        deleteObject @3 :DeleteObjectEvent;
        editField @4 :EditFieldEvent;
        editOrder @5 :EditOrderEvent;
        addChild @6 :AddChildEvent;
        removeChild @7 :RemoveChildEvent;
    }
}
