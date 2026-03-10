using Microsoft.Win32.SafeHandles;

namespace Typst.Native.Interop;

/// <summary>
/// A safe handle wrapping the opaque <c>typst_compiler_t</c> pointer
/// returned by the native library. Ensures the native compiler is freed when
/// the managed wrapper is disposed or finalized.
/// </summary>
internal sealed class SafeCompilerHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    private SafeCompilerHandle() : base(ownsHandle: true) { }

    internal SafeCompilerHandle(IntPtr handle) : base(ownsHandle: true)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.typst_compiler_free(handle);
        return true;
    }
}

/// <summary>
/// A safe handle wrapping the opaque <c>typst_result_t</c> pointer.
/// </summary>
internal sealed class SafeResultHandle : SafeHandleZeroOrMinusOneIsInvalid
{
    private SafeResultHandle() : base(ownsHandle: true) { }

    internal SafeResultHandle(IntPtr handle) : base(ownsHandle: true)
    {
        SetHandle(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.typst_result_free(handle);
        return true;
    }
}
