using System.Runtime.InteropServices;
using System.Text;
using Typst.Native.Interop;

namespace Typst.Native;

/// <summary>
/// The primary entry point for compiling Typst documents from .NET.
/// </summary>
/// <remarks>
/// <para>
/// Each <see cref="TypstCompiler"/> instance wraps a native Typst compiler
/// handle. You may create multiple instances for independent compilation
/// contexts, but each instance is <b>not</b> thread-safe — do not call
/// <see cref="Compile(string)"/> concurrently on the same instance.
/// </para>
/// <para>
/// Dispose the compiler when you are done to release native resources.
/// </para>
/// </remarks>
/// <example>
/// <code>
/// using var compiler = new TypstCompiler();
/// compiler.AddFontPath(@"C:\Fonts");
///
/// using var result = compiler.Compile("= Hello\nThis is *Typst*!");
///
/// if (result.IsSuccess)
/// {
///     File.WriteAllBytes("output.pdf", result.ToPdf());
/// }
/// </code>
/// </example>
public sealed class TypstCompiler : IDisposable
{
    private SafeCompilerHandle? _handle;
    private bool _disposed;

    /// <summary>
    /// The root last set by a caller through <see cref="SetRoot"/>, or
    /// <see langword="null"/> if the caller never set one.
    /// </summary>
    /// <remarks>
    /// <see cref="CompileFile"/> sets a root of its own when the caller has
    /// not. That implicit root is deliberately not recorded here, so that a
    /// first <see cref="CompileFile"/> call cannot masquerade as a
    /// caller-chosen root on a later one.
    /// </remarks>
    private string? _explicitRoot;

    /// <summary>
    /// Initializes a new <see cref="TypstCompiler"/> instance.
    /// </summary>
    /// <exception cref="TypstException">
    /// Thrown if the native library could not be loaded or the compiler
    /// could not be created.
    /// </exception>
    public TypstCompiler()
    {
        IntPtr ptr = NativeMethods.typst_compiler_new();
        if (ptr == IntPtr.Zero)
            throw new TypstException("Failed to create native Typst compiler.", NativeMethods.TYPST_ERR_INTERNAL);

        _handle = new SafeCompilerHandle(ptr);
    }

    /// <summary>
    /// Gets the version string of the underlying native <c>typst_ffi</c> library.
    /// </summary>
    public static string NativeVersion
    {
        get
        {
            IntPtr ptr = NativeMethods.typst_version();
            return Marshal.PtrToStringUTF8(ptr) ?? "unknown";
        }
    }

    /// <summary>
    /// Sets the root directory used to resolve <c>#import</c>,
    /// <c>#include</c>, <c>#read</c> and <c>#image</c> paths in Typst source.
    /// Typst refuses to read anything outside this directory.
    /// </summary>
    /// <param name="rootPath">Absolute path to the root directory.</param>
    /// <remarks>
    /// A root set here is honoured by <see cref="CompileFile"/>, which places
    /// the entry file at its real position underneath it. Setting a root wide
    /// enough to span sibling directories is what allows an entry file to
    /// reach them with <c>../</c>.
    /// </remarks>
    /// <exception cref="ArgumentNullException">
    /// Thrown if <paramref name="rootPath"/> is <see langword="null"/>.
    /// </exception>
    /// <exception cref="TypstException">
    /// Thrown if the native call fails.
    /// </exception>
    public void SetRoot(string rootPath)
    {
        ArgumentNullException.ThrowIfNull(rootPath);
        SetRootCore(rootPath);
        _explicitRoot = Path.GetFullPath(rootPath);
    }

    /// <summary>
    /// Sets the native root without recording it as a caller-chosen root.
    /// </summary>
    private void SetRootCore(string rootPath)
    {
        ThrowIfDisposed();

        int rc = NativeMethods.typst_compiler_set_root(
            _handle!.DangerousGetHandle(), rootPath);

        ThrowOnError(rc, $"Failed to set root to '{rootPath}'");
    }

    /// <summary>
    /// Adds a directory to the list of paths searched for font files.
    /// </summary>
    /// <param name="fontPath">Absolute path to a directory containing font files.</param>
    /// <exception cref="ArgumentNullException">
    /// Thrown if <paramref name="fontPath"/> is <see langword="null"/>.
    /// </exception>
    public void AddFontPath(string fontPath)
    {
        ArgumentNullException.ThrowIfNull(fontPath);
        ThrowIfDisposed();

        int rc = NativeMethods.typst_compiler_add_font_path(
            _handle!.DangerousGetHandle(), fontPath);

        ThrowOnError(rc, $"Failed to add font path '{fontPath}'");
    }

    /// <summary>
    /// Registers an in-memory file that Typst source can reference by path,
    /// for example <c>#image("logo.png")</c> or <c>#import "helper.typ"</c>.
    /// </summary>
    /// <param name="path">
    /// Virtual path of the file, rooted at the compilation root:
    /// <c>"logo.png"</c> and <c>"/logo.png"</c> are equivalent. Forward and
    /// backward slashes are both accepted as separators. The entry source
    /// always shadows a virtual file at the same path, so avoid the path the
    /// entry occupies: <c>"main.typ"</c> for <see cref="Compile(string)"/>,
    /// or whatever path was given to <see cref="Compile(string, string)"/> or
    /// derived by <see cref="CompileFile"/>.
    /// </param>
    /// <param name="data">
    /// The file contents, e.g. raw image bytes. Typst decodes PNG, JPEG, GIF,
    /// WebP, SVG, and PDF as <c>#image</c> sources; unsupported or corrupt
    /// data surfaces as a compile diagnostic. Font files cannot be registered
    /// this way — use <see cref="AddFontPath"/> instead.
    /// </param>
    /// <remarks>
    /// Virtual files take precedence over files on disk under the root set
    /// via <see cref="SetRoot"/>. Adding a file with an existing path
    /// overwrites the previous contents. Files persist across
    /// <see cref="Compile(string)"/> calls until <see cref="ClearFiles"/> is called
    /// or the compiler is disposed; each file is held once in native memory.
    /// </remarks>
    /// <exception cref="ArgumentNullException">
    /// Thrown if <paramref name="path"/> or <paramref name="data"/> is <see langword="null"/>.
    /// </exception>
    /// <exception cref="ArgumentException">
    /// Thrown if <paramref name="path"/> is empty or whitespace.
    /// </exception>
    /// <exception cref="TypstException">
    /// Thrown if the native call fails.
    /// </exception>
    public void AddFile(string path, byte[] data)
    {
        ArgumentNullException.ThrowIfNull(data);
        AddFile(path, data.AsSpan());
    }

    /// <inheritdoc cref="AddFile(string, byte[])"/>
    public unsafe void AddFile(string path, ReadOnlySpan<byte> data)
    {
        ArgumentNullException.ThrowIfNull(path);
        if (string.IsNullOrWhiteSpace(path))
            throw new ArgumentException(
                "Virtual file path must not be empty or whitespace.", nameof(path));
        ThrowIfDisposed();

        int rc;
        fixed (byte* dataPtr = data)
        {
            // dataPtr is null for empty spans; length 0 is valid natively.
            rc = NativeMethods.typst_compiler_add_file(
                _handle!.DangerousGetHandle(), path, dataPtr, data.Length);
        }

        ThrowOnError(rc, $"Failed to add virtual file '{path}'");
    }

    /// <summary>
    /// Removes all in-memory files previously registered with
    /// <see cref="AddFile(string, byte[])"/>.
    /// </summary>
    public void ClearFiles()
    {
        ThrowIfDisposed();

        int rc = NativeMethods.typst_compiler_clear_files(
            _handle!.DangerousGetHandle());

        ThrowOnError(rc, "Failed to clear virtual files");
    }

    /// <summary>
    /// Compiles a Typst source string and returns the result.
    /// </summary>
    /// <param name="source">The Typst markup source code.</param>
    /// <returns>
    /// A <see cref="TypstCompileResult"/> that indicates success or failure.
    /// Dispose it after extracting the outputs you need.
    /// </returns>
    /// <exception cref="ArgumentNullException">
    /// Thrown if <paramref name="source"/> is <see langword="null"/>.
    /// </exception>
    /// <exception cref="TypstException">
    /// Thrown if the compilation call itself fails (not the same as a
    /// compilation error in the Typst source, which is reported via
    /// <see cref="TypstCompileResult.Diagnostics"/>).
    /// </exception>
    public TypstCompileResult Compile(string source)
    {
        ArgumentNullException.ThrowIfNull(source);
        return CompileCore(source, virtualPath: null);
    }

    /// <summary>
    /// Compiles a Typst source string as though it were the file at
    /// <paramref name="virtualPath"/>, so that relative paths inside it
    /// resolve from that location.
    /// </summary>
    /// <param name="source">The Typst markup source code.</param>
    /// <param name="virtualPath">
    /// Where the source sits relative to the root set by
    /// <see cref="SetRoot"/>: <c>"sub/main.typ"</c> and <c>"/sub/main.typ"</c>
    /// are equivalent, and both slash styles are accepted as separators. The
    /// file need not exist on disk; the text passed here is what gets
    /// compiled, and it shadows any real or virtual file of the same path.
    /// </param>
    /// <remarks>
    /// <para>
    /// Typst resolves a relative path against the directory of the file doing
    /// the referencing. The single-argument <see cref="Compile(string)"/>
    /// overload has no location to resolve against, so it behaves as though
    /// the source sat directly at the root, and any leading <c>../</c> in it
    /// escapes the root and fails. Give the source a location here and
    /// <c>../</c> works exactly as it does for the <c>typst</c> command line
    /// tool.
    /// </para>
    /// </remarks>
    /// <returns>
    /// A <see cref="TypstCompileResult"/> that indicates success or failure.
    /// Dispose it after extracting the outputs you need.
    /// </returns>
    /// <exception cref="ArgumentNullException">
    /// Thrown if <paramref name="source"/> or <paramref name="virtualPath"/>
    /// is <see langword="null"/>.
    /// </exception>
    /// <exception cref="ArgumentException">
    /// Thrown if <paramref name="virtualPath"/> is empty, is whitespace, names
    /// the root itself, or points outside the root.
    /// </exception>
    public TypstCompileResult Compile(string source, string virtualPath)
    {
        ArgumentNullException.ThrowIfNull(source);
        ArgumentNullException.ThrowIfNull(virtualPath);
        if (string.IsNullOrWhiteSpace(virtualPath))
            throw new ArgumentException(
                "Virtual path must not be empty or whitespace.", nameof(virtualPath));

        return CompileCore(source, virtualPath);
    }

    private unsafe TypstCompileResult CompileCore(string source, string? virtualPath)
    {
        ThrowIfDisposed();

        byte[] sourceBytes = Encoding.UTF8.GetBytes(source);

        IntPtr resultPtr;
        int rc;

        fixed (byte* sourcePtr = sourceBytes)
        {
            rc = NativeMethods.typst_compile_with_path(
                _handle!.DangerousGetHandle(),
                sourcePtr,
                sourceBytes.Length,
                virtualPath,
                out resultPtr);
        }

        if (rc == NativeMethods.TYPST_ERR_INVALID_ARGUMENT && virtualPath is not null)
            throw new ArgumentException(
                $"'{virtualPath}' is not a usable virtual path. It must name a file " +
                "inside the root, not the root itself.",
                nameof(virtualPath));

        ThrowOnError(rc, "Compilation failed at the FFI layer");

        if (resultPtr == IntPtr.Zero)
            throw new TypstException(
                "Native compilation returned a null result.",
                NativeMethods.TYPST_ERR_INTERNAL);

        return new TypstCompileResult(new SafeResultHandle(resultPtr));
    }

    /// <summary>
    /// Compiles a Typst source file and returns the result.
    /// </summary>
    /// <param name="filePath">Path to a <c>.typ</c> file.</param>
    /// <returns>A <see cref="TypstCompileResult"/>.</returns>
    /// <remarks>
    /// <para>
    /// When <see cref="SetRoot"/> has been called and the file lies inside
    /// that root, the root is left alone and the file is compiled at its real
    /// position underneath it. Relative paths in the file then resolve the way
    /// the <c>typst</c> command line tool resolves them, including <c>../</c>
    /// into a sibling directory.
    /// </para>
    /// <para>
    /// When no root has been set, the file's own directory becomes the root.
    /// That keeps single-directory projects working without ceremony, but it
    /// confines the compilation to that directory: a <c>../</c> in the file
    /// escapes the root and fails. Call <see cref="SetRoot"/> with a directory
    /// wide enough to span everything the document needs.
    /// </para>
    /// </remarks>
    /// <exception cref="ArgumentNullException">
    /// Thrown if <paramref name="filePath"/> is <see langword="null"/>.
    /// </exception>
    /// <exception cref="FileNotFoundException">
    /// Thrown if <paramref name="filePath"/> does not exist.
    /// </exception>
    /// <exception cref="ArgumentException">
    /// Thrown if a root was set via <see cref="SetRoot"/> and
    /// <paramref name="filePath"/> lies outside it. Typst cannot read a file
    /// outside the root, so there is no meaningful compilation to perform.
    /// </exception>
    public TypstCompileResult CompileFile(string filePath)
    {
        ArgumentNullException.ThrowIfNull(filePath);

        if (!File.Exists(filePath))
            throw new FileNotFoundException(
                $"Typst source file not found: '{filePath}'", filePath);

        string fullPath = Path.GetFullPath(filePath);
        string? virtualPath = null;

        if (_explicitRoot is not null)
        {
            // Honour the caller's root and locate the entry underneath it.
            virtualPath = TryGetVirtualPath(_explicitRoot, fullPath)
                ?? throw new ArgumentException(
                    $"'{fullPath}' is outside the configured root '{_explicitRoot}'. " +
                    "Typst cannot read files outside the root; widen the root or " +
                    "compile a file inside it.",
                    nameof(filePath));
        }
        else
        {
            // No caller root: confine the compilation to the file's directory.
            string? directory = Path.GetDirectoryName(fullPath);
            if (directory is not null)
            {
                SetRootCore(directory);
                virtualPath = Path.GetFileName(fullPath);
            }
        }

        string source = File.ReadAllText(fullPath, Encoding.UTF8);
        return CompileCore(source, virtualPath);
    }

    /// <summary>
    /// Returns <paramref name="fullPath"/> expressed relative to
    /// <paramref name="root"/> using forward slashes, or <see langword="null"/>
    /// if it does not lie inside that root.
    /// </summary>
    private static string? TryGetVirtualPath(string root, string fullPath)
    {
        // Compare with a trailing separator so that "C:\rootx\a.typ" is not
        // mistaken for a file inside "C:\root".
        string rootWithSeparator = root.EndsWith(Path.DirectorySeparatorChar)
            ? root
            : root + Path.DirectorySeparatorChar;

        if (!fullPath.StartsWith(rootWithSeparator, PathComparison))
            return null;

        string relative = Path.GetRelativePath(root, fullPath);

        // A rooted result means the two paths share no common base at all,
        // which happens across drives and across UNC shares.
        if (Path.IsPathRooted(relative))
            return null;

        return relative.Replace(Path.DirectorySeparatorChar, '/')
                       .Replace(Path.AltDirectorySeparatorChar, '/');
    }

    /// <summary>
    /// How to compare filesystem paths on the current platform.
    /// </summary>
    private static StringComparison PathComparison =>
        OperatingSystem.IsWindows() || OperatingSystem.IsMacOS()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal;

    // -----------------------------------------------------------------------
    // IDisposable
    // -----------------------------------------------------------------------

    /// <inheritdoc />
    public void Dispose()
    {
        if (!_disposed)
        {
            _handle?.Dispose();
            _handle = null;
            _disposed = true;
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
    }

    private static void ThrowOnError(int rc, string context)
    {
        if (rc != NativeMethods.TYPST_OK)
            throw new TypstException($"{context} (native error code {rc}).", rc);
    }
}
