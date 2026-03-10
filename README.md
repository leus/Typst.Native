# Typst.Native

[![CI](https://github.com/leus/Typst.Native/actions/workflows/ci.yml/badge.svg)](https://github.com/leus/Typst.Native/actions/workflows/ci.yml)
[![Security Audit](https://github.com/leus/Typst.Native/actions/workflows/security-audit.yml/badge.svg)](https://github.com/leus/Typst.Native/actions/workflows/security-audit.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

.NET bindings for [Typst](https://typst.app/), the modern markup-based typesetting system.

**Typst.Native** lets you compile Typst documents to PDF, SVG, and PNG directly from .NET — no CLI installation or external processes required.

## Features

- **Native performance**: calls Typst's Rust core via P/Invoke (no WASM overhead)
- **Cross-platform**: ships pre-built native libraries for Windows, Linux, and macOS (x64 + ARM64)
- **Zero external dependencies**: everything is bundled in the NuGet package
- **Idiomatic C# API**: `TypstCompiler`, diagnostics, streaming output

## Quick Start

```bash
dotnet add package Typst.Native
```

```csharp
using Typst.Native;

using var compiler = new TypstCompiler();

var result = compiler.Compile("Hello, *Typst* from .NET!");

if (result.IsSuccess)
{
    byte[] pdf = result.ToPdf();
    File.WriteAllBytes("output.pdf", pdf);
}
else
{
    foreach (var diag in result.Diagnostics)
        Console.Error.WriteLine(diag);
}
```

## Supported Platforms

| Runtime         | NuGet RID         | CI Runner                          |
|----------------|-------------------|------------------------------------|
| Windows x64    | `win-x64`         | `windows-latest`                   |
| Linux x64      | `linux-x64`       | `ubuntu-latest`                    |
| Linux ARM64    | `linux-arm64`     | `ubuntu-24.04-arm`                 |
| macOS x64      | `osx-x64`         | `macos-latest` (cross-compile)     |
| macOS ARM64    | `osx-arm64`       | `macos-latest`                     |

## Building from Source

### Prerequisites

- [.NET 8.0 SDK](https://dotnet.microsoft.com/) or later
- [Rust toolchain](https://rustup.rs/) (stable)

### Build the native library

```bash
cd native/typst-ffi
cargo build --release
```

The shared library will be output to `native/typst-ffi/target/release/`.

### Build the .NET solution

```bash
dotnet build
dotnet test
```

> **Note:** To run tests locally, you need to first build the native library and place it
> in the appropriate `runtimes/{rid}/native/` directory under `src/Typst.Native.Runtime/`.

## Architecture

```
Typst.Native/
├── native/typst-ffi/          # Rust FFI crate — thin C API over Typst
├── src/
│   ├── Typst.Native/           # Managed C# wrapper (main NuGet: Typst.Native)
│   ├── Typst.Native.Runtime/   # Native binaries package (Typst.Native.Runtime)
│   └── Typst.Native.Tests/     # Unit & integration tests
└── .github/workflows/         # CI/CD pipelines
```

### Packages

| Package               | Description                                      |
|-----------------------|--------------------------------------------------|
| `Typst.Native`         | Managed wrapper — the package you reference       |
| `Typst.Native.Runtime` | Pre-built native libraries for all platforms      |

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like to change.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes
4. Push to the branch and open a Pull Request

## License

This project is licensed under the [MIT License](LICENSE).

Typst itself is licensed under the Apache License 2.0. See the [Typst repository](https://github.com/typst/typst) for details.
