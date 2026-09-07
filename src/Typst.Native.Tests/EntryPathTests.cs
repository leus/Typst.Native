using Xunit;

namespace Typst.Native.Tests;

/// <summary>
/// Tests for how the entry source is located in Typst's virtual file system,
/// which is what decides whether a relative path inside it resolves.
/// </summary>
/// <remarks>
/// Regression coverage for the case where an entry file in a subdirectory
/// reaches a sibling directory through <c>../</c>. Before the entry source was
/// given a real location, it was always treated as sitting at the root, so any
/// leading <c>../</c> escaped the root no matter how the root was configured.
/// </remarks>
public class EntryPathTests : IDisposable
{
    private readonly string _root;

    public EntryPathTests()
    {
        _root = Path.Combine(Path.GetTempPath(), Path.GetRandomFileName());
        Directory.CreateDirectory(Path.Combine(_root, "sub"));
        Directory.CreateDirectory(Path.Combine(_root, "other"));
    }

    public void Dispose()
    {
        if (Directory.Exists(_root))
            Directory.Delete(_root, recursive: true);
        GC.SuppressFinalize(this);
    }

    /// <summary>Writes a file under the temporary root.</summary>
    private string Write(string relativePath, string contents)
    {
        string full = Path.Combine(_root, relativePath);
        Directory.CreateDirectory(Path.GetDirectoryName(full)!);
        File.WriteAllText(full, contents);
        return full;
    }

    /// <summary>Renders a result's diagnostics for assertion messages.</summary>
    private static string Explain(TypstCompileResult result) =>
        string.Join("; ", result.Diagnostics.Select(d => d.ToString()));

    // -----------------------------------------------------------------------
    // The reported bug
    // -----------------------------------------------------------------------

    [Fact]
    public void CompileFile_WithWiderRoot_ResolvesParentRelativeImport()
    {
        Write("other/dep.typ", "#let greet = \"hello from dep\"\n");
        string entry = Write("sub/main.typ", "#import \"../other/dep.typ\": greet\n#greet\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        using var result = compiler.CompileFile(entry);

        Assert.True(result.IsSuccess, Explain(result));
    }

    [Fact]
    public void CompileFile_WithWiderRoot_ResolvesParentRelativeRead()
    {
        Write("other/data.txt", "payload");
        string entry = Write("sub/main.typ", "#read(\"../other/data.txt\")\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        using var result = compiler.CompileFile(entry);

        Assert.True(result.IsSuccess, Explain(result));
    }

    [Fact]
    public void CompileFile_WithoutRoot_ParentRelativeImportStillEscapes()
    {
        // The entry's own directory becomes the root, so ../ leaves it. This
        // is Typst's confinement working as intended, not the bug.
        Write("other/dep.typ", "#let greet = \"hello from dep\"\n");
        string entry = Write("sub/main.typ", "#import \"../other/dep.typ\": greet\n#greet\n");

        using var compiler = new TypstCompiler();

        using var result = compiler.CompileFile(entry);

        Assert.False(result.IsSuccess);
    }

    // -----------------------------------------------------------------------
    // Behaviour that must not regress
    // -----------------------------------------------------------------------

    [Fact]
    public void CompileFile_WithoutRoot_ResolvesSiblingImport()
    {
        Write("sub/sib.typ", "#let sib = \"hello from sibling\"\n");
        string entry = Write("sub/main.typ", "#import \"sib.typ\": sib\n#sib\n");

        using var compiler = new TypstCompiler();

        using var result = compiler.CompileFile(entry);

        Assert.True(result.IsSuccess, Explain(result));
    }

    [Fact]
    public void CompileFile_WithWiderRoot_ResolvesSiblingRelativeToEntry()
    {
        // A decoy at the root: resolution must start from the entry's own
        // directory, not from the root. Widening the root must not change
        // which sibling file a bare import picks up.
        Write("sib.typ", "#let sib = \"WRONG\"\n");
        Write("sub/sib.typ", "#let sib = \"hello from sibling\"\n");
        string entry = Write("sub/main.typ", "#import \"sib.typ\": sib\n#sib\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        using var result = compiler.CompileFile(entry);

        Assert.True(result.IsSuccess, Explain(result));
    }

    [Fact]
    public void CompileFile_DoesNotLeakItsImplicitRootToLaterCalls()
    {
        // The first call sets an implicit root of root/sub. If that leaked as
        // though the caller had chosen it, the second call would be treated as
        // compiling outside the root and would throw.
        Write("sub/sib.typ", "#let sib = \"a\"\n");
        string first = Write("sub/main.typ", "#import \"sib.typ\": sib\n#sib\n");
        Write("other/sib.typ", "#let sib = \"b\"\n");
        string second = Write("other/main.typ", "#import \"sib.typ\": sib\n#sib\n");

        using var compiler = new TypstCompiler();

        using (var firstResult = compiler.CompileFile(first))
            Assert.True(firstResult.IsSuccess, Explain(firstResult));

        using var secondResult = compiler.CompileFile(second);

        Assert.True(secondResult.IsSuccess, Explain(secondResult));
    }

    // -----------------------------------------------------------------------
    // Root and entry disagreement
    // -----------------------------------------------------------------------

    [Fact]
    public void CompileFile_EntryOutsideRoot_Throws()
    {
        string outsideRoot = Path.Combine(Path.GetTempPath(), Path.GetRandomFileName());
        Directory.CreateDirectory(outsideRoot);
        try
        {
            string entry = Path.Combine(outsideRoot, "main.typ");
            File.WriteAllText(entry, "Hello\n");

            using var compiler = new TypstCompiler();
            compiler.SetRoot(_root);

            Assert.Throws<ArgumentException>(() => compiler.CompileFile(entry));
        }
        finally
        {
            Directory.Delete(outsideRoot, recursive: true);
        }
    }

    [Fact]
    public void CompileFile_RootIsPrefixOfSiblingDirectory_Throws()
    {
        // "<root>x" must not count as inside "<root>", despite the shared
        // string prefix.
        string sibling = _root + "x";
        Directory.CreateDirectory(sibling);
        try
        {
            string entry = Path.Combine(sibling, "main.typ");
            File.WriteAllText(entry, "Hello\n");

            using var compiler = new TypstCompiler();
            compiler.SetRoot(_root);

            Assert.Throws<ArgumentException>(() => compiler.CompileFile(entry));
        }
        finally
        {
            Directory.Delete(sibling, recursive: true);
        }
    }

    [Fact]
    public void CompileFile_EntryAtRoot_Compiles()
    {
        Write("other/dep.typ", "#let greet = \"hello from dep\"\n");
        string entry = Write("main.typ", "#import \"other/dep.typ\": greet\n#greet\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        using var result = compiler.CompileFile(entry);

        Assert.True(result.IsSuccess, Explain(result));
    }

    // -----------------------------------------------------------------------
    // Compile(source, virtualPath)
    // -----------------------------------------------------------------------

    [Fact]
    public void Compile_WithVirtualPath_ResolvesParentRelativeImport()
    {
        Write("other/dep.typ", "#let greet = \"hello from dep\"\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        using var result = compiler.Compile(
            "#import \"../other/dep.typ\": greet\n#greet\n", "sub/main.typ");

        Assert.True(result.IsSuccess, Explain(result));
    }

    [Fact]
    public void Compile_WithVirtualPath_NeedNotExistOnDisk()
    {
        Write("other/dep.typ", "#let greet = \"hello from dep\"\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        // Nothing was ever written to sub/nowhere/main.typ.
        using var result = compiler.Compile(
            "#import \"../../other/dep.typ\": greet\n#greet\n", "sub/nowhere/main.typ");

        Assert.True(result.IsSuccess, Explain(result));
    }

    [Theory]
    [InlineData("sub\\main.typ")]
    [InlineData("/sub/main.typ")]
    [InlineData("./sub/main.typ")]
    public void Compile_WithVirtualPath_AcceptsEquivalentSpellings(string virtualPath)
    {
        Write("other/dep.typ", "#let greet = \"hello from dep\"\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        using var result = compiler.Compile(
            "#import \"../other/dep.typ\": greet\n#greet\n", virtualPath);

        Assert.True(result.IsSuccess, Explain(result));
    }

    [Fact]
    public void Compile_WithoutVirtualPath_IsUnchanged()
    {
        // The single-argument overload keeps treating the entry as though it
        // sat at the root, so ../ still escapes.
        Write("other/dep.typ", "#let greet = \"hello from dep\"\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        using var result = compiler.Compile("#import \"../other/dep.typ\": greet\n#greet\n");

        Assert.False(result.IsSuccess);
    }

    [Fact]
    public void Compile_NullVirtualPath_ThrowsArgumentNull()
    {
        using var compiler = new TypstCompiler();

        Assert.Throws<ArgumentNullException>(
            () => compiler.Compile("Hello", null!));
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("/")]
    [InlineData(".")]
    [InlineData("..")]
    [InlineData("../escapes.typ")]
    public void Compile_UnusableVirtualPath_ThrowsArgumentException(string virtualPath)
    {
        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        Assert.Throws<ArgumentException>(
            () => compiler.Compile("Hello", virtualPath));
    }

    [Fact]
    public void Compile_WithVirtualPath_ShadowsFileOnDisk()
    {
        // A perfectly importable file sits at the entry's path on disk. If the
        // entry did not shadow it, importing that path would simply succeed.
        // Because the entry occupies it, the import is a self-import instead,
        // which Typst reports as a cycle.
        Write("sub/main.typ", "#let marker = \"from disk\"\n");

        using var compiler = new TypstCompiler();
        compiler.SetRoot(_root);

        using var result = compiler.Compile(
            "#import \"main.typ\": marker\n#marker\n", "sub/main.typ");

        Assert.False(result.IsSuccess);
        Assert.Contains("cyclic", Explain(result), StringComparison.OrdinalIgnoreCase);
    }
}
