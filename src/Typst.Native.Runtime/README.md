# Typst.Native.Runtime

Pre-built native libraries (`typst_ffi`) for the [Typst.Native](https://www.nuget.org/packages/Typst.Native) package.

This package is a dependency of `Typst.Native` and is not intended to be referenced directly.
Install `Typst.Native` instead:

```
dotnet add package Typst.Native
```

## Contents

This package bundles the `typst_ffi` shared library for the following platforms:

| Runtime       | RID            | Library              |
|---------------|----------------|----------------------|
| Windows x64   | `win-x64`      | `typst_ffi.dll`      |
| Linux x64     | `linux-x64`    | `libtypst_ffi.so`    |
| Linux ARM64   | `linux-arm64`  | `libtypst_ffi.so`    |
| macOS x64     | `osx-x64`      | `libtypst_ffi.dylib` |
| macOS ARM64   | `osx-arm64`    | `libtypst_ffi.dylib` |

The correct binary is automatically copied to the output directory at build time via the included MSBuild targets.

## License

MIT. See the [repository](https://github.com/leus/Typst.Native) for details.
