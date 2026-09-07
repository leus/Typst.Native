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

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;
use std::slice;

use typst::foundations::Bytes;
use typst_layout::PagedDocument;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};
use typst::{World, WorldExt};

// ---------------------------------------------------------------------------
// Opaque handle types
// ---------------------------------------------------------------------------

/// Opaque compiler handle.
pub struct TypstCompiler {
    root: Option<PathBuf>,
    font_paths: Vec<PathBuf>,
    /// In-memory files keyed by normalized rooted virtual path.
    files: HashMap<VirtualPath, Bytes>,
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
        /// SLA (Scribus) XML produced by `typst-scribus`.
        sla: String,
        /// HTML produced by `typst-html`.
        html: String,
        /// Total number of pages in the compiled document.
        page_count: i32,
        /// The compiled document, kept alive for on-demand PNG rendering.
        document: PagedDocument,
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
const TYPST_ERR_INVALID_ARGUMENT: i32 = -5;
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
        files: HashMap::new(),
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

/// Register an in-memory file that Typst source can reference by path,
/// e.g. `#image("logo.png")` or `#import "helper.typ"`.
///
/// `path` is a virtual path rooted at the compilation root (`"logo.png"` and
/// `"/logo.png"` are equivalent). Backslashes are treated as path separators
/// on every platform. Re-adding the same path overwrites the previous data.
/// Virtual files take precedence over files on disk and persist until the
/// compiler is freed or `typst_compiler_clear_files` is called.
///
/// `data` may be null only if `data_len` is `0` (registers an empty file).
///
/// # Safety
/// `compiler` must be a valid handle. `path` must be a valid null-terminated
/// UTF-8 string. `data` must point to `data_len` valid bytes (or be null when
/// `data_len` is `0`).
#[no_mangle]
pub unsafe extern "C" fn typst_compiler_add_file(
    compiler: *mut TypstCompiler,
    path: *const c_char,
    data: *const u8,
    data_len: i32,
) -> i32 {
    if compiler.is_null() || path.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    if data_len < 0 || (data.is_null() && data_len > 0) {
        return TYPST_ERR_INVALID_ARGUMENT;
    }
    let compiler = &mut *compiler;
    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return TYPST_ERR_INVALID_UTF8,
    };
    // Backslashes are separators only on Windows — normalize so virtual
    // paths behave identically everywhere.
    let normalized = path_str.replace('\\', "/");
    let vpath = match VirtualPath::new(normalized.as_str()) {
        Ok(vpath) => vpath,
        Err(_) => return TYPST_ERR_INVALID_ARGUMENT,
    };
    // Reject paths that normalize to the bare root ("", ".", "/").
    if vpath.is_root() {
        return TYPST_ERR_INVALID_ARGUMENT;
    }
    let bytes = if data_len == 0 {
        Vec::new()
    } else {
        slice::from_raw_parts(data, data_len as usize).to_vec()
    };
    compiler.files.insert(vpath, Bytes::new(bytes));
    TYPST_OK
}

/// Remove all in-memory files previously registered with
/// `typst_compiler_add_file`.
///
/// # Safety
/// `compiler` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn typst_compiler_clear_files(compiler: *mut TypstCompiler) -> i32 {
    if compiler.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    (*compiler).files.clear();
    TYPST_OK
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
/// The entry source is *detached*: it occupies the virtual path `/main.typ`,
/// directly at the compilation root. Relative paths inside it therefore
/// resolve against the root, and a leading `..` always escapes it. Use
/// `typst_compile_with_path` to place the entry elsewhere in the virtual
/// file system.
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
    typst_compile_with_path(compiler, source, source_len, ptr::null(), result)
}

/// Compile a Typst source string, giving the entry source a location in the
/// virtual file system.
///
/// Identical to `typst_compile`, except that `main_vpath` names where the
/// entry source sits relative to the compilation root. This is what makes
/// relative paths in the entry resolve the way the `typst` CLI resolves them:
/// an entry registered at `"sub/main.typ"` can reach `"../other/dep.typ"`,
/// because that resolves to `/other/dep.typ`, still inside the root.
///
/// `main_vpath` is a null-terminated UTF-8 virtual path rooted at the
/// compilation root, using the same rules as `typst_compiler_add_file`:
/// `"sub/main.typ"` and `"/sub/main.typ"` are equivalent, and backslashes are
/// treated as separators on every platform. Passing `NULL` reproduces
/// `typst_compile` exactly, i.e. a detached entry at `/main.typ`.
///
/// The entry source always shadows both an on-disk file and a virtual file
/// registered at the same path, so `main_vpath` need not exist on disk.
///
/// Returns `TYPST_ERR_INVALID_ARGUMENT` if `main_vpath` is not a usable
/// virtual path, for example if it escapes the root or normalizes to the bare
/// root itself.
///
/// # Safety
/// All pointer arguments must be valid. `source` must point to `source_len`
/// valid bytes. `main_vpath` must be a valid null-terminated UTF-8 string, or
/// `NULL`.
#[no_mangle]
pub unsafe extern "C" fn typst_compile_with_path(
    compiler: *mut TypstCompiler,
    source: *const u8,
    source_len: i32,
    main_vpath: *const c_char,
    result: *mut *mut TypstCompileResult,
) -> i32 {
    if compiler.is_null() || source.is_null() || result.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    if source_len < 0 {
        return TYPST_ERR_INVALID_ARGUMENT;
    }

    let _compiler = &*compiler;
    let source_bytes = slice::from_raw_parts(source, source_len as usize);
    let source_str = match std::str::from_utf8(source_bytes) {
        Ok(s) => s,
        Err(_) => return TYPST_ERR_INVALID_UTF8,
    };

    // Resolve the entry's virtual path up front so a bad path is reported as
    // API misuse rather than as a compile diagnostic.
    let main_id = if main_vpath.is_null() {
        None
    } else {
        let vpath_str = match CStr::from_ptr(main_vpath).to_str() {
            Ok(s) => s,
            Err(_) => return TYPST_ERR_INVALID_UTF8,
        };
        match parse_virtual_path(vpath_str) {
            Some(vpath) => Some(RootedPath::new(VirtualRoot::Project, vpath).intern()),
            None => return TYPST_ERR_INVALID_ARGUMENT,
        }
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

    let compile_result = compile_inner(_compiler, source_str, main_id);
    let boxed = Box::new(compile_result);
    *result = Box::into_raw(boxed);

    TYPST_OK
}

/// Parse a caller-supplied virtual path using the same rules as
/// `typst_compiler_add_file`: backslashes are separators on every platform,
/// and a path that normalizes to the bare root is rejected.
///
/// Returns `None` if the path is unusable, including when it escapes the root.
fn parse_virtual_path(path: &str) -> Option<VirtualPath> {
    // Backslashes are separators only on Windows — normalize so virtual
    // paths behave identically everywhere.
    let normalized = path.replace('\\', "/");
    let vpath = VirtualPath::new(normalized.as_str()).ok()?;
    if vpath.is_root() {
        return None;
    }
    Some(vpath)
}

/// Internal compilation logic, separated for readability and to keep
/// unsafe code minimal.
fn compile_inner(
    compiler: &TypstCompiler,
    source: &str,
    main_id: Option<FileId>,
) -> TypstCompileResult {
    use typst::diag::{Severity, Warned};
    use typst_pdf::PdfOptions;
    use typst_scribus::SlaOptions;
    use typst_html::HtmlOptions;

    let world = world::SimpleWorld::new(
        source,
        main_id,
        compiler.root.clone(),
        &compiler.font_paths,
        compiler.files.clone(),
    );

    let Warned { output, warnings: _ } =
        typst::compile::<PagedDocument>(&world);

    match output {
        Ok(document) => {
            // Export to PDF
            let pdf = match typst_pdf::pdf(&document, &PdfOptions::default()) {
                Ok(bytes) => bytes,
                Err(errors) => {
                    let diagnostics = errors
                        .iter()
                        .map(|d| TypstDiagnosticEntry {
                            severity: match d.severity {
                                Severity::Error => 0,
                                Severity::Warning => 1,
                            },
                            message: d.message.to_string(),
                            line: 0,
                            column: 0,
                        })
                        .collect();
                    return TypstCompileResult {
                        kind: CompileResultKind::Failure { diagnostics },
                    };
                }
            };

            // Export SVG per page
            let svg_pages: Vec<String> = document
                .pages()
                .iter()
                .map(|page| typst_svg::svg(page, &typst_svg::SvgOptions::default()))
                .collect();

            // Export to Scribus SLA
            let sla = typst_scribus::sla(&document, &SlaOptions::default());

            // Export to HTML
            let html = match typst::compile::<typst_html::HtmlDocument>(&world) {
                Warned { output: Ok(html_document), .. } => {
                    match typst_html::html(&html_document, &HtmlOptions::default()) {
                        Ok(strs) => strs,
                        Err(errors) => {
                            let diagnostics = errors
                                .iter()
                                .map(|d| TypstDiagnosticEntry {
                                    severity: match d.severity {
                                        Severity::Error => 0,
                                        Severity::Warning => 1,
                                    },
                                    message: d.message.to_string(),
                                    line: 0,
                                    column: 0,
                                })
                                .collect();
                            return TypstCompileResult {
                                kind: CompileResultKind::Failure { diagnostics },
                            };
                        }
                    }
                }
                Warned { output: Err(errors), .. } => {
                    let diagnostics = errors
                        .iter()
                        .map(|d| TypstDiagnosticEntry {
                            severity: match d.severity {
                                Severity::Error => 0,
                                Severity::Warning => 1,
                            },
                            message: d.message.to_string(),
                            line: 0,
                            column: 0,
                        })
                        .collect();
                    return TypstCompileResult {
                        kind: CompileResultKind::Failure { diagnostics },
                    };
                }
            };

            let page_count = document.pages().len() as i32;

            TypstCompileResult {
                kind: CompileResultKind::Success {
                    pdf,
                    svg_pages,
                    sla,
                    html,
                    page_count,
                    document,
                },
            }
        }
        Err(errors) => {
            let diagnostics = errors
                .iter()
                .map(|d| {
                    let (line, column) = if let Some(id) = d.span.id() {
                        if let Ok(src) = world.source(id) {
                            let range = world.range(d.span).unwrap_or(0..0);
                            let line_idx = src.lines().byte_to_line(range.start).unwrap_or(0);
                            let col_idx = src.lines().byte_to_column(range.start).unwrap_or(0);
                            (line_idx as i64, col_idx as i64)
                        } else {
                            (0, 0)
                        }
                    } else {
                        (0, 0)
                    };

                    TypstDiagnosticEntry {
                        severity: match d.severity {
                            Severity::Error => 0,
                            Severity::Warning => 1,
                        },
                        message: d.message.to_string(),
                        line,
                        column,
                    }
                })
                .collect();

            TypstCompileResult {
                kind: CompileResultKind::Failure { diagnostics },
            }
        }
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

/// Get the Scribus SLA (XML) output.
///
/// On success, `*data` and `*len` are set to the UTF-8 SLA string. The buffer
/// is owned by the result and remains valid until `typst_result_free` is called.
///
/// Returns `TYPST_OK` on success, `TYPST_ERR_COMPILE_FAILED` if the result
/// is a failure.
///
/// # Safety
/// All pointers must be valid. The returned `*data` pointer must not be used
/// after `typst_result_free`.
#[no_mangle]
pub unsafe extern "C" fn typst_result_get_sla(
    result: *const TypstCompileResult,
    data: *mut *const u8,
    len: *mut i32,
) -> i32 {
    if result.is_null() || data.is_null() || len.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    match &(*result).kind {
        CompileResultKind::Success { sla, .. } => {
            *data = sla.as_ptr();
            *len = sla.len() as i32;
            TYPST_OK
        }
        CompileResultKind::Failure { .. } => TYPST_ERR_COMPILE_FAILED,
    }
}

/// Get the HTML output.
///
/// On success, `*data` and `*len` are set to the UTF-8 HTML string. The buffer
/// is owned by the result and remains valid until `typst_result_free` is called.
///
/// Returns `TYPST_OK` on success, `TYPST_ERR_COMPILE_FAILED` if the result
/// is a failure.
///
/// # Safety
/// All pointers must be valid. The returned `*data` pointer must not be used
/// after `typst_result_free`.
#[no_mangle]
pub unsafe extern "C" fn typst_result_get_html(
    result: *const TypstCompileResult,
    data: *mut *const u8,
    len: *mut i32,
) -> i32 {
    if result.is_null() || data.is_null() || len.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    match &(*result).kind {
        CompileResultKind::Success { html, .. } => {
            *data = html.as_ptr();
            *len = html.len() as i32;
            TYPST_OK
        }
        CompileResultKind::Failure { .. } => TYPST_ERR_COMPILE_FAILED,
    }
}

/// Render one page (0-indexed) to PNG at the given scale.
///
/// `pixels_per_pt` controls the resolution: `2.0` ≈ 144 DPI (the Typst CLI
/// default). On success, `*out_buffer` receives a buffer handle that must be
/// freed with `typst_buffer_free`; read its contents via
/// `typst_buffer_get_data`.
///
/// Returns `TYPST_OK` on success, `TYPST_ERR_COMPILE_FAILED` if the result is
/// a failure, `TYPST_ERR_PAGE_OUT_OF_RANGE` for a bad page index,
/// `TYPST_ERR_INVALID_ARGUMENT` for a non-finite or non-positive scale, and
/// `TYPST_ERR_INTERNAL` if PNG encoding fails.
///
/// # Safety
/// All pointers must be valid.
#[no_mangle]
pub unsafe extern "C" fn typst_result_render_png(
    result: *const TypstCompileResult,
    page: i32,
    pixels_per_pt: f32,
    out_buffer: *mut *mut TypstBuffer,
) -> i32 {
    if result.is_null() || out_buffer.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    if !pixels_per_pt.is_finite() || pixels_per_pt <= 0.0 {
        return TYPST_ERR_INVALID_ARGUMENT;
    }
    match &(*result).kind {
        CompileResultKind::Success { document, .. } => {
            if page < 0 || page as usize >= document.pages().len() {
                return TYPST_ERR_PAGE_OUT_OF_RANGE;
            }
            let options = typst_render::RenderOptions {
                pixel_per_pt: (pixels_per_pt as f64).into(),
                render_bleed: false,
            };
            let pixmap =
                typst_render::render(&document.pages()[page as usize], &options);
            match pixmap.encode_png() {
                Ok(data) => {
                    *out_buffer = Box::into_raw(Box::new(TypstBuffer { data }));
                    TYPST_OK
                }
                Err(_) => TYPST_ERR_INTERNAL,
            }
        }
        CompileResultKind::Failure { .. } => TYPST_ERR_COMPILE_FAILED,
    }
}

/// Get a pointer to a buffer's data.
///
/// `*data` and `*len` are set to the buffer contents. The pointer is owned by
/// the buffer and remains valid until `typst_buffer_free` is called.
///
/// # Safety
/// All pointers must be valid. The returned `*data` pointer must not be used
/// after `typst_buffer_free`.
#[no_mangle]
pub unsafe extern "C" fn typst_buffer_get_data(
    buffer: *const TypstBuffer,
    data: *mut *const u8,
    len: *mut i32,
) -> i32 {
    if buffer.is_null() || data.is_null() || len.is_null() {
        return TYPST_ERR_NULL_POINTER;
    }
    *data = (*buffer).data.as_ptr();
    *len = (*buffer).data.len() as i32;
    TYPST_OK
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
    // Derived from Cargo.toml so the reported version cannot drift from the
    // crate version the way a hardcoded literal does.
    // SAFETY: The byte string is null-terminated and lives for 'static.
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

// ===========================================================================
// World implementation (to be expanded)
// ===========================================================================

mod world {
    //! Minimal `typst::World` implementation for in-memory compilation.

    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use chrono::{Datelike, Timelike};
    use typst::diag::FileResult;
    use typst::foundations::{Bytes, Datetime, Duration};
    use typst::syntax::{FileId, Source, VirtualPath, VirtualRoot};
    use typst::text::{Font, FontBook};
    use typst::utils::LazyHash;
    use typst::{Feature, Library, LibraryExt, World};
    use typst_kit::fonts::FontStore;

    pub struct SimpleWorld {
        library: LazyHash<Library>,
        fonts: FontStore,
        main_source: Source,
        root: Option<PathBuf>,
        sources: Mutex<HashMap<FileId, Source>>,
        /// In-memory files; take precedence over files on disk.
        files: HashMap<VirtualPath, Bytes>,
    }

    impl SimpleWorld {
        /// Build a world for one compilation.
        ///
        /// `main_id` gives the entry source a position in the virtual file
        /// system. Typst resolves a relative path against the *importing*
        /// file's virtual parent, so this is what decides whether `../` from
        /// the entry stays inside the root. `None` falls back to
        /// `Source::detached`, which pins the entry at `/main.typ`, i.e.
        /// directly at the root, where any `..` necessarily escapes.
        pub fn new(
            text: &str,
            main_id: Option<FileId>,
            root: Option<PathBuf>,
            font_paths: &[PathBuf],
            files: HashMap<VirtualPath, Bytes>,
        ) -> Self {
            // Font paths have the highest priority, then system fonts, then
            // embedded fonts — matching the old typst-kit `FontSearcher` order.
            let mut fonts = FontStore::new();
            for path in font_paths {
                fonts.extend(typst_kit::fonts::scan(path));
            }
            fonts.extend(typst_kit::fonts::system());
            fonts.extend(typst_kit::fonts::embedded());

            let main_source = match main_id {
                Some(id) => Source::new(id, text.to_owned()),
                None => Source::detached(text),
            };

            let library = Library::builder()
                .with_features(std::iter::once(Feature::Html).collect())
                .build();

            SimpleWorld {
                library: LazyHash::new(library),
                fonts,
                main_source,
                root,
                sources: Mutex::new(HashMap::new()),
                files,
            }
        }

        /// Resolve a FileId to a real filesystem path using the root.
        fn resolve_path(&self, id: FileId) -> FileResult<PathBuf> {
            if matches!(id.root(), VirtualRoot::Package(_)) {
                return Err(typst::diag::FileError::Package(
                    typst::diag::PackageError::Other(Some(
                        typst::ecow::eco_format!("package imports are not supported"),
                    )),
                ));
            }
            let root = self.root.as_deref().unwrap_or_else(|| Path::new("."));
            id.vpath()
                .realize(root)
                .map_err(|_| typst::diag::FileError::AccessDenied)
        }
    }

    impl World for SimpleWorld {
        fn library(&self) -> &LazyHash<Library> {
            &self.library
        }

        fn book(&self) -> &LazyHash<FontBook> {
            self.fonts.book()
        }

        fn main(&self) -> FileId {
            self.main_source.id()
        }

        fn source(&self, id: FileId) -> FileResult<Source> {
            if id == self.main_source.id() {
                return Ok(self.main_source.clone());
            }

            // Check cache
            {
                let sources = self.sources.lock().unwrap();
                if let Some(source) = sources.get(&id) {
                    return Ok(source.clone());
                }
            }

            // Virtual files take precedence over disk.
            let virtual_text = if matches!(id.root(), VirtualRoot::Project) {
                match self.files.get(id.vpath()) {
                    Some(bytes) => Some(
                        std::str::from_utf8(bytes)
                            .map(str::to_owned)
                            .map_err(|_| typst::diag::FileError::InvalidUtf8)?,
                    ),
                    None => None,
                }
            } else {
                None
            };

            let text = match virtual_text {
                Some(text) => text,
                None => {
                    // Load from disk
                    let path = self.resolve_path(id)?;
                    fs::read_to_string(&path)
                        .map_err(|e| typst::diag::FileError::from_io(e, &path))?
                }
            };
            let source = Source::new(id, text);

            let mut sources = self.sources.lock().unwrap();
            sources.insert(id, source.clone());
            Ok(source)
        }

        fn file(&self, id: FileId) -> FileResult<Bytes> {
            // The entry source shadows disk and virtual files alike, so that
            // `#read`-ing the entry's own path yields the text that was
            // actually compiled rather than whatever is on disk. `source`
            // short-circuits the same way.
            if id == self.main_source.id() {
                return Ok(Bytes::new(self.main_source.text().as_bytes().to_vec()));
            }

            // Virtual files take precedence over disk.
            if matches!(id.root(), VirtualRoot::Project) {
                if let Some(bytes) = self.files.get(id.vpath()) {
                    return Ok(bytes.clone());
                }
            }
            let path = self.resolve_path(id)?;
            let data = fs::read(&path)
                .map_err(|e| typst::diag::FileError::from_io(e, &path))?;
            Ok(Bytes::new(data))
        }

        fn font(&self, index: usize) -> Option<Font> {
            self.fonts.font(index)
        }

        fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
            let now = chrono::Local::now();
            if let Some(offset) = offset {
                let utc = now.naive_utc()
                    + chrono::Duration::seconds(offset.seconds() as i64);
                Datetime::from_ymd_hms(
                    utc.year(),
                    utc.month() as u8,
                    utc.day() as u8,
                    utc.hour() as u8,
                    utc.minute() as u8,
                    utc.second() as u8,
                )
            } else {
                let local = now.naive_local();
                Datetime::from_ymd_hms(
                    local.year(),
                    local.month() as u8,
                    local.day() as u8,
                    local.hour() as u8,
                    local.minute() as u8,
                    local.second() as u8,
                )
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::path::Path;
    use std::ptr;

    /// A minimal valid 1x1 transparent PNG.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
        0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
        0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
        0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
        0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    unsafe fn add_file(compiler: *mut TypstCompiler, path: &str, data: &[u8]) -> i32 {
        let path = CString::new(path).unwrap();
        typst_compiler_add_file(compiler, path.as_ptr(), data.as_ptr(), data.len() as i32)
    }

    unsafe fn compile(compiler: *mut TypstCompiler, source: &str) -> *mut TypstCompileResult {
        let mut result: *mut TypstCompileResult = ptr::null_mut();
        let rc = typst_compile(compiler, source.as_ptr(), source.len() as i32, &mut result);
        assert_eq!(rc, TYPST_OK);
        assert!(!result.is_null());
        result
    }

    #[test]
    fn compiler_lifecycle() {
        unsafe {
            let compiler = typst_compiler_new();
            assert!(!compiler.is_null());
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn compile_simple_source() {
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

    #[test]
    fn add_file_then_compile_image() {
        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(add_file(compiler, "logo.png", TINY_PNG), TYPST_OK);

            let result = compile(compiler, "#image(\"logo.png\")");
            assert_eq!(typst_result_is_success(result), 1);

            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn compile_image_without_file_fails() {
        unsafe {
            let compiler = typst_compiler_new();
            let result = compile(compiler, "#image(\"missing.png\")");
            assert_eq!(typst_result_is_success(result), 0);
            assert!(typst_result_diagnostic_count(result) >= 1);

            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn add_file_leading_slash_equivalent() {
        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(add_file(compiler, "/logo.png", TINY_PNG), TYPST_OK);

            let result = compile(compiler, "#image(\"logo.png\")");
            assert_eq!(typst_result_is_success(result), 1);

            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn add_file_overwrite() {
        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(add_file(compiler, "logo.png", b"not a png"), TYPST_OK);
            assert_eq!(add_file(compiler, "logo.png", TINY_PNG), TYPST_OK);

            let result = compile(compiler, "#image(\"logo.png\")");
            assert_eq!(typst_result_is_success(result), 1);

            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn virtual_typ_import() {
        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(
                add_file(compiler, "helper.typ", b"#let greeting = \"hi\""),
                TYPST_OK
            );

            let result = compile(compiler, "#import \"helper.typ\": greeting\n#greeting");
            assert_eq!(typst_result_is_success(result), 1);

            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn add_file_invalid_args() {
        unsafe {
            let compiler = typst_compiler_new();
            let path = CString::new("x.png").unwrap();

            assert_eq!(
                typst_compiler_add_file(
                    ptr::null_mut(),
                    path.as_ptr(),
                    TINY_PNG.as_ptr(),
                    TINY_PNG.len() as i32
                ),
                TYPST_ERR_NULL_POINTER
            );
            assert_eq!(
                typst_compiler_add_file(
                    compiler,
                    ptr::null(),
                    TINY_PNG.as_ptr(),
                    TINY_PNG.len() as i32
                ),
                TYPST_ERR_NULL_POINTER
            );
            // Null data with positive length.
            assert_eq!(
                typst_compiler_add_file(compiler, path.as_ptr(), ptr::null(), 1),
                TYPST_ERR_INVALID_ARGUMENT
            );
            // Negative length.
            assert_eq!(
                typst_compiler_add_file(compiler, path.as_ptr(), TINY_PNG.as_ptr(), -1),
                TYPST_ERR_INVALID_ARGUMENT
            );
            // Paths that normalize to the bare root.
            assert_eq!(add_file(compiler, "", TINY_PNG), TYPST_ERR_INVALID_ARGUMENT);
            assert_eq!(add_file(compiler, "/", TINY_PNG), TYPST_ERR_INVALID_ARGUMENT);
            // Null data with zero length registers an empty file.
            assert_eq!(
                typst_compiler_add_file(compiler, path.as_ptr(), ptr::null(), 0),
                TYPST_OK
            );

            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn clear_files_removes_virtual_files() {
        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(add_file(compiler, "logo.png", TINY_PNG), TYPST_OK);
            assert_eq!(typst_compiler_clear_files(compiler), TYPST_OK);

            let result = compile(compiler, "#image(\"logo.png\")");
            assert_eq!(typst_result_is_success(result), 0);

            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn render_png_produces_png_bytes() {
        unsafe {
            let compiler = typst_compiler_new();
            let result = compile(compiler, "Hello, PNG!");
            assert_eq!(typst_result_is_success(result), 1);

            let mut buffer: *mut TypstBuffer = ptr::null_mut();
            assert_eq!(typst_result_render_png(result, 0, 1.0, &mut buffer), TYPST_OK);
            assert!(!buffer.is_null());

            let mut data: *const u8 = ptr::null();
            let mut len: i32 = 0;
            assert_eq!(typst_buffer_get_data(buffer, &mut data, &mut len), TYPST_OK);
            assert!(len > 8);
            let magic = slice::from_raw_parts(data, 4);
            assert_eq!(magic, &[0x89, 0x50, 0x4E, 0x47]);

            typst_buffer_free(buffer);
            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    #[test]
    fn render_png_error_codes() {
        unsafe {
            let compiler = typst_compiler_new();
            let ok_result = compile(compiler, "Hello");
            let failed_result = compile(compiler, "#image(\"missing.png\")");
            let mut buffer: *mut TypstBuffer = ptr::null_mut();

            assert_eq!(
                typst_result_render_png(ok_result, 5, 1.0, &mut buffer),
                TYPST_ERR_PAGE_OUT_OF_RANGE
            );
            assert_eq!(
                typst_result_render_png(ok_result, 0, 0.0, &mut buffer),
                TYPST_ERR_INVALID_ARGUMENT
            );
            assert_eq!(
                typst_result_render_png(ok_result, 0, f32::NAN, &mut buffer),
                TYPST_ERR_INVALID_ARGUMENT
            );
            assert_eq!(
                typst_result_render_png(failed_result, 0, 1.0, &mut buffer),
                TYPST_ERR_COMPILE_FAILED
            );
            assert_eq!(
                typst_result_render_png(ptr::null(), 0, 1.0, &mut buffer),
                TYPST_ERR_NULL_POINTER
            );

            typst_result_free(ok_result);
            typst_result_free(failed_result);
            typst_compiler_free(compiler);
        }
    }

    // -----------------------------------------------------------------------
    // Entry virtual path (`typst_compile_with_path`)
    // -----------------------------------------------------------------------

    /// A throwaway directory tree that deletes itself on drop.
    struct TempTree(PathBuf);

    impl TempTree {
        fn new(tag: &str) -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("typst-ffi-{tag}-{unique}"));
            std::fs::create_dir_all(&dir).unwrap();
            TempTree(dir)
        }

        /// Write `contents` to `relative`, creating parent directories.
        fn write(&self, relative: &str, contents: &str) {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    unsafe fn set_root(compiler: *mut TypstCompiler, path: &Path) -> i32 {
        let path = CString::new(path.to_str().unwrap()).unwrap();
        typst_compiler_set_root(compiler, path.as_ptr())
    }

    /// Compile with an explicit entry virtual path. `None` means detached.
    unsafe fn compile_at(
        compiler: *mut TypstCompiler,
        source: &str,
        vpath: Option<&str>,
    ) -> *mut TypstCompileResult {
        let vpath = vpath.map(|v| CString::new(v).unwrap());
        let vpath_ptr = vpath.as_ref().map_or(ptr::null(), |v| v.as_ptr());
        let mut result: *mut TypstCompileResult = ptr::null_mut();
        let rc = typst_compile_with_path(
            compiler,
            source.as_ptr(),
            source.len() as i32,
            vpath_ptr,
            &mut result,
        );
        assert_eq!(rc, TYPST_OK);
        assert!(!result.is_null());
        result
    }

    /// Concatenate a result's diagnostics, for assertion messages.
    unsafe fn diagnostics(result: *const TypstCompileResult) -> String {
        let count = typst_result_diagnostic_count(result);
        let mut out = String::new();
        for i in 0..count {
            let mut severity: i32 = 0;
            let mut message: *const u8 = ptr::null();
            let mut message_len: i32 = 0;
            let mut line: i64 = 0;
            let mut column: i64 = 0;
            if typst_result_get_diagnostic(
                result,
                i,
                &mut severity,
                &mut message,
                &mut message_len,
                &mut line,
                &mut column,
            ) == TYPST_OK
                && !message.is_null()
            {
                let bytes = slice::from_raw_parts(message, message_len as usize);
                out.push_str(&String::from_utf8_lossy(bytes));
                out.push('\n');
            }
        }
        out
    }

    /// The bug from issue #19: an entry in a subdirectory reaching a sibling
    /// directory through `..`. Fails when the entry is detached, because a
    /// detached entry sits at the root and `..` escapes it.
    #[test]
    fn entry_vpath_allows_parent_relative_import() {
        let tree = TempTree::new("parent-import");
        tree.write("other/dep.typ", "#let greet = \"hi\"\n");
        let source = "#import \"../other/dep.typ\": greet\n#greet\n";

        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(set_root(compiler, &tree.0), TYPST_OK);

            // Detached: the entry is pinned at /main.typ, so `..` escapes.
            let detached = compile_at(compiler, source, None);
            assert_eq!(
                typst_result_is_success(detached),
                0,
                "detached entry unexpectedly resolved `..`"
            );
            typst_result_free(detached);

            // Placed at /sub/main.typ, `..` resolves to /other/dep.typ.
            let placed = compile_at(compiler, source, Some("sub/main.typ"));
            assert_eq!(
                typst_result_is_success(placed),
                1,
                "placed entry failed: {}",
                diagnostics(placed)
            );
            typst_result_free(placed);

            typst_compiler_free(compiler);
        }
    }

    /// A sibling import must resolve next to the entry, not at the root.
    /// This is the case that regresses if the entry keeps a detached identity
    /// while the root is widened.
    #[test]
    fn entry_vpath_resolves_sibling_relative_to_entry() {
        let tree = TempTree::new("sibling-import");
        tree.write("sub/sib.typ", "#let sib = \"hi\"\n");
        // A decoy at the root: if resolution is relative to the root rather
        // than to the entry, this one gets picked up and the marker differs.
        tree.write("sib.typ", "#let sib = \"WRONG\"\n");
        let source = "#import \"sib.typ\": sib\n#sib\n";

        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(set_root(compiler, &tree.0), TYPST_OK);

            let placed = compile_at(compiler, source, Some("sub/main.typ"));
            assert_eq!(
                typst_result_is_success(placed),
                1,
                "sibling import failed: {}",
                diagnostics(placed)
            );
            typst_result_free(placed);

            typst_compiler_free(compiler);
        }
    }

    /// `#read` and friends go through the same relative-path resolution as
    /// `#import`, so the fix must cover them too.
    #[test]
    fn entry_vpath_applies_to_read() {
        let tree = TempTree::new("read");
        tree.write("other/data.txt", "payload");
        let source = "#read(\"../other/data.txt\")\n";

        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(set_root(compiler, &tree.0), TYPST_OK);

            let placed = compile_at(compiler, source, Some("sub/main.typ"));
            assert_eq!(
                typst_result_is_success(placed),
                1,
                "parent-relative read failed: {}",
                diagnostics(placed)
            );
            typst_result_free(placed);

            typst_compiler_free(compiler);
        }
    }

    /// A null vpath must behave exactly like `typst_compile`, keeping the
    /// entry at `/main.typ`. Importing "main.typ" from a detached entry is
    /// therefore a self-import, which Typst reports as a cycle.
    #[test]
    fn entry_vpath_null_stays_detached() {
        unsafe {
            let compiler = typst_compiler_new();
            let result = compile_at(compiler, "#import \"main.typ\": x\n#x\n", None);
            assert_eq!(typst_result_is_success(result), 0);
            assert!(
                diagnostics(result).contains("cyclic"),
                "expected a cyclic import, got: {}",
                diagnostics(result)
            );
            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    /// Backslashes are separators on every platform, matching
    /// `typst_compiler_add_file`. A leading slash is also accepted.
    #[test]
    fn entry_vpath_normalizes_separators() {
        let tree = TempTree::new("separators");
        tree.write("other/dep.typ", "#let greet = \"hi\"\n");
        let source = "#import \"../other/dep.typ\": greet\n#greet\n";

        for vpath in ["sub\\main.typ", "/sub/main.typ", "./sub/main.typ"] {
            unsafe {
                let compiler = typst_compiler_new();
                assert_eq!(set_root(compiler, &tree.0), TYPST_OK);
                let result = compile_at(compiler, source, Some(vpath));
                assert_eq!(
                    typst_result_is_success(result),
                    1,
                    "vpath {vpath:?} failed: {}",
                    diagnostics(result)
                );
                typst_result_free(result);
                typst_compiler_free(compiler);
            }
        }
    }

    /// An unusable entry vpath is API misuse, not a compile diagnostic.
    #[test]
    fn entry_vpath_rejects_invalid_paths() {
        unsafe {
            let compiler = typst_compiler_new();
            let source = b"Hello";

            for vpath in ["", "/", ".", "..", "../escapes.typ"] {
                let c_vpath = CString::new(vpath).unwrap();
                let mut result: *mut TypstCompileResult = ptr::null_mut();
                let rc = typst_compile_with_path(
                    compiler,
                    source.as_ptr(),
                    source.len() as i32,
                    c_vpath.as_ptr(),
                    &mut result,
                );
                assert_eq!(
                    rc, TYPST_ERR_INVALID_ARGUMENT,
                    "vpath {vpath:?} should have been rejected"
                );
                assert!(result.is_null());
            }

            typst_compiler_free(compiler);
        }
    }

    /// The entry text wins over a file of the same name on disk, so that what
    /// gets compiled is what the caller passed.
    #[test]
    fn entry_vpath_shadows_disk() {
        let tree = TempTree::new("shadow");
        tree.write("sub/main.typ", "#let marker = \"FROM DISK\"\n");

        unsafe {
            let compiler = typst_compiler_new();
            assert_eq!(set_root(compiler, &tree.0), TYPST_OK);

            // Importing the entry's own path is a self-import either way; the
            // point is that it resolves to the in-memory entry, not the file.
            let result = compile_at(
                compiler,
                "#import \"main.typ\": marker\n#marker\n",
                Some("sub/main.typ"),
            );
            assert_eq!(typst_result_is_success(result), 0);
            assert!(
                diagnostics(result).contains("cyclic"),
                "entry did not shadow the on-disk file, got: {}",
                diagnostics(result)
            );
            typst_result_free(result);
            typst_compiler_free(compiler);
        }
    }

    /// Null-pointer and argument handling on the new export.
    #[test]
    fn entry_vpath_null_safety() {
        unsafe {
            let compiler = typst_compiler_new();
            let source = b"Hello";
            let mut result: *mut TypstCompileResult = ptr::null_mut();

            assert_eq!(
                typst_compile_with_path(
                    ptr::null_mut(),
                    source.as_ptr(),
                    source.len() as i32,
                    ptr::null(),
                    &mut result
                ),
                TYPST_ERR_NULL_POINTER
            );
            assert_eq!(
                typst_compile_with_path(compiler, ptr::null(), 0, ptr::null(), &mut result),
                TYPST_ERR_NULL_POINTER
            );
            assert_eq!(
                typst_compile_with_path(
                    compiler,
                    source.as_ptr(),
                    source.len() as i32,
                    ptr::null(),
                    ptr::null_mut()
                ),
                TYPST_ERR_NULL_POINTER
            );
            assert_eq!(
                typst_compile_with_path(compiler, source.as_ptr(), -1, ptr::null(), &mut result),
                TYPST_ERR_INVALID_ARGUMENT
            );

            typst_compiler_free(compiler);
        }
    }
}
