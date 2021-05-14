use std::{collections::HashMap};


fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let file = args[0].as_str();
    
    let text = std::fs::read_to_string(file).unwrap();
    let tree: syn::File = syn::parse_str(&text).unwrap();

    let ir = load_proto(&tree);

    let mut e = Emitter::new();
    emit_proto(&ir, &mut e);
}

fn load_proto(file: &syn::File) -> IrModule {
    let mut ir = IrModule::new();
    for item in &file.items {
        match item {
            syn::Item::Enum(item) => ir.add_type(item.ident.to_string(), Ty::Enum(load_enum(item))),
            syn::Item::Struct(item) => ir.add_type(item.ident.to_string(), Ty::Struct(load_struct(item))),
            _ => panic!("only enums and structs allowed"),
        }
    }
    ir
}

fn load_struct(item: &syn::ItemStruct) -> StructTy {
    if item.generics.params.len() > 0 {
        panic!("no generics (yet)");
    }

    StructTy {
        fields: load_fields(&item.fields),
    }
}

fn load_fields(fields: &syn::Fields) -> Fields {
    if fields.iter().all(|f| f.ident.is_some()) {
        Fields::Struct(
            fields.iter().map(|f| {
                (f.ident.as_ref().unwrap().to_string(), type_ref(&f.ty))
            }).collect()
        )
    } else {
        Fields::Tuple(
            fields.iter().map(|f| type_ref(&f.ty)).collect()
        )
    }
}

enum TypeRef {
    Normal(String),
    Generic(String, Vec<TypeRef>),
}

fn type_ref(ty: &syn::Type) -> TypeRef {
    match ty {
        syn::Type::Path(p) => {
            assert!(p.qself.is_none());
            assert_eq!(p.path.segments.len(), 1);

            let seg = &p.path.segments[0];
            match &seg.arguments {
                syn::PathArguments::None => TypeRef::Normal(seg.ident.to_string()),
                syn::PathArguments::AngleBracketed(args) => {
                    TypeRef::Generic(seg.ident.to_string(), args.args.iter().map(|a| {
                        match a {
                            syn::GenericArgument::Type(ty) => type_ref(ty),
                            _ => panic!(),
                        }
                    }).collect())
                }
                syn::PathArguments::Parenthesized(_) => todo!(),
            }
        }
        _ => panic!("other types not allowed (yet)"),
    }
}

fn load_enum(item: &syn::ItemEnum) -> EnumTy {
    if item.generics.params.len() > 0 {
        panic!("no generics (yet)");
    }
    
    EnumTy {
        variants: item.variants.iter().map(|v| {
            (v.ident.to_string(), load_fields(&v.fields))
        }).collect(),
    }
}

struct IrModule {
    types: HashMap<String, Ty>,
    types_in_order: Vec<String>,
}

enum Ty {
    Struct(StructTy),
    Enum(EnumTy),
}

struct StructTy {
    fields: Fields,
}

struct EnumTy {
    variants: Vec<(String, Fields)>,
}

enum Fields {
    Tuple(Vec<TypeRef>),
    Struct(Vec<(String, TypeRef)>),
}

impl Fields {
    fn singleton(&self) -> Option<&TypeRef> {
        match self {
            Fields::Tuple(fields) => {
                if fields.len() == 1 {
                    Some(&fields[0])
                } else {
                    None
                }
            }
            Fields::Struct(fields) => {
                if fields.len() == 1 {
                    Some(&fields[0].1)
                } else {
                    None
                }
            }
        }
    }
}

impl IrModule {
    fn new() -> IrModule {
        IrModule {
            types: HashMap::new(),
            types_in_order: Vec::new(),
        }
    }

    fn add_type(&mut self, name: String, ty: Ty) {
        self.types.insert(name.clone(), ty);
        self.types_in_order.push(name);
    }
}

struct Emitter {
    buf: String,
    at_line_start: bool,
    indent: usize,
}

impl Emitter {
    fn new() -> Emitter {
        Emitter {
            buf: String::new(),
            at_line_start: true,
            indent: 0,
        }
    }

    fn _output(&mut self, text: &str) {
        self.buf.push_str(text);
        print!("{}", text);
    }

    fn text(&mut self, text: &str) {
        if self.at_line_start {
            for _ in 0..self.indent {
                self._output("  ");
            }
        }
        self.at_line_start = false;
        self._output(text);
    }

    fn line(&mut self) {
        self.text("\n");
        self.at_line_start = true;
    }

    fn begin_message(&mut self, name: &str) {
        self.text("message ");
        self.text(name);
        self.text(" {");
        self.indent += 1;
        self.line();
    }

    fn begin_oneof(&mut self, name: &str) {
        self.text("oneof ");
        self.text(name);
        self.text(" {");
        self.indent += 1;
        self.line();
    }

    fn repeated_field(&mut self, name: &str, ty: &str, id: &mut usize) {
        self.text("repeated ");
        self.plain_field(name, ty, id);
    }

    fn plain_field(&mut self, name: &str, ty: &str, id: &mut usize) {
        self.field_with_annotations(name, ty, id, |_| {});
    }

    fn defaulted_field(&mut self, name: &str, ty: &str, id: &mut usize) {
        self.field_with_annotations(name, ty, id, |s| {
            s.text(" [(gogoproto.nullable)=false]");
        });
    }

    fn field_with_annotations(&mut self, name: &str, ty: &str, id: &mut usize, f: impl FnOnce(&mut Self)) {
        self.text(ty);
        self.text(" ");
        self.text(name);
        self.text(" = ");
        self.text(&format!("{}", &id));
        f(self);
        self.text(";");
        self.line();
        *id += 1;
    }

    fn end(&mut self) {
        self.indent -= 1;
        self.text("}");
        self.line();
    }
}

fn emit_proto(ir: &IrModule, e: &mut Emitter) {
    for name in &ir.types_in_order {
        let ty = &ir.types[name];

        match ty {
            Ty::Struct(item) => emit_struct(ir, name, item, e),
            Ty::Enum(item) => emit_enum(ir, name, item, e),
        }
    }
}

fn emit_enum(ir: &IrModule, name: &str, item: &EnumTy, e: &mut Emitter) {
    e.begin_message(name);
    e.begin_oneof(&to_underscore_case(&name));

    let mut id = 1;

    let mut to_append = Vec::new();
    for (field_name, fields) in &item.variants {
        if let Some(ty) = fields.singleton() {
            emit_field(field_name, ty, &mut id, e);
        } else {
            to_append.push((field_name, fields));

            e.plain_field(field_name, &format!("{}{}", name, field_name), &mut id);
        }
    }

    e.end();
    e.end();

    for (field_name, fields) in to_append {
        emit_fields(ir, field_name, fields, e);
    }
}

fn emit_fields(ir: &IrModule, name: &str, fields: &Fields, e: &mut Emitter) {
    e.begin_message(name);
    let mut id = 1;
    match fields {
        Fields::Tuple(_) => panic!(),
        Fields::Struct(fields) => {
            for (field_name, ty) in fields {
                emit_field(field_name, ty, &mut id, e);
            }
        }
    }
    e.end();
}

fn emit_field(field_name: &str, ty: &TypeRef, id: &mut usize, e: &mut Emitter) {
    match ty {
        TypeRef::Normal(ty) => {
            if let Some(simple) = translate_simple_type_name(ty) {
                e.plain_field(field_name, simple, id);
            } else {
                e.defaulted_field(field_name, ty, id);
            };
        }
        TypeRef::Generic(ty, args) => {
            match ty.as_str() {
                "Vec" => {
                    let single = singular(args).unwrap();
                    match single {
                        TypeRef::Normal(name) => {
                            if name == "u8" {
                                e.defaulted_field(field_name, "bytes", id);
                            } else {
                                e.repeated_field(field_name, name, id);
                            }
                        }
                        TypeRef::Generic(_, _) => panic!(),
                    }
                }
                "HashMap" => {
                    let (k, v) = double(args).unwrap();
                    e.plain_field(field_name, &format!("map<{}, {}>", simple_type(k), simple_type(v)), id);
                }
                "Option" => {
                    let single = singular(args).unwrap();
                    match single {
                        TypeRef::Normal(name) => {
                            if let Some(simple) = translate_simple_type_name(name) {
                                // optional must be encoded as a nullable oneof
                                e.begin_oneof(&format!("{}_value", to_underscore_case(&field_name)));
                                e.defaulted_field(field_name, simple, id);
                                e.end();
                            } else {
                                e.plain_field(field_name, ty, id);
                            }
                        }
                        TypeRef::Generic(_, _) => panic!(),
                    }
                }
                _ => panic!(),
            }
        }
    }
}

fn translate_simple_type_name(name: &str) -> Option<&str> {
    match name {
        "u8" | "u16" | "u32" => Some("uint32"),
        "u64" => Some("uint64"),
        "i8" | "i16" | "i32" => Some("int32"),
        "i64" => Some("int64"),
        "f32" => Some("float"),
        "f64" => Some("double"),
        "bool" => Some("bool"),
        _ => None,
    }
}

fn simple_type(ty: &TypeRef) -> &str {
    match ty {
        TypeRef::Normal(ty) => ty,
        TypeRef::Generic(_, _) => panic!(),
    }
}

fn singular<T>(args: &[T]) -> Option<&T> {
    if args.len() == 1 {
        Some(&args[0])
    } else {
        None
    }
}

fn double<T>(args: &[T]) -> Option<(&T, &T)> {
    if args.len() == 2 {
        Some((&args[0], &args[1]))
    } else {
        None
    }
}

fn emit_struct(ir: &IrModule, name: &str, item: &StructTy, e: &mut Emitter) {
    emit_fields(ir, name, &item.fields, e)
}

fn to_underscore_case(name: &str) -> String {
    let mut s = String::new();
    for ch in name.chars() {
        if ch.is_uppercase() {
            if s.len() > 0 {
                s.push('_');
            }
            s.push_str(&ch.to_lowercase().to_string());
        } else {
            s.push(ch);
        }
    }
    s
}
