using System.Text;
using Typst.Native.Interop;

namespace Typst.Native;

/// <summary>
/// Represents the result of a Typst compilation. Provides access to the
/// compiled output (PDF, SVG) or diagnostic information if compilation failed.
/// </summary>
/// <remarks>
/// This object holds a reference to native memory. Dispose it when you are
/// done extracting outputs to free the underlying resources.
/// </remarks>
public sealed class TypstCompileResult : IDisposable
{
    private SafeResultHandle? _handle;
    private bool _disposed;

    internal TypstCompileResult(SafeResultHandle handle)
    {
        _handle = handle ?? throw new ArgumentNullException(nameof(handle));
    }

    /// <summary>
    /// Gets a value indicating whether the compilation succeeded.
    /// </summary>
    public bool IsSuccess
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.typst_result_is_success(_handle!.DangerousGetHandle()) == 1;
        }
    }

    /// <summary>
    /// Gets the number of pages in the compiled document.
    /// Returns <c>0</c> if the compilation failed.
    /// </summary>
    public int PageCount
    {
        get
        {
            ThrowIfDisposed();
            return NativeMethods.typst_result_page_count(_handle!.DangerousGetHandle());
        }
    }

    /// <summary>
    /// Gets the compiled output as a PDF byte array.
    /// </summary>
    /// <returns>A new byte array containing the PDF document.</returns>
    /// <exception cref="TypstException">
    /// Thrown if the compilation failed or the PDF could not be retrieved.
    /// </exception>
    public unsafe byte[] ToPdf()
    {
        ThrowIfDisposed();
        EnsureSuccess();

        byte* data;
        int len;
        int rc = NativeMethods.typst_result_get_pdf(
            _handle!.DangerousGetHandle(), out data, out len);

        ThrowOnError(rc, "Failed to retrieve PDF output");

        // Copy the data out so the caller owns it independently.
        var pdf = new byte[len];
        new ReadOnlySpan<byte>(data, len).CopyTo(pdf);
        return pdf;
    }

    /// <summary>
    /// Gets the compiled output as a PDF and writes it to a stream.
    /// </summary>
    /// <param name="stream">The stream to write the PDF to.</param>
    /// <exception cref="TypstException">
    /// Thrown if the compilation failed or the PDF could not be retrieved.
    /// </exception>
    public unsafe void WritePdfTo(Stream stream)
    {
        ArgumentNullException.ThrowIfNull(stream);
        ThrowIfDisposed();
        EnsureSuccess();

        byte* data;
        int len;
        int rc = NativeMethods.typst_result_get_pdf(
            _handle!.DangerousGetHandle(), out data, out len);

        ThrowOnError(rc, "Failed to retrieve PDF output");

        stream.Write(new ReadOnlySpan<byte>(data, len));
    }

    /// <summary>
    /// Gets the SVG output for a specific page.
    /// </summary>
    /// <param name="pageIndex">The 0-based page index.</param>
    /// <returns>The SVG markup as a string.</returns>
    /// <exception cref="ArgumentOutOfRangeException">
    /// Thrown if <paramref name="pageIndex"/> is out of range.
    /// </exception>
    public unsafe string GetSvgPage(int pageIndex)
    {
        ThrowIfDisposed();
        EnsureSuccess();

        if (pageIndex < 0 || pageIndex >= PageCount)
            throw new ArgumentOutOfRangeException(nameof(pageIndex));

        byte* data;
        int len;
        int rc = NativeMethods.typst_result_get_svg_page(
            _handle!.DangerousGetHandle(), pageIndex, out data, out len);

        ThrowOnError(rc, $"Failed to retrieve SVG for page {pageIndex}");

        return Encoding.UTF8.GetString(data, len);
    }

    /// <summary>
    /// Gets all diagnostics (errors and warnings) from the compilation.
    /// Returns an empty list for successful compilations.
    /// </summary>
    public unsafe IReadOnlyList<TypstDiagnostic> GetDiagnostics()
    {
        ThrowIfDisposed();

        int count = NativeMethods.typst_result_diagnostic_count(
            _handle!.DangerousGetHandle());

        if (count == 0)
            return Array.Empty<TypstDiagnostic>();

        var diagnostics = new TypstDiagnostic[count];
        for (int i = 0; i < count; i++)
        {
            int rc = NativeMethods.typst_result_get_diagnostic(
                _handle!.DangerousGetHandle(),
                i,
                out int severity,
                out byte* message,
                out int messageLen,
                out long line,
                out long column);

            if (rc != NativeMethods.TYPST_OK)
            {
                diagnostics[i] = new TypstDiagnostic(
                    DiagnosticSeverity.Error,
                    $"Failed to read diagnostic {i} (error code {rc})",
                    -1, -1);
                continue;
            }

            string msg = messageLen > 0
                ? Encoding.UTF8.GetString(message, messageLen)
                : string.Empty;

            diagnostics[i] = new TypstDiagnostic(
                (DiagnosticSeverity)severity, msg, line, column);
        }

        return diagnostics;
    }

    /// <summary>
    /// Convenience property that returns <see cref="GetDiagnostics"/>.
    /// </summary>
    public IReadOnlyList<TypstDiagnostic> Diagnostics => GetDiagnostics();

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

    private void EnsureSuccess()
    {
        if (!IsSuccess)
            throw new TypstException(
                "Compilation failed. Check Diagnostics for details.",
                NativeMethods.TYPST_ERR_COMPILE_FAILED);
    }

    private static void ThrowOnError(int rc, string context)
    {
        if (rc != NativeMethods.TYPST_OK)
            throw new TypstException($"{context} (native error code {rc}).", rc);
    }
}
