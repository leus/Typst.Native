//! # typst-scribus
//!
//! Export Typst [`PagedDocument`]s into the Scribus SLA (XML) format.

mod color;
mod image;
mod render;

use typst_library::layout::{PagedDocument, Page};
use xmlwriter::XmlWriter;

use crate::render::SlaRenderer;

/// Scratch-space constants (Scribus positions pages inside a virtual canvas).
const SCRATCH_LEFT: f64 = 100.0;
const SCRATCH_TOP: f64 = 20.0;
const GAP_VERTICAL: f64 = 40.0;

/// Options for the SLA export.
#[derive(Debug, Clone)]
pub struct SlaOptions {
    pub title: String,
    pub author: String,
}

impl Default for SlaOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
        }
    }
}

/// Export a full [`PagedDocument`] to a Scribus SLA XML string.
pub fn sla(document: &PagedDocument, options: &SlaOptions) -> String {
    let mut xml = XmlWriter::new(xmlwriter::Options::default());

    xml.write_declaration();

    xml.start_element("SCRIBUSUTF8NEW");
    xml.write_attribute("Version", "1.6.4");

    xml.start_element("DOCUMENT");
    write_document_attrs(&mut xml, document, options);

    // Pre-collect all colors used in the document.
    let mut renderer = SlaRenderer::new();
    for page in document.pages.iter() {
        renderer.collect_colors_page(page);
    }

    // --- CheckProfile elements ---
    write_check_profiles(&mut xml);

    // --- Color palette ---
    write_color_defs(&mut xml, &renderer);

    // --- Structural preamble ---
    write_structural_elements(&mut xml);

    // --- MASTERPAGE ---
    write_master_page(&mut xml, document);

    // --- PAGE elements ---
    for (page_idx, page) in document.pages.iter().enumerate() {
        write_page_element(&mut xml, page_idx, page, document);
    }

    // --- PAGEOBJECT elements ---
    for (page_idx, page) in document.pages.iter().enumerate() {
        let ypos = page_ypos(document, page_idx);
        renderer.render_page(&mut xml, page_idx, page, ypos);
    }

    xml.end_element(); // DOCUMENT
    xml.end_element(); // SCRIBUSUTF8NEW

    xml.end_document()
}

/// Compute the PAGEYPOS for a given page index, accounting for scratch space
/// and inter-page gaps.
pub fn page_ypos(document: &PagedDocument, page_idx: usize) -> f64 {
    let mut y = SCRATCH_TOP;
    for i in 0..page_idx {
        let h = document.pages[i].frame.height().to_pt();
        y += h + GAP_VERTICAL;
    }
    y
}

/// Write the comprehensive DOCUMENT attributes that Scribus 1.6.4 expects.
fn write_document_attrs(
    xml: &mut XmlWriter,
    document: &PagedDocument,
    options: &SlaOptions,
) {
    let (w, h) = document
        .pages
        .first()
        .map(|p| (p.frame.width().to_pt(), p.frame.height().to_pt()))
        .unwrap_or((595.2744, 841.8888));

    let orientation = if w > h { "1" } else { "0" };

    xml.write_attribute("ANZPAGES", &document.pages.len().to_string());
    xml.write_attribute("PAGEWIDTH", &format!("{w:.4}"));
    xml.write_attribute("PAGEHEIGHT", &format!("{h:.4}"));
    xml.write_attribute("BORDERLEFT", "0");
    xml.write_attribute("BORDERRIGHT", "0");
    xml.write_attribute("BORDERTOP", "0");
    xml.write_attribute("BORDERBOTTOM", "0");
    xml.write_attribute("PRESET", "0");
    xml.write_attribute("BleedTop", "0");
    xml.write_attribute("BleedLeft", "0");
    xml.write_attribute("BleedRight", "0");
    xml.write_attribute("BleedBottom", "0");
    xml.write_attribute("ORIENTATION", orientation);
    xml.write_attribute("PAGESIZE", "Custom");
    xml.write_attribute("FIRSTNUM", "1");
    xml.write_attribute("BOOK", "0");
    xml.write_attribute("AUTOSPALTEN", "1");
    xml.write_attribute("ABSTSPALTEN", "11");
    xml.write_attribute("UNITS", "0");
    xml.write_attribute("DFONT", "Arial Regular");
    xml.write_attribute("DSIZE", "12");
    xml.write_attribute("DCOL", "1");
    xml.write_attribute("DGAP", "0");
    xml.write_attribute("TabFill", "");
    xml.write_attribute("TabWidth", "36");
    xml.write_attribute("TextDistLeft", "0");
    xml.write_attribute("TextDistRight", "0");
    xml.write_attribute("TextDistBottom", "0");
    xml.write_attribute("TextDistTop", "0");
    xml.write_attribute("FirstLineOffset", "1");
    xml.write_attribute("AUTHOR", &options.author);
    xml.write_attribute("COMMENTS", "");
    xml.write_attribute("KEYWORDS", "");
    xml.write_attribute("PUBLISHER", "");
    xml.write_attribute("DOCDATE", "");
    xml.write_attribute("DOCTYPE", "");
    xml.write_attribute("DOCFORMAT", "");
    xml.write_attribute("DOCIDENT", "");
    xml.write_attribute("DOCSOURCE", "");
    xml.write_attribute("DOCLANGINFO", "");
    xml.write_attribute("DOCRELATION", "");
    xml.write_attribute("DOCCOVER", "");
    xml.write_attribute("DOCRIGHTS", "");
    xml.write_attribute("DOCCONTRIB", "");
    xml.write_attribute("TITLE", &options.title);
    xml.write_attribute("SUBJECT", "");
    xml.write_attribute("VHOCH", "33");
    xml.write_attribute("VHOCHSC", "66");
    xml.write_attribute("VTIEF", "33");
    xml.write_attribute("VTIEFSC", "66");
    xml.write_attribute("VKAPIT", "75");
    xml.write_attribute("BASEGRID", "14.4");
    xml.write_attribute("BASEO", "0");
    xml.write_attribute("AUTOL", "100");
    xml.write_attribute("UnderlinePos", "-1");
    xml.write_attribute("UnderlineWidth", "-1");
    xml.write_attribute("StrikeThruPos", "-1");
    xml.write_attribute("StrikeThruWidth", "-1");
    xml.write_attribute("GROUPC", "1");
    xml.write_attribute("HCMS", "0");
    xml.write_attribute("DPSo", "0");
    xml.write_attribute("DPSFo", "0");
    xml.write_attribute("DPuse", "0");
    xml.write_attribute("DPgam", "0");
    xml.write_attribute("DPbla", "1");
    xml.write_attribute("DPPr", "");
    xml.write_attribute("DPIn", "");
    xml.write_attribute("DPInCMYK", "");
    xml.write_attribute("DPIn2", "");
    xml.write_attribute("DPIn3", "");
    xml.write_attribute("DISc", "1");
    xml.write_attribute("DIIm", "0");
    xml.write_attribute("ALAYER", "0");
    xml.write_attribute("LANGUAGE", "en_US");
    xml.write_attribute("AUTOMATIC", "1");
    xml.write_attribute("AUTOCHECK", "0");
    xml.write_attribute("GUIDELOCK", "0");
    xml.write_attribute("SnapToGuides", "0");
    xml.write_attribute("SnapToGrid", "0");
    xml.write_attribute("SnapToElement", "0");
    xml.write_attribute("MINGRID", "20");
    xml.write_attribute("MAJGRID", "100");
    xml.write_attribute("SHOWGRID", "0");
    xml.write_attribute("SHOWGUIDES", "1");
    xml.write_attribute("showcolborders", "0");
    xml.write_attribute("SHOWFRAME", "1");
    xml.write_attribute("SHOWControl", "0");
    xml.write_attribute("SHOWLAYERM", "0");
    xml.write_attribute("SHOWMARGIN", "1");
    xml.write_attribute("SHOWBASE", "0");
    xml.write_attribute("SHOWPICT", "1");
    xml.write_attribute("SHOWLINK", "0");
    xml.write_attribute("rulerMode", "1");
    xml.write_attribute("showrulers", "1");
    xml.write_attribute("showBleed", "1");
    xml.write_attribute("rulerXoffset", "0");
    xml.write_attribute("rulerYoffset", "0");
    xml.write_attribute("GuideRad", "10");
    xml.write_attribute("GRAB", "4");
    xml.write_attribute("POLYC", "4");
    xml.write_attribute("POLYF", "0.5");
    xml.write_attribute("POLYR", "0");
    xml.write_attribute("POLYIR", "0");
    xml.write_attribute("POLYCUR", "0");
    xml.write_attribute("POLYOCUR", "0");
    xml.write_attribute("POLYS", "0");
    xml.write_attribute("arcStartAngle", "30");
    xml.write_attribute("arcSweepAngle", "300");
    xml.write_attribute("spiralStartAngle", "0");
    xml.write_attribute("spiralEndAngle", "1080");
    xml.write_attribute("spiralFactor", "1.2");
    xml.write_attribute("AutoSave", "1");
    xml.write_attribute("AutoSaveTime", "600000");
    xml.write_attribute("AutoSaveCount", "1");
    xml.write_attribute("AutoSaveKeep", "0");
    xml.write_attribute("AUtoSaveInDocDir", "1");
    xml.write_attribute("AutoSaveDir", "");
    xml.write_attribute("ScratchBottom", "20");
    xml.write_attribute("ScratchLeft", "100");
    xml.write_attribute("ScratchRight", "100");
    xml.write_attribute("ScratchTop", "20");
    xml.write_attribute("GapHorizontal", "0");
    xml.write_attribute("GapVertical", "40");
    xml.write_attribute("StartArrow", "0");
    xml.write_attribute("EndArrow", "0");
    xml.write_attribute("PEN", "Black");
    xml.write_attribute("BRUSH", "None");
    xml.write_attribute("PENLINE", "Black");
    xml.write_attribute("PENTEXT", "Black");
    xml.write_attribute("StrokeText", "Black");
    xml.write_attribute("TextBackGround", "None");
    xml.write_attribute("TextLineColor", "None");
    xml.write_attribute("TextBackGroundShade", "100");
    xml.write_attribute("TextLineShade", "100");
    xml.write_attribute("TextPenShade", "100");
    xml.write_attribute("TextStrokeShade", "100");
    xml.write_attribute("STIL", "1");
    xml.write_attribute("STILLINE", "1");
    xml.write_attribute("WIDTH", "1");
    xml.write_attribute("WIDTHLINE", "1");
    xml.write_attribute("PENSHADE", "100");
    xml.write_attribute("LINESHADE", "100");
    xml.write_attribute("BRUSHSHADE", "100");
    xml.write_attribute("CPICT", "None");
    xml.write_attribute("PICTSHADE", "100");
    xml.write_attribute("CSPICT", "None");
    xml.write_attribute("PICTSSHADE", "100");
    xml.write_attribute("PICTSCX", "1");
    xml.write_attribute("PICTSCY", "1");
    xml.write_attribute("PSCALE", "1");
    xml.write_attribute("PASPECT", "1");
    xml.write_attribute("EmbeddedPath", "0");
    xml.write_attribute("HalfRes", "1");
    xml.write_attribute("dispX", "10");
    xml.write_attribute("dispY", "10");
    xml.write_attribute("constrain", "15");
    xml.write_attribute("MINORC", "#00ff00");
    xml.write_attribute("MAJORC", "#00ff00");
    xml.write_attribute("GuideC", "#000080");
    xml.write_attribute("BaseC", "#c0c0c0");
    xml.write_attribute("renderStack", "2 0 4 1 3");
    xml.write_attribute("GridType", "0");
    xml.write_attribute("PAGEC", "#ffffff");
    xml.write_attribute("MARGC", "#0000ff");
    xml.write_attribute("RANDF", "0");
    xml.write_attribute("currentProfile", "PDF 1.4");
    xml.write_attribute("calligraphicPenFillColor", "Black");
    xml.write_attribute("calligraphicPenLineColor", "Black");
    xml.write_attribute("calligraphicPenFillColorShade", "100");
    xml.write_attribute("calligraphicPenLineColorShade", "100");
    xml.write_attribute("calligraphicPenLineWidth", "1");
    xml.write_attribute("calligraphicPenAngle", "0");
    xml.write_attribute("calligraphicPenWidth", "10");
    xml.write_attribute("calligraphicPenStyle", "1");
}

/// Write the 8 CheckProfile elements that Scribus 1.6.4 expects.
fn write_check_profiles(xml: &mut XmlWriter) {
    let profiles = [
        ("PDF 1.3", "1"),
        ("PDF 1.4", "0"),
        ("PDF 1.5", "0"),
        ("PDF 1.6", "0"),
        ("PDF/X-1a", "1"),
        ("PDF/X-3", "1"),
        ("PDF/X-4", "0"),
        ("PostScript", "1"),
    ];
    for (name, transparency) in &profiles {
        xml.start_element("CheckProfile");
        xml.write_attribute("Name", name);
        xml.write_attribute("ignoreErrors", "0");
        xml.write_attribute("autoCheck", "1");
        xml.write_attribute("checkGlyphs", "1");
        xml.write_attribute("checkOrphans", "1");
        xml.write_attribute("checkOverflow", "1");
        xml.write_attribute("checkPictures", "1");
        xml.write_attribute("checkPartFilledImageFrames", "0");
        xml.write_attribute("checkResolution", "1");
        xml.write_attribute("checkTransparency", transparency);
        xml.write_attribute("minResolution", "144");
        xml.write_attribute("maxResolution", "2400");
        xml.write_attribute("checkAnnotations", "0");
        xml.write_attribute("checkRasterPDF", "1");
        xml.write_attribute("checkForGIF", "1");
        xml.write_attribute("ignoreOffLayers", "0");
        xml.write_attribute("checkNotCMYKOrSpot", "0");
        xml.write_attribute("checkDeviceColorsAndOutputIntent", "0");
        xml.write_attribute("checkFontNotEmbedded", "1");
        xml.write_attribute("checkFontIsOpenType", "1");
        xml.write_attribute("checkAppliedMasterDifferentSide", "1");
        xml.write_attribute("checkEmptyTextFrames", "1");
        xml.end_element();
    }
}

/// Write the full Scribus standard color palette plus document-specific colors.
fn write_color_defs(xml: &mut XmlWriter, renderer: &SlaRenderer) {
    // Standard Scribus colors
    let standard_cmyk: &[(&str, &str, &str, &str, &str)] = &[
        ("Black",      "0",   "0",   "0",   "100"),
        ("Cool Black", "60",  "0",   "0",   "100"),
        ("Cyan",       "100", "0",   "0",   "0"),
        ("Magenta",    "0",   "100", "0",   "0"),
        ("Rich Black", "60",  "40",  "40",  "100"),
        ("Warm Black", "0",   "60",  "30",  "100"),
        ("White",      "0",   "0",   "0",   "0"),
        ("Yellow",     "0",   "0",   "100", "0"),
    ];
    for (name, c, m, y, k) in standard_cmyk {
        xml.start_element("COLOR");
        xml.write_attribute("NAME", name);
        xml.write_attribute("SPACE", "CMYK");
        xml.write_attribute("C", c);
        xml.write_attribute("M", m);
        xml.write_attribute("Y", y);
        xml.write_attribute("K", k);
        xml.end_element();
    }

    // Standard RGB colors
    let standard_rgb: &[(&str, &str, &str, &str)] = &[
        ("Blue",  "0",   "0",   "255"),
        ("Green", "0",   "255", "0"),
        ("Red",   "255", "0",   "0"),
    ];
    for (name, r, g, b) in standard_rgb {
        xml.start_element("COLOR");
        xml.write_attribute("NAME", name);
        xml.write_attribute("SPACE", "RGB");
        xml.write_attribute("R", r);
        xml.write_attribute("G", g);
        xml.write_attribute("B", b);
        xml.end_element();
    }

    // Registration color
    xml.start_element("COLOR");
    xml.write_attribute("NAME", "Registration");
    xml.write_attribute("SPACE", "CMYK");
    xml.write_attribute("C", "100");
    xml.write_attribute("M", "100");
    xml.write_attribute("Y", "100");
    xml.write_attribute("K", "100");
    xml.write_attribute("Register", "1");
    xml.end_element();

    // Document-specific colors
    let skip = [
        "Black", "White", "Registration", "Blue", "Green", "Red",
        "Cyan", "Magenta", "Yellow", "Cool Black", "Rich Black", "Warm Black",
    ];
    for (name, color) in renderer.colors() {
        if skip.contains(&name.as_str()) {
            continue;
        }
        xml.start_element("COLOR");
        xml.write_attribute("NAME", name);
        xml.write_attribute("SPACE", "CMYK");
        xml.write_attribute("C", &format!("{:.4}", color.c));
        xml.write_attribute("M", &format!("{:.4}", color.m));
        xml.write_attribute("Y", &format!("{:.4}", color.y));
        xml.write_attribute("K", &format!("{:.4}", color.k));
        xml.end_element();
    }
}

/// Write all structural elements that Scribus 1.6.4 requires.
fn write_structural_elements(xml: &mut XmlWriter) {
    // HYPHEN
    xml.start_element("HYPHEN");
    xml.end_element();

    // CHARSTYLE
    xml.start_element("CHARSTYLE");
    xml.write_attribute("CNAME", "Default Character Style");
    xml.write_attribute("DefaultStyle", "1");
    xml.write_attribute("FONT", "Arial Regular");
    xml.write_attribute("FONTSIZE", "12");
    xml.write_attribute("FONTFEATURES", "");
    xml.write_attribute("FEATURES", "inherit");
    xml.write_attribute("FCOLOR", "Black");
    xml.write_attribute("FSHADE", "100");
    xml.write_attribute("HyphenWordMin", "3");
    xml.write_attribute("SCOLOR", "Black");
    xml.write_attribute("BGCOLOR", "None");
    xml.write_attribute("BGSHADE", "100");
    xml.write_attribute("SSHADE", "100");
    xml.write_attribute("TXTSHX", "5");
    xml.write_attribute("TXTSHY", "-5");
    xml.write_attribute("TXTOUT", "1");
    xml.write_attribute("TXTULP", "-0.1");
    xml.write_attribute("TXTULW", "-0.1");
    xml.write_attribute("TXTSTP", "-0.1");
    xml.write_attribute("TXTSTW", "-0.1");
    xml.write_attribute("SCALEH", "100");
    xml.write_attribute("SCALEV", "100");
    xml.write_attribute("BASEO", "0");
    xml.write_attribute("KERN", "0");
    xml.write_attribute("LANGUAGE", "en_US");
    xml.end_element();

    // STYLE
    xml.start_element("STYLE");
    xml.write_attribute("NAME", "Default Paragraph Style");
    xml.write_attribute("DefaultStyle", "1");
    xml.write_attribute("ALIGN", "0");
    xml.write_attribute("DIRECTION", "0");
    xml.write_attribute("LINESPMode", "0");
    xml.write_attribute("LINESP", "15");
    xml.write_attribute("INDENT", "0");
    xml.write_attribute("RMARGIN", "0");
    xml.write_attribute("FIRST", "0");
    xml.write_attribute("VOR", "0");
    xml.write_attribute("NACH", "0");
    xml.write_attribute("ParagraphEffectOffset", "0");
    xml.write_attribute("DROP", "0");
    xml.write_attribute("DROPLIN", "2");
    xml.write_attribute("Bullet", "0");
    xml.write_attribute("Numeration", "0");
    xml.write_attribute("HyphenConsecutiveLines", "2");
    xml.write_attribute("BCOLOR", "None");
    xml.write_attribute("BSHADE", "100");
    xml.end_element();

    // TableStyle
    xml.start_element("TableStyle");
    xml.write_attribute("NAME", "Default Table Style");
    xml.write_attribute("DefaultStyle", "1");
    xml.write_attribute("FillColor", "None");
    xml.write_attribute("FillShade", "100");
    for border in &["TableBorderLeft", "TableBorderRight", "TableBorderTop", "TableBorderBottom"] {
        xml.start_element(border);
        xml.start_element("TableBorderLine");
        xml.write_attribute("Width", "1");
        xml.write_attribute("PenStyle", "1");
        xml.write_attribute("Color", "Black");
        xml.write_attribute("Shade", "100");
        xml.end_element();
        xml.end_element();
    }
    xml.end_element();

    // CellStyle
    xml.start_element("CellStyle");
    xml.write_attribute("NAME", "Default Cell Style");
    xml.write_attribute("DefaultStyle", "1");
    xml.write_attribute("FillColor", "None");
    xml.write_attribute("FillShade", "100");
    xml.write_attribute("LeftPadding", "1");
    xml.write_attribute("RightPadding", "1");
    xml.write_attribute("TopPadding", "1");
    xml.write_attribute("BottomPadding", "1");
    for border in &["TableBorderLeft", "TableBorderRight", "TableBorderTop", "TableBorderBottom"] {
        xml.start_element(border);
        xml.start_element("TableBorderLine");
        xml.write_attribute("Width", "1");
        xml.write_attribute("PenStyle", "1");
        xml.write_attribute("Color", "Black");
        xml.write_attribute("Shade", "100");
        xml.end_element();
        xml.end_element();
    }
    xml.end_element();

    // LAYERS
    xml.start_element("LAYERS");
    xml.write_attribute("NUMMER", "0");
    xml.write_attribute("LEVEL", "0");
    xml.write_attribute("NAME", "Background");
    xml.write_attribute("SICHTBAR", "1");
    xml.write_attribute("DRUCKEN", "1");
    xml.write_attribute("EDIT", "1");
    xml.write_attribute("SELECT", "0");
    xml.write_attribute("FLOW", "1");
    xml.write_attribute("TRANS", "1");
    xml.write_attribute("BLEND", "0");
    xml.write_attribute("OUTL", "0");
    xml.write_attribute("LAYERC", "#000000");
    xml.end_element();

    // Printer
    xml.start_element("Printer");
    xml.write_attribute("firstUse", "1");
    xml.write_attribute("toFile", "0");
    xml.write_attribute("useAltPrintCommand", "0");
    xml.write_attribute("outputSeparations", "0");
    xml.write_attribute("useSpotColors", "1");
    xml.write_attribute("useColor", "1");
    xml.write_attribute("mirrorH", "0");
    xml.write_attribute("mirrorV", "0");
    xml.write_attribute("useICC", "0");
    xml.write_attribute("doGCR", "0");
    xml.write_attribute("doClip", "0");
    xml.write_attribute("setDevParam", "0");
    xml.write_attribute("useDocBleeds", "1");
    xml.write_attribute("cropMarks", "0");
    xml.write_attribute("bleedMarks", "0");
    xml.write_attribute("registrationMarks", "0");
    xml.write_attribute("colorMarks", "0");
    xml.write_attribute("includePDFMarks", "1");
    xml.write_attribute("PSLevel", "3");
    xml.write_attribute("PrintEngine", "4");
    xml.write_attribute("markLength", "20");
    xml.write_attribute("markOffset", "0");
    xml.write_attribute("BleedTop", "0");
    xml.write_attribute("BleedLeft", "0");
    xml.write_attribute("BleedRight", "0");
    xml.write_attribute("BleedBottom", "0");
    xml.write_attribute("printer", "");
    xml.write_attribute("filename", "");
    xml.write_attribute("separationName", "All");
    xml.write_attribute("printerCommand", "");
    xml.end_element();

    // PDF
    xml.start_element("PDF");
    xml.write_attribute("firstUse", "1");
    xml.write_attribute("Thumbnails", "0");
    xml.write_attribute("Articles", "0");
    xml.write_attribute("Bookmarks", "0");
    xml.write_attribute("Compress", "1");
    xml.write_attribute("CMethod", "0");
    xml.write_attribute("Quality", "0");
    xml.write_attribute("EmbedPDF", "0");
    xml.write_attribute("MirrorH", "0");
    xml.write_attribute("MirrorV", "0");
    xml.write_attribute("Clip", "0");
    xml.write_attribute("rangeSel", "0");
    xml.write_attribute("rangeTxt", "");
    xml.write_attribute("RotateDeg", "0");
    xml.write_attribute("PresentMode", "0");
    xml.write_attribute("RecalcPic", "0");
    xml.write_attribute("FontEmbedding", "0");
    xml.write_attribute("Grayscale", "0");
    xml.write_attribute("RGBMode", "1");
    xml.write_attribute("UseProfiles", "0");
    xml.write_attribute("UseProfiles2", "0");
    xml.write_attribute("Binding", "0");
    xml.write_attribute("PicRes", "300");
    xml.write_attribute("Resolution", "300");
    xml.write_attribute("Version", "14");
    xml.write_attribute("Intent", "1");
    xml.write_attribute("Intent2", "0");
    xml.write_attribute("SolidP", "");
    xml.write_attribute("ImageP", "");
    xml.write_attribute("PrintP", "");
    xml.write_attribute("InfoString", "");
    xml.write_attribute("BTop", "0");
    xml.write_attribute("BLeft", "0");
    xml.write_attribute("BRight", "0");
    xml.write_attribute("BBottom", "0");
    xml.write_attribute("useDocBleeds", "1");
    xml.write_attribute("cropMarks", "0");
    xml.write_attribute("bleedMarks", "0");
    xml.write_attribute("registrationMarks", "0");
    xml.write_attribute("colorMarks", "0");
    xml.write_attribute("docInfoMarks", "0");
    xml.write_attribute("markLength", "20");
    xml.write_attribute("markOffset", "0");
    xml.write_attribute("ImagePr", "0");
    xml.write_attribute("PassOwner", "");
    xml.write_attribute("PassUser", "");
    xml.write_attribute("Permissions", "-4");
    xml.write_attribute("Encrypt", "0");
    xml.write_attribute("UseLayers", "0");
    xml.write_attribute("UseLpi", "0");
    xml.write_attribute("UseSpotColors", "1");
    xml.write_attribute("doMultiFile", "0");
    xml.write_attribute("displayBookmarks", "0");
    xml.write_attribute("displayFullscreen", "0");
    xml.write_attribute("displayLayers", "0");
    xml.write_attribute("displayThumbs", "0");
    xml.write_attribute("hideMenuBar", "0");
    xml.write_attribute("hideToolBar", "0");
    xml.write_attribute("fitWindow", "0");
    xml.write_attribute("openAfterExport", "0");
    xml.write_attribute("PageLayout", "0");
    xml.write_attribute("openAction", "");
    for &(color, freq, angle) in &[
        ("",        "133", "45"),
        ("Black",   "133", "45"),
        ("Cyan",    "133", "105"),
        ("Magenta", "133", "75"),
        ("Yellow",  "133", "90"),
    ] {
        xml.start_element("LPI");
        xml.write_attribute("Color", color);
        xml.write_attribute("Frequency", freq);
        xml.write_attribute("Angle", angle);
        xml.write_attribute("SpotFunction", "3");
        xml.end_element();
    }
    xml.end_element(); // PDF

    // DocItemAttributes
    xml.start_element("DocItemAttributes");
    xml.end_element();

    // TablesOfContents
    xml.start_element("TablesOfContents");
    xml.end_element();

    // NotesStyles
    xml.start_element("NotesStyles");
    xml.start_element("notesStyle");
    xml.write_attribute("Name", "Default");
    xml.write_attribute("Start", "1");
    xml.write_attribute("Endnotes", "0");
    xml.write_attribute("Type", "Type_1_2_3");
    xml.write_attribute("Range", "0");
    xml.write_attribute("Prefix", "");
    xml.write_attribute("Suffix", ")");
    xml.write_attribute("AutoHeight", "1");
    xml.write_attribute("AutoWidth", "1");
    xml.write_attribute("AutoRemove", "1");
    xml.write_attribute("AutoWeld", "1");
    xml.write_attribute("SuperNote", "1");
    xml.write_attribute("SuperMaster", "1");
    xml.write_attribute("MarksStyle", "");
    xml.write_attribute("NotesStyle", "");
    xml.end_element();
    xml.end_element();

    // PageSets
    xml.start_element("PageSets");

    xml.start_element("Set");
    xml.write_attribute("Name", "Single Page");
    xml.write_attribute("FirstPage", "0");
    xml.write_attribute("Rows", "1");
    xml.write_attribute("Columns", "1");
    xml.end_element();

    xml.start_element("Set");
    xml.write_attribute("Name", "Facing Pages");
    xml.write_attribute("FirstPage", "1");
    xml.write_attribute("Rows", "1");
    xml.write_attribute("Columns", "2");
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Left Page");
    xml.end_element();
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Right Page");
    xml.end_element();
    xml.end_element();

    xml.start_element("Set");
    xml.write_attribute("Name", "3-Fold");
    xml.write_attribute("FirstPage", "0");
    xml.write_attribute("Rows", "1");
    xml.write_attribute("Columns", "3");
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Left Page");
    xml.end_element();
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Middle");
    xml.end_element();
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Right Page");
    xml.end_element();
    xml.end_element();

    xml.start_element("Set");
    xml.write_attribute("Name", "4-Fold");
    xml.write_attribute("FirstPage", "0");
    xml.write_attribute("Rows", "1");
    xml.write_attribute("Columns", "4");
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Left Page");
    xml.end_element();
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Middle Left");
    xml.end_element();
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Middle Right");
    xml.end_element();
    xml.start_element("PageNames");
    xml.write_attribute("Name", "Right Page");
    xml.end_element();
    xml.end_element();

    xml.end_element(); // PageSets

    // Sections
    xml.start_element("Sections");
    xml.start_element("Section");
    xml.write_attribute("Number", "0");
    xml.write_attribute("Name", "Section 1");
    xml.write_attribute("From", "0");
    xml.write_attribute("To", "0");
    xml.write_attribute("Type", "Type_1_2_3");
    xml.write_attribute("Start", "1");
    xml.write_attribute("Reversed", "0");
    xml.write_attribute("Active", "1");
    xml.write_attribute("FillChar", "0");
    xml.write_attribute("FieldWidth", "0");
    xml.end_element();
    xml.end_element();
}

/// Write the MASTERPAGE element.
fn write_master_page(xml: &mut XmlWriter, document: &PagedDocument) {
    let (w, h) = document
        .pages
        .first()
        .map(|p| (p.frame.width().to_pt(), p.frame.height().to_pt()))
        .unwrap_or((595.2744, 841.8888));

    let orientation = if w > h { "1" } else { "0" };

    xml.start_element("MASTERPAGE");
    xml.write_attribute("PAGEXPOS", &format!("{SCRATCH_LEFT}"));
    xml.write_attribute("PAGEYPOS", &format!("{SCRATCH_TOP}"));
    xml.write_attribute("PAGEWIDTH", &format!("{w:.4}"));
    xml.write_attribute("PAGEHEIGHT", &format!("{h:.4}"));
    xml.write_attribute("BORDERLEFT", "0");
    xml.write_attribute("BORDERRIGHT", "0");
    xml.write_attribute("BORDERTOP", "0");
    xml.write_attribute("BORDERBOTTOM", "0");
    xml.write_attribute("NUM", "0");
    xml.write_attribute("NAM", "Normal");
    xml.write_attribute("MNAM", "");
    xml.write_attribute("Size", "Custom");
    xml.write_attribute("Orientation", orientation);
    xml.write_attribute("LEFT", "0");
    xml.write_attribute("PRESET", "0");
    xml.write_attribute("VerticalGuides", "");
    xml.write_attribute("HorizontalGuides", "");
    xml.write_attribute("AGhorizontalAutoGap", "0");
    xml.write_attribute("AGverticalAutoGap", "0");
    xml.write_attribute("AGhorizontalAutoCount", "0");
    xml.write_attribute("AGverticalAutoCount", "0");
    xml.write_attribute("AGhorizontalAutoRefer", "0");
    xml.write_attribute("AGverticalAutoRefer", "0");
    xml.write_attribute("AGSelection", "0 0 0 0");
    xml.write_attribute("pageEffectDuration", "1");
    xml.write_attribute("pageViewDuration", "1");
    xml.write_attribute("effectType", "0");
    xml.write_attribute("Dm", "0");
    xml.write_attribute("M", "0");
    xml.write_attribute("Di", "0");
    xml.end_element();
}

/// Write a PAGE element with correct positioning.
fn write_page_element(xml: &mut XmlWriter, page_idx: usize, page: &Page, document: &PagedDocument) {
    let w = page.frame.width().to_pt();
    let h = page.frame.height().to_pt();
    let orientation = if w > h { "1" } else { "0" };
    let ypos = page_ypos(document, page_idx);

    xml.start_element("PAGE");
    xml.write_attribute("PAGEXPOS", &format!("{SCRATCH_LEFT}"));
    xml.write_attribute("PAGEYPOS", &format!("{ypos:.4}"));
    xml.write_attribute("PAGEWIDTH", &format!("{w:.4}"));
    xml.write_attribute("PAGEHEIGHT", &format!("{h:.4}"));
    xml.write_attribute("BORDERLEFT", "0");
    xml.write_attribute("BORDERRIGHT", "0");
    xml.write_attribute("BORDERTOP", "0");
    xml.write_attribute("BORDERBOTTOM", "0");
    xml.write_attribute("NUM", &page_idx.to_string());
    xml.write_attribute("NAM", "");
    xml.write_attribute("MNAM", "Normal");
    xml.write_attribute("Size", "Custom");
    xml.write_attribute("Orientation", orientation);
    xml.write_attribute("LEFT", "0");
    xml.write_attribute("PRESET", "0");
    xml.write_attribute("VerticalGuides", "");
    xml.write_attribute("HorizontalGuides", "");
    xml.write_attribute("AGhorizontalAutoGap", "0");
    xml.write_attribute("AGverticalAutoGap", "0");
    xml.write_attribute("AGhorizontalAutoCount", "0");
    xml.write_attribute("AGverticalAutoCount", "0");
    xml.write_attribute("AGhorizontalAutoRefer", "0");
    xml.write_attribute("AGverticalAutoRefer", "0");
    xml.write_attribute("AGSelection", "0 0 0 0");
    xml.write_attribute("pageEffectDuration", "1");
    xml.write_attribute("pageViewDuration", "1");
    xml.write_attribute("effectType", "0");
    xml.write_attribute("Dm", "0");
    xml.write_attribute("M", "0");
    xml.write_attribute("Di", "0");
    xml.end_element();
}
