namespace Typst.Native;

/// <summary>
/// Represents the severity of a Typst compilation diagnostic.
/// </summary>
public enum DiagnosticSeverity
{
    /// <summary>An error that prevented compilation.</summary>
    Error = 0,

    /// <summary>A warning that did not prevent compilation.</summary>
    Warning = 1,
}
