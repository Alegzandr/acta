# Building Acta from source

Install Rust with [rustup](https://rustup.rs); the toolchain is pinned in
`rust-toolchain.toml` and installed on first use. Then, per system:

- **Windows:** Visual Studio Build Tools with the Windows SDK, and LLVM (libclang, for
  the Whisper bindings; set `LIBCLANG_PATH` if it is not in `C:\Program Files\LLVM\bin`).
  The encrypted storage builds a vendored OpenSSL, which needs a native Perl such as
  [Strawberry Perl](https://strawberryperl.com); Git's Perl does not work. Point
  `OPENSSL_SRC_PERL` at its `perl.exe` if it is not first on your PATH. Dictation's
  Vulkan backend needs the [Vulkan SDK](https://vulkan.lunarg.com/sdk/home) (free; set
  `VULKAN_SDK`, which its installer does). Keep the target directory on a short path:
  OpenSSL's Configure and the shader generator's nested CMake build fail on long paths
  (beyond 260 characters).
- **macOS:** Xcode command line tools and CMake.
- **Linux:** the packages listed in `scripts/ci/linux-deps.sh` (Ubuntu names), and the
  Vulkan SDK's headers and `glslc`: `scripts/ci/vulkan-sdk.sh <folder>` fetches them,
  then set `VULKAN_SDK` to that folder (a distribution's `glslc` package works too).

Run the app:

```sh
cargo run -p acta-app
```

`cargo build --release --locked` produces `target/release/Acta` (the app) and
`target/release/acta-cli` (the command line, installed as `acta`). Packaging and
releases are described in [releasing.md](releasing.md).
