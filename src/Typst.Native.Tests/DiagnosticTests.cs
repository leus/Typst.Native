using Xunit;

namespace Typst.Native.Tests;

/// <summary>
/// Tests for the diagnostics reported on a <see cref="TypstCompileResult"/>.
/// </summary>
public class DiagnosticTests : IDisposable
{
    private readonly TypstCompiler _compiler;

    public DiagnosticTests()
    {
        _compiler = new TypstCompiler();
    }

    public void Dispose()
    {
        _compiler.Dispose();
        GC.SuppressFinalize(this);
    }

    [Fact]
    public void Compile_CleanSource_HasNoDiagnostics()
    {
        using var result = _compiler.Compile("Hello, *world*!\n");

        Assert.True(result.IsSuccess);
        Assert.Empty(result.Diagnostics);
    }

    [Fact]
    public void Compile_SuccessWithWarning_ReportsWarning()
    {
        using var result = _compiler.Compile(
            "#set text(font: \"No Such Family At All\")\nhello\n");

        Assert.True(result.IsSuccess);

        // Exactly one: the HTML export compiles the document a second time,
        // and its copy of the warning must not be reported again.
        var warning = Assert.Single(result.Diagnostics);
        Assert.Equal(DiagnosticSeverity.Warning, warning.Severity);
        Assert.Contains("unknown font family", warning.Message);

        // The span is the font name literal, 17th character of line 1.
        Assert.Equal(1, warning.Line);
        Assert.Equal(17, warning.Column);
    }

    [Fact]
    public void Compile_Error_ReportsOneBasedPosition()
    {
        using var result = _compiler.Compile("#undefined_thing\n");

        Assert.False(result.IsSuccess);

        // The span is the identifier, which starts right after the '#'.
        var error = Assert.Single(result.Diagnostics);
        Assert.Equal(DiagnosticSeverity.Error, error.Severity);
        Assert.Equal(1, error.Line);
        Assert.Equal(2, error.Column);
        Assert.StartsWith("[Error] (1:2) ", error.ToString());
    }

    [Fact]
    public void Compile_ErrorOnLaterLine_CountsLinesAndCharactersFromOne()
    {
        // Two non-ASCII characters before the error: columns count
        // characters, not UTF-8 bytes.
        using var result = _compiler.Compile("first\n\u00e9\u00e9 #undefined_thing\n");

        Assert.False(result.IsSuccess);

        var error = Assert.Single(result.Diagnostics);
        Assert.Equal(2, error.Line);
        Assert.Equal(5, error.Column);
    }
}
