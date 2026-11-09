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
    }
}
