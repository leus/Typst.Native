using System.Text;
using Xunit;

namespace Typst.Native.Tests;

/// <summary>
/// Tests for <see cref="TypstCompiler.AddFile(string, byte[])"/> and
/// <see cref="TypstCompiler.ClearFiles"/> — in-memory files referenced from
/// Typst source, e.g. via <c>#image(...)</c> or <c>#import</c>.
/// </summary>
public class VirtualFileTests : IDisposable
{
    /// <summary>A minimal valid 1x1 transparent PNG.</summary>
    private static readonly byte[] TinyPng =
    {
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
        0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    };

    private const string ImageSource = "#image(\"logo.png\")";

    private readonly TypstCompiler _compiler;

    public VirtualFileTests()
    {
        _compiler = new TypstCompiler();
    }

    public void Dispose()
    {
        _compiler.Dispose();
        GC.SuppressFinalize(this);
    }

    [Fact]
    public void AddFile_ThenCompileImage_Succeeds()
    {
        _compiler.AddFile("logo.png", TinyPng);

        using var result = _compiler.Compile(ImageSource);

        Assert.True(result.IsSuccess);
        Assert.True(result.ToPdf().Length > 0);
    }

    [Fact]
    public void Compile_ImageWithoutVirtualFile_FailsWithDiagnostic()
    {
        using var result = _compiler.Compile("#image(\"missing.png\")");

        Assert.False(result.IsSuccess);
        Assert.NotEmpty(result.Diagnostics);
    }

    [Fact]
    public void AddFile_LeadingSlash_ResolvesSamePath()
    {
        _compiler.AddFile("/logo.png", TinyPng);

        using var result = _compiler.Compile(ImageSource);

        Assert.True(result.IsSuccess);
    }

    [Fact]
    public void AddFile_TakesPrecedenceOverDisk()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), Path.GetRandomFileName());
        Directory.CreateDirectory(tempDir);
        try
        {
            // The on-disk file is not a valid image; success proves the
            // virtual file was used instead.
            File.WriteAllBytes(Path.Combine(tempDir, "logo.png"), new byte[] { 1, 2, 3 });
            _compiler.SetRoot(tempDir);
            _compiler.AddFile("logo.png", TinyPng);

            using var result = _compiler.Compile(ImageSource);

            Assert.True(result.IsSuccess);
        }
        finally
        {
            Directory.Delete(tempDir, recursive: true);
        }
    }

    [Fact]
    public void AddFile_Overwrite_LastWriteWins()
    {
        _compiler.AddFile("logo.png", new byte[] { 1, 2, 3 });
        _compiler.AddFile("logo.png", TinyPng);

        using var result = _compiler.Compile(ImageSource);

        Assert.True(result.IsSuccess);
    }

    [Fact]
    public void AddFile_PersistsAcrossCompiles()
    {
        _compiler.AddFile("logo.png", TinyPng);

        using (var first = _compiler.Compile(ImageSource))
            Assert.True(first.IsSuccess);

        using var second = _compiler.Compile(ImageSource);
        Assert.True(second.IsSuccess);
    }

    [Fact]
    public void AddFile_VirtualTypImport_Works()
    {
        _compiler.AddFile("helper.typ", Encoding.UTF8.GetBytes("#let greeting = \"hi\""));

        using var result = _compiler.Compile("#import \"helper.typ\": greeting\n#greeting");

        Assert.True(result.IsSuccess);
    }

    [Fact]
    public void AddFile_NestedPath_Works()
    {
        _compiler.AddFile("assets/logo.png", TinyPng);

        using var result = _compiler.Compile("#image(\"assets/logo.png\")");

        Assert.True(result.IsSuccess);
    }

    [Fact]
    public void AddFile_ImageIn_PngOut_EndToEnd()
    {
        _compiler.AddFile("logo.png", TinyPng);

        using var result = _compiler.Compile(ImageSource);

        Assert.True(result.IsSuccess);
        byte[] png = result.RenderPng(0);
        Assert.Equal(0x89, png[0]);
        Assert.Equal((byte)'P', png[1]);
    }

    [Fact]
    public void ClearFiles_RemovesVirtualFiles()
    {
        _compiler.AddFile("logo.png", TinyPng);
        _compiler.ClearFiles();

        using var result = _compiler.Compile(ImageSource);

        Assert.False(result.IsSuccess);
    }

    [Fact]
    public void AddFile_EmptyData_AcceptedButFailsAtDecode()
    {
        _compiler.AddFile("empty.png", Array.Empty<byte>());

        using var result = _compiler.Compile("#image(\"empty.png\")");

        Assert.False(result.IsSuccess);
        Assert.NotEmpty(result.Diagnostics);
    }

    [Fact]
    public void AddFile_NullPath_ThrowsArgumentNull()
    {
        Assert.Throws<ArgumentNullException>(() => _compiler.AddFile(null!, TinyPng));
    }

    [Fact]
    public void AddFile_NullData_ThrowsArgumentNull()
    {
        Assert.Throws<ArgumentNullException>(
            () => _compiler.AddFile("logo.png", (byte[])null!));
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    public void AddFile_EmptyOrWhitespacePath_ThrowsArgument(string path)
    {
        Assert.Throws<ArgumentException>(() => _compiler.AddFile(path, TinyPng));
    }

    [Fact]
    public void AddFile_DisposedCompiler_ThrowsObjectDisposed()
    {
        var compiler = new TypstCompiler();
        compiler.Dispose();

        Assert.Throws<ObjectDisposedException>(
            () => compiler.AddFile("logo.png", TinyPng));
    }
}
