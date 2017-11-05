// build.rs

// Bring in a dependency on an externally maintained `gcc` package which manages
// invoking the C compiler.
extern crate gcc;

fn main() {
    gcc::Build::new()
                .file("src/bls.c")
                .include("/usr/local/include/pbc")
                .static_flag(true)
                .compile("libbls.a");
}