# Spike: Whisper on Vulkan, CPU when there is none (ADR 0014, step 2)

Throwaway. Results and the decision are in ADR 0014, amendment "Vulkan spike".

`src/main.rs` transcribes a 16 kHz mono 16-bit WAV with whisper-rs 0.16 and prints
the time of each `full()` call. With `--features vulkan`, ggml-vulkan is linked
statically and the binary defines the three Vulkan loader symbols ggml calls
directly (`vkGetInstanceProcAddr`, `vkCmdCopyBuffer`,
`vkGetPhysicalDeviceFeatures2`), so it never imports `vulkan-1.dll`. The shim
loads the loader from System32 at run time; without it, it reports Vulkan 1.0,
ggml refuses the backend and Whisper runs on the CPU. `SPIKE_LOADER=<name>`
replaces the loader's file name to simulate a machine without it.

```
whisper-vulkan-spike <model.bin> <clip.wav> <cpu|gpu> [runs]
```

## Building on Windows

- `VULKAN_SDK` must point at a folder with `Include/` (Vulkan-Headers, including
  `vulkan.hpp`), `Lib/vulkan-1.lib` and `Bin/glslc.exe`. The LunarG SDK has all
  three. The spike was built with Vulkan-Headers v1.4.369, an import library
  generated from System32's `vulkan-1.dll` exports (`dumpbin /exports`, `lib /def`)
  and `glslc` from conda-forge's `shaderc` 2026.4 (plus `SPIRV-Tools*.dll`), so
  nothing was installed.
- `LIBCLANG_PATH=C:\Program Files\LLVM\bin`, as for the app.
- ggml-vulkan builds its shader generator as a nested CMake project; under a
  long target directory its compiler check fails on MAX_PATH ("No
  CMAKE_C_COMPILER could be found", "Invalid argument"). Keep the target
  directory short (the spike used a drive letter, `subst`, over the scratchpad)
  or use Ninja from a `vcvars64` shell (`CMAKE_GENERATOR=Ninja`).
- Set `CMAKE_C_FLAGS_RELEASE` / `CMAKE_CXX_FLAGS_RELEASE` (the repository's
  `.cargo/config.toml` does): without them the Visual Studio generator builds
  ggml without optimisation.
