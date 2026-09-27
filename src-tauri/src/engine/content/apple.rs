//! PDFs through PDFKit, and pictures through Vision's text recognition, both
//! on the Mac. A PDF page with almost no text of its own (a scan) is drawn
//! and read like a picture.

use std::path::Path;
use std::ptr;

use objc2::rc::{autoreleasepool, Retained};
use objc2::AllocAnyThread;
use objc2_core_foundation::{
    CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGPoint, CGRect, CGSize, CFURL,
};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGImage,
    CGImageAlphaInfo,
};
use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
use objc2_image_io::{
    kCGImageSourceCreateThumbnailFromImageAlways, kCGImageSourceCreateThumbnailWithTransform,
    kCGImageSourceThumbnailMaxPixelSize, CGImageSource,
};
use objc2_pdf_kit::{PDFDisplayBox, PDFDocument, PDFPage};
use objc2_vision::{
    VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
};

use super::{Collect, Text};

/// A page with fewer letters than this is taken for a scan.
const SCANNED: usize = 20;
/// Text recognition takes about a second a page: read at most this many.
const MAX_RECOGNISED_PAGES: usize = 20;
/// Pages are drawn at twice their size (144 dpi), for recognition.
const SCALE: f64 = 2.0;
/// No side of a drawn page is bigger than this.
const MAX_SIDE: f64 = 4000.0;

fn url(path: &Path) -> Option<Retained<NSURL>> {
    Some(NSURL::fileURLWithPath(&NSString::from_str(path.to_str()?)))
}

pub fn pdf(path: &Path) -> Option<Text> {
    autoreleasepool(|_| {
        let url = url(path)?;
        let doc = unsafe { PDFDocument::initWithURL(PDFDocument::alloc(), &url) }?;
        if unsafe { doc.isLocked() } {
            return None;
        }
        let mut out = Collect::default();
        let mut recognised = 0;
        for i in 0..unsafe { doc.pageCount() } {
            if out.full() {
                break;
            }
            let Some(page) = (unsafe { doc.pageAtIndex(i) }) else {
                continue;
            };
            let own = unsafe { page.string() }
                .map(|s| s.to_string())
                .unwrap_or_default();
            let letters = own.chars().filter(|c| !c.is_whitespace()).count();
            if letters >= SCANNED {
                out.push(&own);
            } else if recognised < MAX_RECOGNISED_PAGES {
                recognised += 1;
                if let Some(text) = drawn(&page).and_then(|image| drawn_text(&image)) {
                    out.push(&text);
                }
            }
            out.gap();
        }
        Some(Text {
            text: out.0,
            cut: false,
            recognised: recognised > 0,
        })
    })
}

/// The page drawn on white, for text recognition.
fn drawn(page: &PDFPage) -> Option<CFRetained<CGImage>> {
    let bounds = unsafe { page.boundsForBox(PDFDisplayBox::MediaBox) };
    let scale = SCALE.min(MAX_SIDE / bounds.size.width.max(bounds.size.height).max(1.0));
    let ctx = white(
        (bounds.size.width * scale).round() as usize,
        (bounds.size.height * scale).round() as usize,
    )?;
    CGContext::scale_ctm(Some(&ctx), scale, scale);
    unsafe { page.drawWithBox_toContext(PDFDisplayBox::MediaBox, &ctx) };
    CGBitmapContextCreateImage(Some(&ctx))
}

/// A blank white canvas `w` by `h` pixels.
fn white(w: usize, h: usize) -> Option<CFRetained<CGContext>> {
    if w == 0 || h == 0 {
        return None;
    }
    let space = CGColorSpace::new_device_rgb()?;
    let ctx = unsafe {
        CGBitmapContextCreate(
            ptr::null_mut(),
            w,
            h,
            8,
            0,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }?;
    CGContext::set_rgb_fill_color(Some(&ctx), 1.0, 1.0, 1.0, 1.0);
    CGContext::fill_rect(Some(&ctx), rect(w, h));
    Some(ctx)
}

fn rect(w: usize, h: usize) -> CGRect {
    CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w as f64, h as f64))
}

fn drawn_text(image: &CGImage) -> Option<String> {
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_options(
            VNImageRequestHandler::alloc(),
            image,
            &NSDictionary::new(),
        )
    };
    recognise(&handler)
}

/// The text in a picture file: turned upright by its orientation, no bigger
/// than `MAX_SIDE`, and put on white, since dark text on a transparent
/// background reads as nothing.
pub fn picture(path: &Path) -> Option<String> {
    autoreleasepool(|_| {
        let url = CFURL::from_file_path(path)?;
        let source = unsafe { CGImageSource::with_url(&url, None) }?;
        let yes: &CFType = CFBoolean::new(true).as_ref();
        let max = CFNumber::new_i32(MAX_SIDE as i32);
        let keys: [&CFString; 3] = unsafe {
            [
                kCGImageSourceCreateThumbnailFromImageAlways,
                kCGImageSourceCreateThumbnailWithTransform,
                kCGImageSourceThumbnailMaxPixelSize,
            ]
        };
        let options =
            CFDictionary::<CFString, CFType>::from_slices(&keys, &[yes, yes, max.as_ref()]);
        let image = unsafe { source.thumbnail_at_index(0, Some(options.as_opaque())) }?;
        let (w, h) = (CGImage::width(Some(&image)), CGImage::height(Some(&image)));
        let ctx = white(w, h)?;
        CGContext::draw_image(Some(&ctx), rect(w, h), Some(&image));
        let flat = CGBitmapContextCreateImage(Some(&ctx))?;
        drawn_text(&flat)
    })
}

/// Every line of text Vision finds, top to bottom.
fn recognise(handler: &VNImageRequestHandler) -> Option<String> {
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    request.setUsesLanguageCorrection(true);
    let as_request: Retained<VNRequest> =
        Retained::into_super(Retained::into_super(request.clone()));
    handler
        .performRequests_error(&NSArray::from_retained_slice(&[as_request]))
        .ok()?;
    let mut lines: Vec<(f64, f64, String)> = request
        .results()?
        .iter()
        .filter_map(|seen| {
            let best = seen.topCandidates(1).firstObject()?;
            let bounds = unsafe { seen.boundingBox() };
            // Vision's origin is the bottom left.
            Some((
                -(bounds.origin.y + bounds.size.height),
                bounds.origin.x,
                best.string().to_string(),
            ))
        })
        .collect();
    lines.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    Some(
        lines
            .into_iter()
            .map(|(_, _, line)| line)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
