extern crate gcc;

fn main() {
    gcc::compile_library("libtinykeccak.a", &["src/tinykeccak.c"]);
}

