namespace Typst.Native;

/// <summary>
/// Represents an error thrown by the Typst native library when an operation
/// fails due to an unexpected error code.
/// </summary>
public class TypstException : Exception
{
    /// <summary>
    /// Gets the native error code returned by the FFI layer.
    /// </summary>
    public int NativeErrorCode { get; }

    /// <summary>
    /// Initializes a new <see cref="TypstException"/>.
    /// </summary>
    public TypstException(string message, int nativeErrorCode)
        : base(message)
    {
        NativeErrorCode = nativeErrorCode;
    }

    /// <summary>
    /// Initializes a new <see cref="TypstException"/> with an inner exception.
    /// </summary>
    public TypstException(string message, int nativeErrorCode, Exception innerException)
        : base(message, innerException)
    {
        NativeErrorCode = nativeErrorCode;
    }
}
