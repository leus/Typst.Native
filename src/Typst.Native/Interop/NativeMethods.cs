using System.Runtime.InteropServices;

namespace Typst.Native.Interop;

/// <summary>
/// Raw P/Invoke declarations for the <c>typst_ffi</c> native library.
/// These are low-level; consumers should use <see cref="TypstCompiler"/> instead.
/// </summary>
internal static partial class NativeMethods
{
    /// <summary>
    /// The name of the native shared library (without platform-specific extension).
    /// The .NET runtime resolves this to <c>typst_ffi.dll</c>, <c>libtypst_ffi.so</c>,
    /// or <c>libtypst_ffi.dylib</c> depending on the platform.
    /// </summary>
    private const string LibraryName = "typst_ffi";

    // -----------------------------------------------------------------------
    // Error codes (must match native/typst-ffi/src/lib.rs)
    // -----------------------------------------------------------------------

    internal const int TYPST_OK = 0;
    internal const int TYPST_ERR_NULL_POINTER = -1;
    internal const int TYPST_ERR_INVALID_UTF8 = -2;
    internal const int TYPST_ERR_COMPILE_FAILED = -3;
    internal const int TYPST_ERR_PAGE_OUT_OF_RANGE = -4;
    internal const int TYPST_ERR_INTERNAL = -99;

    // -----------------------------------------------------------------------
    // Compiler lifecycle
    // -----------------------------------------------------------------------

    /// <summary>
    /// Create a new compiler instance. Returns an opaque handle.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr typst_compiler_new();

    /// <summary>
    /// Free a compiler instance.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void typst_compiler_free(IntPtr compiler);

    /// <summary>
    /// Set the root directory for resolving file imports.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int typst_compiler_set_root(
        IntPtr compiler,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string path);

    /// <summary>
    /// Add a directory to the font search paths.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int typst_compiler_add_font_path(
        IntPtr compiler,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string path);

    // -----------------------------------------------------------------------
    // Compilation
    // -----------------------------------------------------------------------

    /// <summary>
    /// Compile a Typst source string. <paramref name="source"/> is a UTF-8
    /// byte pointer, not null-terminated.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern unsafe int typst_compile(
        IntPtr compiler,
        byte* source,
        int sourceLen,
        out IntPtr result);

    // -----------------------------------------------------------------------
    // Result inspection
    // -----------------------------------------------------------------------

    /// <summary>
    /// Returns 1 if compilation succeeded, 0 otherwise.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int typst_result_is_success(IntPtr result);

    /// <summary>
    /// Get the page count of a successful result.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int typst_result_page_count(IntPtr result);

    /// <summary>
    /// Get the PDF output. <paramref name="data"/> points into the result's
    /// internal buffer (valid until <see cref="typst_result_free"/>).
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern unsafe int typst_result_get_pdf(
        IntPtr result,
        out byte* data,
        out int len);

    /// <summary>
    /// Get the SVG output for a page (0-indexed).
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern unsafe int typst_result_get_svg_page(
        IntPtr result,
        int page,
        out byte* data,
        out int len);

    // -----------------------------------------------------------------------
    // Diagnostics
    // -----------------------------------------------------------------------

    /// <summary>
    /// Get the number of diagnostics in a failed result.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int typst_result_diagnostic_count(IntPtr result);

    /// <summary>
    /// Get a diagnostic entry by index.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern unsafe int typst_result_get_diagnostic(
        IntPtr result,
        int index,
        out int severity,
        out byte* message,
        out int messageLen,
        out long line,
        out long column);

    // -----------------------------------------------------------------------
    // Memory management
    // -----------------------------------------------------------------------

    /// <summary>
    /// Free a compilation result.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void typst_result_free(IntPtr result);

    /// <summary>
    /// Free a buffer allocated by the library.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void typst_buffer_free(IntPtr buffer);

    // -----------------------------------------------------------------------
    // Version
    // -----------------------------------------------------------------------

    /// <summary>
    /// Get the library version as a null-terminated UTF-8 string.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr typst_version();
}
