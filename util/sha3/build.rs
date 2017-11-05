extern crate gcc;

fn main() {
    gcc::Build::new()
                .file("src/tinykeccak.c")
                .static_flag(true)
                .compile("libtinykeccak.a");
}

