# Sleepy

## Build dependencies

We recommend installing Rust through [rustup](https://www.rustup.rs/). If you don't already have rustup, you can install it like this:

- Linux:
	```bash
	$ curl https://sh.rustup.rs -sSf | sh
	```

	Sleepy also requires `gcc`, `g++`, and `libssl-dev`/`openssl` packages to be installed.


- OSX:
	```bash
	$ curl https://sh.rustup.rs -sSf | sh
	```

	`clang` is required. It comes with Xcode command line tools or can be installed with homebrew.


- Windows

    Make sure you have Visual Studio 2015 with C++ support installed. Next, download and run the rustup installer from
	https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe, start "VS2015 x64 Native Tools Command Prompt", and use the following command to install and set up the msvc toolchain:
    ```
	$ rustup default stable-x86_64-pc-windows-msvc
    ```

Once you have rustup, build Sleepy from source

----

## Build from source

You should install The PBC library first: https://crypto.stanford.edu/pbc/ .

```bash
# download Sleepy code
$ git clone https://github.com/cryptape/sleepy.git
$ cd sleepy

# build in release mode
$ cargo build --release
```

This will produce an executable in the `./target/release` subdirectory.

----

## Start Sleepy
### 1、generate config files
```bash
$ ./admintool/setup.sh
```

### 2、start Sleepy, just run
```bash
$ ./start.sh
```

and Sleepy will start four nodes and you can find the log in admintool/release/node{0,1,2,3}/log.
