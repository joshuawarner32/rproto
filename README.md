# RProto (working title)
## Working towards making protocol buffers more usable from rust

At my day job, we maintain a moderately large rust codebase that mostly communicates with the outside world via protobuf. We use both GRPC for communicating with other system components and direct protobuf serialization for writing to our database.

Due to a variety of usability issues with modern protobuf rust generators, we've developed a pattern of writing a protobuf message and then usually having a second rust struct, with Into impls going both ways.

There are a few reasons for this.

### oneof naming is... unergonomic

Here's an example protobuf message that's _trying_ to be a rust enum:

```protobuf
message Example {
    oneof thing {
        Thing1 thing1 = 1;
        Thing2 thing2 = 2;
    }
}
```

This results in (roughly):

```rust
struct Example {
    thing: Example_Thing,
}

enum Example_Thing {
    Thing1(Thing1),
    Thing2(Thing2),
}
```

For some of these messages, there are many many instances where we `match` on the field type internally - and having that extra layer of wrapping can obscure the meaning of the match and generally clutter things up.

And so we left with writing our own (manual) `Into` impls that are mostly just boilerplate.

### Nullability is unintuitive

When we want to create a true `Option` field with a message type, we can just use the default "everything is optional" behavior. To create a non-nullable message type, we can use the `[(gogoproto.nullable)=false]` annotation.

However, the semantics of this is unintuitive. If the field wasn't encoded on the wire, this means we get a `Default` version of the message.

For many engineers, this is surprising.

There's also the issue (with proto3) of how to create an optional "scalar" field - i.e. a uint32, string, bytes, etc field that's `Option`al.

This requires wrapping the field in either a `message`, or a `oneof`.

### service methods lead to a lot of boilerplate

There are several cases where our rust component is communicating over grpc services to another rust component, where we have something like this:

```protobuf
service MyService {
    rpc MyMethod(MyMethodReq) returns (MyMethodRes) {}
}

message MyMethodReq {
    uint32 arg1 = 1;
    uint32 arg2 = 2;
}

message MyMethodRes {
    string result = 1;
    bool did_a_thing = 2;
}
```

This leads to us having to write a bunch of boilerplate in rust to encode/decode args and return values. Encoding/decoding boilerplate is something that protobuf is ostensibly supposed to be taking responsibility for, so this is mildly ironic.

# The pitch

Write rust struct/enum/trait declarations, and have a tool to transpile those into both:
* Rust code to do the canonical (de)serialization
* Protobuf messages to allow communication with other languages

Currently this repo is noting more than a partial-proof-of-concept, nowhere remotely production ready.

If you look at `ex/simple.rproto`:

```
struct File {
    contents: Vec<u8>,
}

struct Dir {
    entries: HashMap<String, DirEntry>,
}

enum DirEntry {
    File(File),
    Dir(Dir),
}
```

We can currently translate that to the following (correct-ish?) protobuf declarations:

```
message File {
  bytes contents = 1;
}
message Dir {
  map<String, DirEntry> entries = 1;
}
message DirEntry {
  oneof dir_entry {
    File File = 1;
    Dir Dir = 2;
  }
}
```
