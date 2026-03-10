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
/// <see cref="Compile"/> concurrently on the same instance.
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
    /// Sets the root directory used to resolve <c>#import</c> and
    /// <c>#include</c> paths in Typst source.
    /// </summary>
    /// <param name="rootPath">Absolute path to the root directory.</param>
    /// <exception cref="ArgumentNullException">
    /// Thrown if <paramref name="rootPath"/> is <see langword="null"/>.
    /// </exception>
    /// <exception cref="TypstException">
    /// Thrown if the native call fails.
    /// </exception>
    public void SetRoot(string rootPath)
    {
        ArgumentNullException.ThrowIfNull(rootPath);
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
    public unsafe TypstCompileResult Compile(string source)
    {
        ArgumentNullException.ThrowIfNull(source);
        ThrowIfDisposed();

        byte[] sourceBytes = Encoding.UTF8.GetBytes(source);

        IntPtr resultPtr;
        int rc;

        fixed (byte* sourcePtr = sourceBytes)
        {
            rc = NativeMethods.typst_compile(
                _handle!.DangerousGetHandle(),
                sourcePtr,
                sourceBytes.Length,
                out resultPtr);
        }

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
    /// <exception cref="FileNotFoundException">
    /// Thrown if <paramref name="filePath"/> does not exist.
    /// </exception>
    public TypstCompileResult CompileFile(string filePath)
    {
        ArgumentNullException.ThrowIfNull(filePath);

        if (!File.Exists(filePath))
            throw new FileNotFoundException(
                $"Typst source file not found: '{filePath}'", filePath);

        // Set the root to the file's directory so relative imports work.
        string? directory = Path.GetDirectoryName(Path.GetFullPath(filePath));
        if (directory is not null)
            SetRoot(directory);

        string source = File.ReadAllText(filePath, Encoding.UTF8);
        return Compile(source);
    }

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
