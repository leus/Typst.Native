namespace Typst.Native;

/// <summary>
/// Represents a single diagnostic (error or warning) produced during Typst
/// compilation. Diagnostics include a human-readable message and optional
/// source location information.
/// </summary>
public sealed class TypstDiagnostic
{
    /// <summary>
    /// Gets the severity level of this diagnostic.
    /// </summary>
    public DiagnosticSeverity Severity { get; }

    /// <summary>
    /// Gets the human-readable diagnostic message.
    /// </summary>
    public string Message { get; }

    /// <summary>
    /// Gets the 1-based line number in the source where the diagnostic
    /// originated, or <c>-1</c> if the location is unknown.
    /// </summary>
    public long Line { get; }

    /// <summary>
    /// Gets the 1-based column number in the source where the diagnostic
    /// originated, or <c>-1</c> if the location is unknown.
    /// </summary>
    /// <remarks>
    /// Columns count Unicode characters (code points), so a character outside
    /// the Basic Multilingual Plane counts once even though it occupies two
    /// UTF-16 <see cref="char"/> values in a .NET string.
    /// </remarks>
    public long Column { get; }

    /// <param name="severity">The severity.</param>
    /// <param name="message">The message.</param>
    /// <param name="line">The 1-based line, or <c>-1</c> if unknown.</param>
    /// <param name="column">The 1-based column, or <c>-1</c> if unknown.</param>
    internal TypstDiagnostic(DiagnosticSeverity severity, string message, long line, long column)
    {
        Severity = severity;
        Message = message;
        Line = line;
        Column = column;
    }

    /// <inheritdoc />
    public override string ToString()
    {
        var location = Line >= 0 ? $" ({Line}:{Column})" : "";
        return $"[{Severity}]{location} {Message}";
    }
}
