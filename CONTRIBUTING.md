# Contributing to Typst.Native

Thank you for your interest in contributing! This guide covers the workflow for
submitting pull requests and, for maintainers, how to cut a release.

## Reporting Issues

Before writing code, please [open an issue](https://github.com/leus/Typst.Native/issues)
to discuss the change you'd like to make. This avoids duplicate effort and gives
maintainers a chance to provide early feedback.

## Pull Request Workflow

### 1. Fork & clone

```bash
git clone https://github.com/<your-user>/Typst.Native.git
cd Typst.Native
```

### 2. Create a feature branch

Branch from `main` with a descriptive name:

```bash
git checkout -b feature/my-feature
```

### 3. Set up the development environment

You will need:

- [.NET 8.0 SDK](https://dotnet.microsoft.com/) or later
- [Rust toolchain](https://rustup.rs/) (stable)

Build the native library and the .NET solution:

```bash
cd native/typst-ffi
cargo build --release
cd ../..
dotnet build
```

### 4. Make your changes

- Keep commits focused — one logical change per commit.
- Follow the existing code style in both Rust and C#.
- Add or update tests when applicable.

### 5. Run tests locally

```bash
# Rust tests
cd native/typst-ffi
cargo test --release
cd ../..

# .NET tests (requires the native library in runtimes/)
dotnet test
```

### 6. Push and open a PR

```bash
git push origin feature/my-feature
```

Then open a Pull Request against the `main` branch on GitHub.

**PR checklist:**

- [ ] PR targets the `main` branch.
- [ ] All CI checks pass (native builds + .NET build & test).
- [ ] New or changed functionality includes tests.
- [ ] Commit messages are clear and descriptive.

A maintainer will review your PR. Please be patient — we may request changes
before merging.

---

## Releasing (Maintainers)

Releases are fully automated via the
[Release workflow](.github/workflows/release.yml). Pushing a version tag
triggers the pipeline, which builds native libraries for all platforms, packs
NuGet packages, pushes them to NuGet.org, and creates a GitHub Release.

### Steps to release

1. **Ensure `main` is green.** Verify the latest CI run on `main` is passing.

2. **Decide on the version number.** Follow [Semantic Versioning](https://semver.org/):
   - **patch** (`0.1.1`) — bug fixes, no API changes.
   - **minor** (`0.2.0`) — new features, backwards-compatible.
   - **major** (`1.0.0`) — breaking API changes.

3. **Tag the release.** Create an annotated tag on `main` and push it:

   ```bash
   git checkout main
   git pull origin main
   git tag -a v0.2.0 -m "Release v0.2.0"
   git push origin v0.2.0
   ```

4. **Monitor the release workflow.** Go to
   [Actions → Release](https://github.com/leus/Typst.Native/actions/workflows/release.yml)
   and confirm the run completes successfully.

5. **Review the GitHub Release.** The workflow creates a release with
   auto-generated release notes. Edit the release description if needed.

### What the release pipeline does

| Step | Details |
|------|---------|
| Build native libraries | Compiles `typst-ffi` for win-x64, linux-x64, linux-arm64, osx-x64, osx-arm64 |
| Pack NuGet packages | Creates `Typst.Native` and `Typst.Native.Runtime` with the version from the tag |
| Push to NuGet.org | Publishes packages (skips duplicates) |
| GitHub Release | Creates a release with auto-generated notes and attaches the `.nupkg` files |

### Rolling back a release

If a release is defective:

1. Unlist the NuGet packages from nuget.org.
2. Delete the GitHub Release and the tag:
   ```bash
   git tag -d v0.2.0
   git push origin --delete v0.2.0
   ```
3. Fix the issue, then re-tag and push.
