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

### Images and virtual files

Supply in-memory files — images, data files, or `.typ` modules — that your
Typst source can reference by path:

```csharp
using var compiler = new TypstCompiler();
compiler.AddFile("logo.png", File.ReadAllBytes("logo.png"));

using var result = compiler.Compile("#image(\"logo.png\")");
```

Typst accepts PNG, JPEG, GIF, WebP, SVG, and PDF as `#image` sources. Files on
disk also work when a root directory is set via `SetRoot` (or implicitly by
`CompileFile`); virtual files added with `AddFile` take precedence over disk
and persist until `ClearFiles()` is called or the compiler is disposed.

### PNG rendering

Render any page of a successful compilation to a PNG image:

```csharp
using var result = compiler.Compile("Hello, PNG!");
byte[] png = result.RenderPng(pageIndex: 0, pixelsPerPoint: 2.0f); // 2.0 ≈ 144 DPI
File.WriteAllBytes("page1.png", png);
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

### Versioning

The **major and minor** versions of the NuGet packages always match the major
and minor versions of the bundled [Typst](https://github.com/typst/typst)
compiler: `Typst.Native 0.15.x` compiles with Typst `0.15`. The **patch**
version is independent and is incremented for fixes and improvements that
don't change the underlying Typst version.

When upgrading the Typst crates in `native/typst-ffi/Cargo.toml`, bump
`VersionPrefix` in `Directory.Build.props` accordingly.

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like to change.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes
4. Push to the branch and open a Pull Request

## License

This project is licensed under the [MIT License](LICENSE).

Typst itself is licensed under the Apache License 2.0. See the [Typst repository](https://github.com/typst/typst) for details.
