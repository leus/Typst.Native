using Xunit;

namespace Typst.Native.Tests;

/// <summary>
/// Tests for <see cref="TypstCompiler"/> covering the core compilation workflow.
/// These tests require the native <c>typst_ffi</c> library to be present in the
/// output directory.
/// </summary>
public class CompilerTests : IDisposable
{
    private readonly TypstCompiler _compiler;

    public CompilerTests()
    {
        _compiler = new TypstCompiler();
    }

    public void Dispose()
    {
        _compiler.Dispose();
        GC.SuppressFinalize(this);
    }

    [Fact]
    public void NativeVersion_ReturnsNonEmptyString()
    {
        string version = TypstCompiler.NativeVersion;

        Assert.False(string.IsNullOrWhiteSpace(version));
    }

    [Fact]
    public void Compile_SimpleMarkup_Succeeds()
    {
        using var result = _compiler.Compile("Hello, *world*!");

        Assert.True(result.IsSuccess);
        Assert.True(result.PageCount > 0);
    }

    [Fact]
    public void Compile_SimpleMarkup_ProducesPdf()
    {
        using var result = _compiler.Compile("= Title\n\nSome body text.");

        Assert.True(result.IsSuccess);

        byte[] pdf = result.ToPdf();
        Assert.NotNull(pdf);
        Assert.True(pdf.Length > 0);

        // PDF files start with "%PDF"
        Assert.Equal((byte)'%', pdf[0]);
        Assert.Equal((byte)'P', pdf[1]);
        Assert.Equal((byte)'D', pdf[2]);
        Assert.Equal((byte)'F', pdf[3]);
    }

    [Fact]
    public void Compile_SimpleMarkup_ProducesSvg()
    {
        using var result = _compiler.Compile("Hello from SVG!");

        Assert.True(result.IsSuccess);
        Assert.True(result.PageCount >= 1);

        string svg = result.GetSvgPage(0);
        Assert.Contains("<svg", svg);
    }

    [Fact]
    public void RenderPng_SimpleMarkup_ProducesPngBytes()
    {
        using var result = _compiler.Compile("Hello from PNG!");

        Assert.True(result.IsSuccess);

        byte[] png = result.RenderPng(0);
        Assert.True(png.Length > 8);

        // PNG files start with the magic bytes 0x89 "PNG"
        Assert.Equal(0x89, png[0]);
        Assert.Equal((byte)'P', png[1]);
        Assert.Equal((byte)'N', png[2]);
        Assert.Equal((byte)'G', png[3]);
    }

    [Fact]
    public void RenderPng_PageOutOfRange_Throws()
    {
        using var result = _compiler.Compile("test");
        Assert.True(result.IsSuccess);

        Assert.Throws<ArgumentOutOfRangeException>(() => result.RenderPng(-1));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => result.RenderPng(result.PageCount));
    }

    [Fact]
    public void RenderPng_InvalidScale_Throws()
    {
        using var result = _compiler.Compile("test");
        Assert.True(result.IsSuccess);

        Assert.Throws<ArgumentOutOfRangeException>(
            () => result.RenderPng(0, 0f));
        Assert.Throws<ArgumentOutOfRangeException>(
            () => result.RenderPng(0, float.NaN));
    }

    [Fact]
    public void RenderPng_OnFailedResult_Throws()
    {
        using var result = _compiler.Compile("#image(\"missing.png\")");
        Assert.False(result.IsSuccess);

        Assert.Throws<TypstException>(() => result.RenderPng(0));
    }

    [Fact]
    public void Compile_WritePngToStream()
    {
        using var result = _compiler.Compile("PNG stream test.");
        Assert.True(result.IsSuccess);

        using var ms = new MemoryStream();
        result.WritePngTo(ms, 0);

        Assert.True(ms.Length > 0);
    }

    [Fact]
    public void Compile_WritePdfToStream()
    {
        using var result = _compiler.Compile("Stream test.");
        Assert.True(result.IsSuccess);

        using var ms = new MemoryStream();
        result.WritePdfTo(ms);

        Assert.True(ms.Length > 0);
    }

    [Fact]
    public void Compile_NullSource_ThrowsArgumentNull()
    {
        Assert.Throws<ArgumentNullException>(() => _compiler.Compile(null!));
    }

    [Fact]
    public void CompileFile_MissingFile_ThrowsFileNotFound()
    {
        Assert.Throws<FileNotFoundException>(
            () => _compiler.CompileFile("nonexistent.typ"));
    }

    [Fact]
    public void Dispose_PreventsFurtherUse()
    {
        var compiler = new TypstCompiler();
        compiler.Dispose();

        Assert.Throws<ObjectDisposedException>(
            () => compiler.Compile("test"));
    }

    [Fact]
    public void Result_Dispose_PreventsFurtherUse()
    {
        using var result = _compiler.Compile("test");
        result.Dispose();

        Assert.Throws<ObjectDisposedException>(() => result.ToPdf());
    }

    [Fact]
    public void GetSvgPage_OutOfRange_ThrowsArgumentOutOfRange()
    {
        using var result = _compiler.Compile("test");
        Assert.True(result.IsSuccess);

        Assert.Throws<ArgumentOutOfRangeException>(
            () => result.GetSvgPage(-1));

        Assert.Throws<ArgumentOutOfRangeException>(
            () => result.GetSvgPage(result.PageCount));
    }

    [Fact]
    public void SetRoot_NullPath_ThrowsArgumentNull()
    {
        Assert.Throws<ArgumentNullException>(
            () => _compiler.SetRoot(null!));
    }

    [Fact]
    public void AddFontPath_NullPath_ThrowsArgumentNull()
    {
        Assert.Throws<ArgumentNullException>(
            () => _compiler.AddFontPath(null!));
    }
}
