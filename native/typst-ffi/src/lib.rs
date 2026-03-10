//! # typst-ffi
//!
//! A C-compatible FFI layer over the Typst compiler, designed to be called from
//! .NET via P/Invoke. This crate produces a `cdylib` (shared library) that
//! exposes an opaque-handle-based C API.
//!
//! ## Design Principles
//!
//! - **Handle-based API**: All Typst objects are represented as opaque pointers.
//!   The caller (C#) never sees Rust types directly.
//! - **Explicit memory management**: Every `*_new` / `*_create` has a matching
//!   `*_free`. Buffers returned by the library are freed via `typst_buffer_free`.
//! - **Thread safety**: Each `TypstCompiler` handle is `Send` but not `Sync` —
//!   use one compiler per thread or synchronize access on the C# side.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::slice;

// ---------------------------------------------------------------------------
// Opaque handle types
// ---------------------------------------------------------------------------

/// Opaque compiler handle.
pub struct TypstCompiler {
    root: Option<PathBuf>,
    font_paths: Vec<PathBuf>,
}

/// Opaque compilation result handle.
pub struct TypstCompileResult {
    kind: CompileResultKind,
}

#[allow(dead_code)]
enum CompileResultKind {
    Success {
        /// Raw PDF bytes produced by `typst-pdf`.
        pdf: Vec<u8>,
        /// SVG strings per page produced by `typst-svg`.
        svg_pages: Vec<String>,
        /// Total number of pages in the compiled document.
        page_count: i32,
    },
    Failure {
        diagnostics: Vec<TypstDiagnosticEntry>,
    },
}

struct TypstDiagnosticEntry {
    severity: i32, // 0 = error, 1 = warning
    message: String,
    line: i64,
    column: i64,
}

/// A buffer returned to the caller that must be freed with `typst_buffer_free`.
#[allow(dead_code)]
pub struct TypstBuffer {
    data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

const TYPST_OK: i32 = 0;
const TYPST_ERR_NULL_POINTER: i32 = -1;
const TYPST_ERR_INVALID_UTF8: i32 = -2;
const TYPST_ERR_COMPILE_FAILED: i32 = -3;
const TYPST_ERR_PAGE_OUT_OF_RANGE: i32 = -4;
#[allow(dead_code)]
const TYPST_ERR_INTERNAL: i32 = -99;

// ===========================================================================
// Compiler lifecycle
// ===========================================================================

/// Create a new `TypstCompiler` instance.
///
/// Returns an opaque handle that must be freed with `typst_compiler_free`.
/// Returns `NULL` on allocation failure (extremely unlikely).
#[no_mangle]
pub extern "C" fn typst_compiler_new() -> *mut TypstCompiler {
    let compiler = Box::new(TypstCompiler {
        root: None,
        font_paths: Vec::new(),
    });
    Box::into_raw(compiler)
}

/// Free a `TypstCompiler` instance.
///
/// # Safety
/// `compiler` must be a valid pointer returned by `typst_compiler_new`,
/// or `NULL` (in which case this is a no-op).
#[no_mangle]
pub unsafe extern "C" fn typst_compiler_free(compiler: *mut TypstCompiler) {
    if !compiler.is_null() {
        drop(Box::from_raw(compiler));
    }
}

/// Set the root directory for file resolution.
///
/// # Safety
/// `compiler` must be a valid handle. `path` must be a valid null-terminated
/// UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn typst_compiler_set_root(
    compiler: *mut TypstCompiler,
    path: *const c_char,
) -> i32 {
    if compiler.is_null() || path.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    let compiler = &mut *compiler;
    match CStr::from_ptr(path).to_str() {
        Ok(s) => {
            compiler.root = Some(PathBuf::from(s));
            TYPST_OK
        }
        Err(_) => TYPST_ERR_INVALID_UTF8,
    }
}

/// Add a font search path.
///
/// # Safety
/// `compiler` must be a valid handle. `path` must be a valid null-terminated
/// UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn typst_compiler_add_font_path(
    compiler: *mut TypstCompiler,
    path: *const c_char,
) -> i32 {
    if compiler.is_null() || path.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    let compiler = &mut *compiler;
    match CStr::from_ptr(path).to_str() {
        Ok(s) => {
            compiler.font_paths.push(PathBuf::from(s));
            TYPST_OK
        }
        Err(_) => TYPST_ERR_INVALID_UTF8,
    }
}

// ===========================================================================
// Compilation
// ===========================================================================

/// Compile a Typst source string.
///
/// `source` is a pointer to a UTF-8 encoded string of `source_len` bytes
/// (not null-terminated). On success, `*result` receives an opaque handle
/// that must be freed with `typst_result_free`.
///
/// Returns `TYPST_OK` (0) on success (even if compilation produced errors —
/// check with `typst_result_is_success`). Returns a negative error code on
/// API misuse (null pointers, invalid UTF-8).
///
/// # Safety
/// All pointer arguments must be valid. `source` must point to `source_len`
/// valid bytes.
#[no_mangle]
pub unsafe extern "C" fn typst_compile(
    compiler: *mut TypstCompiler,
    source: *const u8,
    source_len: i32,
    result: *mut *mut TypstCompileResult,
) -> i32 {
    if compiler.is_null() || source.is_null() || result.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }

    let _compiler = &*compiler;
    let source_bytes = slice::from_raw_parts(source, source_len as usize);
    let source_str = match std::str::from_utf8(source_bytes) {
        Ok(s) => s,
        Err(_) => return TYPST_ERR_INVALID_UTF8,
    };

    // -----------------------------------------------------------------------
    // Core compilation logic.
    //
    // This is the integration point with the `typst` crate. The implementation
    // follows this pattern:
    //
    //   1. Build a `World` implementation configured with the compiler's root
    //      and font paths.
    //   2. Call `typst::compile(&world)` to produce a `Document`.
    //   3. Export the document to PDF via `typst_pdf::pdf` and to SVG via
    //      `typst_svg::svg`.
    //   4. Wrap the outputs (or diagnostics) into a `TypstCompileResult`.
    //
    // The `SimpleWorld` struct in the `world` module (see below) provides a
    // minimal `typst::World` implementation suitable for in-memory compilation.
    // -----------------------------------------------------------------------

    let compile_result = compile_inner(_compiler, source_str);
    let boxed = Box::new(compile_result);
    *result = Box::into_raw(boxed);

    TYPST_OK
}

/// Internal compilation logic, separated for readability and to keep
/// unsafe code minimal.
fn compile_inner(compiler: &TypstCompiler, source: &str) -> TypstCompileResult {
    // TODO: Replace this stub with actual Typst compilation.
    //
    // Implementation steps:
    //   1. Create a `world::SimpleWorld` with:
    //      - `compiler.root` as the file system root
    //      - `compiler.font_paths` for font discovery
    //      - `source` as the main input
    //   2. Call `typst::compile(&world)`:
    //      - On `Ok(document)`:
    //        a. `typst_pdf::pdf(&document, ...)` → PDF bytes
    //        b. For each page: `typst_svg::svg(&document.pages[i])` → SVG string
    //        c. Return `CompileResultKind::Success`
    //      - On `Err(diagnostics)`:
    //        a. Map each `SourceDiagnostic` to `TypstDiagnosticEntry`
    //        b. Return `CompileResultKind::Failure`
    //
    // For now, produce a stub PDF so the C# integration can be tested
    // end-to-end without the full Typst dependency wired up.

    let _ = compiler;

    // Minimal valid-ish PDF for scaffolding purposes.
    let stub_pdf = format!(
        "%PDF-1.4\n% stub — source length: {} bytes\n%%EOF\n",
        source.len()
    );

    TypstCompileResult {
        kind: CompileResultKind::Success {
            pdf: stub_pdf.into_bytes(),
            svg_pages: vec![format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\"><text>{}</text></svg>",
                "stub"
            )],
            page_count: 1,
        },
    }
}

// ===========================================================================
// Result inspection
// ===========================================================================

/// Returns `1` if the compilation succeeded, `0` if it failed.
///
/// # Safety
/// `result` must be a valid handle returned by `typst_compile`.
#[no_mangle]
pub unsafe extern "C" fn typst_result_is_success(result: *const TypstCompileResult) -> i32 {
    if result.is_null() {
        return 0;
    }
    match (*result).kind {
        CompileResultKind::Success { .. } => 1,
        CompileResultKind::Failure { .. } => 0,
    }
}

/// Get the number of pages in a successful compilation result.
///
/// Returns the page count, or `0` if the result is a failure.
///
/// # Safety
/// `result` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn typst_result_page_count(result: *const TypstCompileResult) -> i32 {
    if result.is_null() {
        return 0;
    }
    match &(*result).kind {
        CompileResultKind::Success { page_count, .. } => *page_count,
        CompileResultKind::Failure { .. } => 0,
    }
}

/// Get the compiled PDF output.
///
/// On success, `*data` and `*len` are set to the PDF buffer. The buffer is
/// owned by the result and remains valid until `typst_result_free` is called.
///
/// Returns `TYPST_OK` on success, `TYPST_ERR_COMPILE_FAILED` if the result
/// is a failure.
///
/// # Safety
/// All pointers must be valid. The returned `*data` pointer must not be used
/// after `typst_result_free`.
#[no_mangle]
pub unsafe extern "C" fn typst_result_get_pdf(
    result: *const TypstCompileResult,
    data: *mut *const u8,
    len: *mut i32,
) -> i32 {
    if result.is_null() || data.is_null() || len.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    match &(*result).kind {
        CompileResultKind::Success { pdf, .. } => {
            *data = pdf.as_ptr();
            *len = pdf.len() as i32;
            TYPST_OK
        }
        CompileResultKind::Failure { .. } => TYPST_ERR_COMPILE_FAILED,
    }
}

/// Get the SVG output for a specific page (0-indexed).
///
/// On success, `*data` and `*len` are set to the UTF-8 SVG string. The buffer
/// is owned by the result.
///
/// # Safety
/// All pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn typst_result_get_svg_page(
    result: *const TypstCompileResult,
    page: i32,
    data: *mut *const u8,
    len: *mut i32,
) -> i32 {
    if result.is_null() || data.is_null() || len.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    match &(*result).kind {
        CompileResultKind::Success { svg_pages, .. } => {
            if page < 0 || page as usize >= svg_pages.len() {
                return TYPST_ERR_PAGE_OUT_OF_RANGE;
            }
            let svg = &svg_pages[page as usize];
            *data = svg.as_ptr();
            *len = svg.len() as i32;
            TYPST_OK
        }
        CompileResultKind::Failure { .. } => TYPST_ERR_COMPILE_FAILED,
    }
}

// ===========================================================================
// Diagnostics
// ===========================================================================

/// Get the number of diagnostics in a failed compilation result.
///
/// Returns `0` for successful results or null pointers.
///
/// # Safety
/// `result` must be a valid handle or null.
#[no_mangle]
pub unsafe extern "C" fn typst_result_diagnostic_count(
    result: *const TypstCompileResult,
) -> i32 {
    if result.is_null() {
        return 0;
    }
    match &(*result).kind {
        CompileResultKind::Success { .. } => 0,
        CompileResultKind::Failure { diagnostics } => diagnostics.len() as i32,
    }
}

/// Get a diagnostic entry by index.
///
/// Sets `*severity` (0 = error, 1 = warning), `*message` (UTF-8, owned by
/// the result), `*message_len`, `*line`, and `*column`.
///
/// # Safety
/// All pointers must be valid. `index` must be in range.
#[no_mangle]
pub unsafe extern "C" fn typst_result_get_diagnostic(
    result: *const TypstCompileResult,
    index: i32,
    severity: *mut i32,
    message: *mut *const u8,
    message_len: *mut i32,
    line: *mut i64,
    column: *mut i64,
) -> i32 {
    if result.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    match &(*result).kind {
        CompileResultKind::Failure { diagnostics } => {
            if index < 0 || index as usize >= diagnostics.len() {
                return TYPST_ERR_PAGE_OUT_OF_RANGE;
            }
            let diag = &diagnostics[index as usize];
            if !severity.is_null() {
                *severity = diag.severity;
            }
            if !message.is_null() && !message_len.is_null() {
                *message = diag.message.as_ptr();
                *message_len = diag.message.len() as i32;
            }
            if !line.is_null() {
                *line = diag.line;
            }
            if !column.is_null() {
                *column = diag.column;
            }
            TYPST_OK
        }
        CompileResultKind::Success { .. } => TYPST_ERR_COMPILE_FAILED,
    }
}

// ===========================================================================
// Memory management
// ===========================================================================

/// Free a compilation result.
///
/// # Safety
/// `result` must be a valid handle or null.
#[no_mangle]
pub unsafe extern "C" fn typst_result_free(result: *mut TypstCompileResult) {
    if !result.is_null() {
        drop(Box::from_raw(result));
    }
}

/// Free a buffer returned by the library.
///
/// # Safety
/// `buffer` must be a valid handle or null.
#[no_mangle]
pub unsafe extern "C" fn typst_buffer_free(buffer: *mut TypstBuffer) {
    if !buffer.is_null() {
        drop(Box::from_raw(buffer));
    }
}

// ===========================================================================
// Version info
// ===========================================================================

/// Returns the version of the typst-ffi library as a static null-terminated
/// C string.
#[no_mangle]
pub extern "C" fn typst_version() -> *const c_char {
    // SAFETY: The byte string is null-terminated and lives for 'static.
    b"0.1.0\0".as_ptr() as *const c_char
}

// ===========================================================================
// World implementation (to be expanded)
// ===========================================================================

mod world {
    //! Minimal `typst::World` implementation for in-memory compilation.
    //!
    //! ## TODO
    //!
    //! Implement the `typst::World` trait here. The struct should hold:
    //!
    //! - The main source text
    //! - A font book loaded from configured font paths + system fonts
    //! - A file resolver rooted at the configured root directory
    //!
    //! Refer to the Typst CLI source (`crates/typst-cli/src/world.rs`) for
    //! a full-featured reference implementation.

    use std::path::PathBuf;

    #[allow(dead_code)]
    pub struct SimpleWorld {
        pub root: PathBuf,
        pub main_source: String,
        pub font_paths: Vec<PathBuf>,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn compiler_lifecycle() {
        unsafe {
            let compiler = typst_compiler_new();
            assert!(!compiler.is_null());
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn compile_stub() {
        unsafe {
            let compiler = typst_compiler_new();
            let source = b"Hello, world!";
            let mut result: *mut TypstCompileResult = ptr::null_mut();

            let rc = typst_compile(
                compiler,
                source.as_ptr(),
                source.len() as i32,
                &mut result,
            );
            assert_eq!(rc, TYPST_OK);
            assert!(!result.is_null());
            assert_eq!(typst_result_is_success(result), 1);
            assert_eq!(typst_result_page_count(result), 1);

            let mut data: *const u8 = ptr::null();
            let mut len: i32 = 0;
            let rc = typst_result_get_pdf(result, &mut data, &mut len);
            assert_eq!(rc, TYPST_OK);
            assert!(len > 0);

            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn null_safety() {
        unsafe {
            // All functions should handle null gracefully.
            typst_compiler_free(ptr::null_mut());
            typst_result_free(ptr::null_mut());
            typst_buffer_free(ptr::null_mut());
            assert_eq!(typst_result_is_success(ptr::null()), 0);
            assert_eq!(typst_result_page_count(ptr::null()), 0);
            assert_eq!(typst_result_diagnostic_count(ptr::null()), 0);
        }
    }
}
