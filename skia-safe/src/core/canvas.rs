use crate::prelude::*;
use crate::{gpu, Drawable, Pixmap};
use crate::{
    scalar, vertices, Bitmap, BlendMode, ClipOp, Color, Data, Font, IPoint, IRect, ISize, Image,
    ImageFilter, ImageInfo, Matrix, Paint, Path, Picture, Point, QuickReject, RRect, Rect, Region,
    Surface, SurfaceProps, TextBlob, TextEncoding, Vector, Vertices,
};
use skia_bindings::{
    C_SkAutoCanvasRestore_Construct, C_SkAutoCanvasRestore_destruct, C_SkAutoCanvasRestore_restore,
    C_SkCanvas_MakeRasterDirect, C_SkCanvas_delete, C_SkCanvas_discard,
    C_SkCanvas_getBaseLayerSize, C_SkCanvas_getGrContext, C_SkCanvas_imageInfo,
    C_SkCanvas_isClipEmpty, C_SkCanvas_makeSurface, C_SkCanvas_newEmpty, C_SkCanvas_newFromBitmap,
    C_SkCanvas_newFromBitmapAndProps, C_SkCanvas_newWidthHeightAndProps, SkAutoCanvasRestore,
    SkCanvas, SkCanvas_PointMode, SkCanvas_SaveLayerFlagsSet_kInitWithPrevious_SaveLayerFlag,
    SkCanvas_SaveLayerRec, SkCanvas_SrcRectConstraint, SkImage, SkImageFilter, SkMatrix, SkPaint,
    SkRect,
};
use std::convert::TryInto;
use std::ffi::CString;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::{mem, slice};

pub use lattice::Lattice;

bitflags! {
    pub struct SaveLayerFlags: u32 {
        const INIT_WITH_PREVIOUS = SkCanvas_SaveLayerFlagsSet_kInitWithPrevious_SaveLayerFlag as _;
    }
}

#[allow(dead_code)]
pub struct SaveLayerRec<'a> {
    // note: we _must_ store _references_ to the
    // native types here, because not all of them
    // are native transmutable, like ImageFilter or Image,
    // which are represented as ref counted pointers and
    // so we would store a reference to a pointer only.
    bounds: Option<&'a SkRect>,
    paint: Option<&'a SkPaint>,
    backdrop: Option<&'a SkImageFilter>,
    // experimental
    clip_mask: Option<&'a SkImage>,
    // experimental
    clip_matrix: Option<&'a SkMatrix>,
    flags: SaveLayerFlags,
}

impl<'a> NativeTransmutable<SkCanvas_SaveLayerRec> for SaveLayerRec<'a> {}

#[test]
fn test_save_layer_rec_layout() {
    SaveLayerRec::test_layout()
}

impl<'a> Default for SaveLayerRec<'a> {
    fn default() -> Self {
        SaveLayerRec {
            bounds: None,
            paint: None,
            backdrop: None,
            clip_mask: None,
            clip_matrix: None,
            flags: SaveLayerFlags::empty(),
        }
    }
}

impl<'a> SaveLayerRec<'a> {
    pub fn bounds(self, bounds: &'a Rect) -> Self {
        Self {
            bounds: Some(bounds.native()),
            ..self
        }
    }

    pub fn paint(self, paint: &'a Paint) -> Self {
        Self {
            paint: Some(paint.native()),
            ..self
        }
    }

    pub fn backdrop(self, backdrop: &'a ImageFilter) -> Self {
        Self {
            backdrop: Some(backdrop.native()),
            ..self
        }
    }

    pub fn clip_mask(self, clip_mask: &'a Image) -> Self {
        Self {
            clip_mask: Some(clip_mask.native()),
            ..self
        }
    }

    pub fn clip_matrix(self, clip_matrix: &'a Matrix) -> Self {
        Self {
            clip_matrix: Some(clip_matrix.native()),
            ..self
        }
    }

    pub fn flags(self, flags: SaveLayerFlags) -> Self {
        Self { flags, ..self }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum PointMode {
    Points = SkCanvas_PointMode::kPoints_PointMode as _,
    Lines = SkCanvas_PointMode::kLines_PointMode as _,
    Polygon = SkCanvas_PointMode::kPolygon_PointMode as _,
}

impl NativeTransmutable<SkCanvas_PointMode> for PointMode {}
#[test]
fn test_canvas_point_mode_layout() {
    PointMode::test_layout()
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum SrcRectConstraint {
    Strict = SkCanvas_SrcRectConstraint::kStrict_SrcRectConstraint as _,
    Fast = SkCanvas_SrcRectConstraint::kFast_SrcRectConstraint as _,
}

impl NativeTransmutable<SkCanvas_SrcRectConstraint> for SrcRectConstraint {}
#[test]
fn test_src_rect_constraint_layout() {
    SrcRectConstraint::test_layout()
}

/// Provides access to Canvas's pixels.
/// Returned by Canvas::access_top_layer_pixels()
pub struct TopLayerPixels<'a> {
    pub pixels: &'a mut [u8],
    pub info: ImageInfo,
    pub row_bytes: usize,
    pub origin: IPoint,
}

// Warning: do never access SkCanvas fields from Rust, bindgen generates a wrong layout
// as of version 0.47.3.

/// The canvas type that is returned when it is managed by another instance,
/// like Surface, for example. For these cases, the Canvas' reference that is
/// returned is bound to the lifetime of the owner.
#[repr(transparent)]
pub struct Canvas(SkCanvas);

impl NativeAccess<SkCanvas> for Canvas {
    fn native(&self) -> &SkCanvas {
        &self.0
    }

    fn native_mut(&mut self) -> &mut SkCanvas {
        &mut self.0
    }
}

/// This is the type representing a canvas that is owned and dropped
/// when it goes out of scope _and_ is bound to a the lifetime of another
/// instance. Function resolvement is done via the Deref trait.
pub struct OwnedCanvas<'lt>(*mut Canvas, PhantomData<&'lt ()>);

impl<'lt> Deref for OwnedCanvas<'lt> {
    type Target = Canvas;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<'lt> DerefMut for OwnedCanvas<'lt> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

impl<'lt> Drop for OwnedCanvas<'lt> {
    fn drop(&mut self) {
        unsafe { C_SkCanvas_delete(self.native()) }
    }
}

impl<'lt> Default for OwnedCanvas<'lt> {
    fn default() -> Self {
        let ptr = unsafe { C_SkCanvas_newEmpty() };
        Canvas::own_from_native_ptr(ptr).unwrap()
    }
}

// We implement AsMut for Canvas & OwnedCanvas
// to simplify a number of API calls.
// TODO: Should we support AsRef, too?
impl AsMut<Canvas> for Canvas {
    fn as_mut(&mut self) -> &mut Canvas {
        self
    }
}

impl<'lt> AsMut<Canvas> for OwnedCanvas<'lt> {
    fn as_mut(&mut self) -> &mut Canvas {
        self.deref_mut()
    }
}

impl Canvas {
    // TODO: Support impl Into<Option<&'a SurfaceProps>>?
    pub fn from_raster_direct<'pixels>(
        info: &ImageInfo,
        pixels: &'pixels mut [u8],
        row_bytes: impl Into<Option<usize>>,
        props: Option<&SurfaceProps>,
    ) -> Option<OwnedCanvas<'pixels>> {
        let row_bytes = row_bytes.into().unwrap_or_else(|| info.min_row_bytes());
        if row_bytes >= info.min_row_bytes() && pixels.len() >= info.compute_byte_size(row_bytes) {
            let ptr = unsafe {
                C_SkCanvas_MakeRasterDirect(
                    info.native(),
                    pixels.as_mut_ptr() as _,
                    row_bytes,
                    props.native_ptr_or_null(),
                )
            };
            Self::own_from_native_ptr(ptr)
        } else {
            None
        }
    }

    pub fn from_raster_direct_n32<'pixels>(
        size: impl Into<ISize>,
        pixels: &'pixels mut [u32],
        row_bytes: impl Into<Option<usize>>,
    ) -> Option<OwnedCanvas<'pixels>> {
        let info = ImageInfo::new_n32_premul(size, None);
        let pixels_ptr: *mut u8 = pixels.as_mut_ptr() as _;
        let pixels_u8: &'pixels mut [u8] =
            unsafe { slice::from_raw_parts_mut(pixels_ptr, pixels.elements_size_of()) };
        Self::from_raster_direct(&info, pixels_u8, row_bytes, None)
    }

    #[allow(clippy::new_ret_no_self)]
    // Decided to call this variant new, because it seems to be the simplest reasonable one.
    // TODO: Support impl Into<Option<&'a SurfaceProps>>?
    pub fn new<'lt>(
        size: impl Into<ISize>,
        props: Option<&SurfaceProps>,
    ) -> Option<OwnedCanvas<'lt>> {
        let size = size.into();
        if size.width >= 0 && size.height >= 0 {
            let ptr = unsafe {
                C_SkCanvas_newWidthHeightAndProps(
                    size.width,
                    size.height,
                    props.native_ptr_or_null(),
                )
            };
            Canvas::own_from_native_ptr(ptr)
        } else {
            None
        }
    }

    // TODO: Support impl Into<Option<&'a SurfaceProps>>?
    pub fn from_bitmap<'lt>(bitmap: &Bitmap, props: Option<&SurfaceProps>) -> OwnedCanvas<'lt> {
        let props_ptr = props.native_ptr_or_null();
        let ptr = if props_ptr.is_null() {
            unsafe { C_SkCanvas_newFromBitmap(bitmap.native()) }
        } else {
            unsafe { C_SkCanvas_newFromBitmapAndProps(bitmap.native(), props_ptr) }
        };
        Canvas::own_from_native_ptr(ptr).unwrap()
    }

    pub fn image_info(&self) -> ImageInfo {
        let mut ii = ImageInfo::default();
        unsafe { C_SkCanvas_imageInfo(self.native(), ii.native_mut()) };
        ii
    }

    pub fn props(&self) -> Option<SurfaceProps> {
        let mut sp = SurfaceProps::default();
        unsafe { self.native().getProps(sp.native_mut()) }.if_true_some(sp)
    }

    pub fn flush(&mut self) -> &mut Self {
        unsafe {
            self.native_mut().flush();
        }
        self
    }

    pub fn base_layer_size(&self) -> ISize {
        let mut size = ISize::default();
        unsafe { C_SkCanvas_getBaseLayerSize(self.native(), size.native_mut()) }
        size
    }

    // Note: implementation creates new canvas, it only takes SkSurfaceProps from &self if no props are given.
    // TODO: Support impl Into<Option<&'a SurfaceProps>>?
    pub fn new_surface(
        &mut self,
        info: &ImageInfo,
        props: Option<&SurfaceProps>,
    ) -> Option<Surface> {
        Surface::from_ptr(unsafe {
            C_SkCanvas_makeSurface(self.native_mut(), info.native(), props.native_ptr_or_null())
        })
    }

    // TODO: test ref count consistency assuming it is not increased in the native part.
    pub fn gpu_context(&mut self) -> Option<gpu::Context> {
        gpu::Context::from_unshared_ptr(unsafe { C_SkCanvas_getGrContext(self.native_mut()) })
    }

    pub fn access_top_layer_pixels(&mut self) -> Option<TopLayerPixels> {
        let mut info = ImageInfo::default();
        let mut row_bytes = 0;
        let mut origin = IPoint::default();
        let ptr = unsafe {
            self.native_mut().accessTopLayerPixels(
                info.native_mut(),
                &mut row_bytes,
                origin.native_mut(),
            )
        };
        if !ptr.is_null() {
            let size = info.compute_byte_size(row_bytes);
            let pixels = unsafe { slice::from_raw_parts_mut(ptr as _, size) };
            Some(TopLayerPixels {
                pixels,
                info,
                row_bytes,
                origin,
            })
        } else {
            None
        }
    }

    // TODO: accessTopRasterHandle()

    pub fn peek_pixels(&mut self) -> Option<Borrows<Pixmap>> {
        let mut pixmap = Pixmap::default();
        unsafe { self.native_mut().peekPixels(pixmap.native_mut()) }
            .if_true_then_some(move || pixmap.borrows(self))
    }

    #[must_use]
    pub fn read_pixels(
        &mut self,
        info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src_point: impl Into<IPoint>,
    ) -> bool {
        let src_point = src_point.into();
        let required_size = info.compute_byte_size(dst_row_bytes);
        (dst_pixels.len() >= required_size)
            && unsafe {
                self.native_mut().readPixels(
                    info.native(),
                    dst_pixels.as_mut_ptr() as _,
                    dst_row_bytes,
                    src_point.x,
                    src_point.y,
                )
            }
    }

    #[must_use]
    pub fn read_pixels_to_pixmap(&mut self, pixmap: &mut Pixmap, src: impl Into<IPoint>) -> bool {
        let src = src.into();
        unsafe { self.native_mut().readPixels1(pixmap.native(), src.x, src.y) }
    }

    #[must_use]
    pub fn read_pixels_to_bitmap(&mut self, bitmap: &mut Bitmap, src: impl Into<IPoint>) -> bool {
        let src = src.into();
        unsafe { self.native_mut().readPixels2(bitmap.native(), src.x, src.y) }
    }

    // TODO: that (pixels, row_bytes) pair is probably worth abstracting over.
    #[must_use]
    pub fn write_pixels(
        &mut self,
        info: &ImageInfo,
        pixels: &[u8],
        row_bytes: usize,
        offset: impl Into<IPoint>,
    ) -> bool {
        let offset = offset.into();
        let required_size = info.compute_byte_size(row_bytes);
        (pixels.len() >= required_size)
            && unsafe {
                self.native_mut().writePixels(
                    info.native(),
                    pixels.as_ptr() as _,
                    row_bytes,
                    offset.x,
                    offset.y,
                )
            }
    }

    #[must_use]
    pub fn write_pixels_from_bitmap(&mut self, bitmap: &Bitmap, offset: impl Into<IPoint>) -> bool {
        let offset = offset.into();
        unsafe {
            self.native_mut()
                .writePixels1(bitmap.native(), offset.x, offset.y)
        }
    }

    // The count can be read via save_count() at any time.
    pub fn save(&mut self) -> usize {
        unsafe { self.native_mut().save().try_into().unwrap() }
    }

    // Note: The save_layer(bounds, paint) variants
    // have been replaced with SaveLayerRec.
    pub fn save_layer(&mut self, layer_rec: &SaveLayerRec) -> usize {
        unsafe { self.native_mut().saveLayer2(layer_rec.native()) }
            .try_into()
            .unwrap()
    }

    pub fn restore(&mut self) -> &mut Self {
        unsafe { self.native_mut().restore() };
        self
    }

    pub fn save_count(&self) -> usize {
        unsafe { self.native().getSaveCount() }.try_into().unwrap()
    }

    pub fn restore_to_count(&mut self, count: usize) -> &mut Self {
        unsafe { self.native_mut().restoreToCount(count.try_into().unwrap()) }
        self
    }

    pub fn translate(&mut self, d: impl Into<Vector>) -> &mut Self {
        let d = d.into();
        unsafe { self.native_mut().translate(d.x, d.y) }
        self
    }

    pub fn scale(&mut self, (sx, sy): (scalar, scalar)) -> &mut Self {
        unsafe { self.native_mut().scale(sx, sy) }
        self
    }

    // impl Into<Option<Point>>?
    pub fn rotate(&mut self, degrees: scalar, point: Option<Point>) -> &mut Self {
        match point {
            Some(point) => unsafe { self.native_mut().rotate1(degrees, point.x, point.y) },
            None => unsafe { self.native_mut().rotate(degrees) },
        }
        self
    }

    pub fn skew(&mut self, (sx, sy): (scalar, scalar)) -> &mut Self {
        unsafe { self.native_mut().skew(sx, sy) }
        self
    }

    pub fn concat(&mut self, matrix: &Matrix) -> &mut Self {
        unsafe { self.native_mut().concat(matrix.native()) }
        self
    }

    pub fn set_matrix(&mut self, matrix: &Matrix) -> &mut Self {
        unsafe { self.native_mut().setMatrix(matrix.native()) }
        self
    }

    pub fn reset_matrix(&mut self) -> &mut Self {
        unsafe { self.native_mut().resetMatrix() }
        self
    }

    pub fn clip_rect(
        &mut self,
        rect: impl AsRef<Rect>,
        op: impl Into<Option<ClipOp>>,
        do_anti_alias: impl Into<Option<bool>>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().clipRect(
                rect.as_ref().native(),
                op.into().unwrap_or_default().into_native(),
                do_anti_alias.into().unwrap_or_default(),
            )
        }
        self
    }

    pub fn clip_rrect(
        &mut self,
        rrect: impl AsRef<RRect>,
        op: impl Into<Option<ClipOp>>,
        do_anti_alias: impl Into<Option<bool>>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().clipRRect(
                rrect.as_ref().native(),
                op.into().unwrap_or_default().into_native(),
                do_anti_alias.into().unwrap_or_default(),
            )
        }
        self
    }

    pub fn clip_path(
        &mut self,
        path: &Path,
        op: impl Into<Option<ClipOp>>,
        do_anti_alias: impl Into<Option<bool>>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().clipPath(
                path.native(),
                op.into().unwrap_or_default().into_native(),
                do_anti_alias.into().unwrap_or_default(),
            )
        }
        self
    }

    pub fn clip_region(&mut self, device_rgn: &Region, op: impl Into<Option<ClipOp>>) -> &mut Self {
        unsafe {
            self.native_mut().clipRegion(
                device_rgn.native(),
                op.into().unwrap_or_default().into_native(),
            )
        }
        self
    }

    // Note: quickReject() functions are implemented as a trait.

    pub fn local_clip_bounds(&self) -> Option<Rect> {
        let r = Rect::from_native(unsafe {
            // pointer versions do not link.
            self.native().getLocalClipBounds()
        });
        r.is_empty().if_false_some(r)
    }

    pub fn device_clip_bounds(&self) -> Option<IRect> {
        let r = IRect::from_native(unsafe {
            // pointer versions do not link.
            self.native().getDeviceClipBounds()
        });
        r.is_empty().if_false_some(r)
    }

    pub fn draw_color(
        &mut self,
        color: impl Into<Color>,
        mode: impl Into<Option<BlendMode>>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawColor(
                color.into().into_native(),
                mode.into().unwrap_or_default().into_native(),
            )
        }
        self
    }

    pub fn clear(&mut self, color: impl Into<Color>) -> &mut Self {
        unsafe { self.native_mut().clear(color.into().into_native()) }
        self
    }

    pub fn discard(&mut self) -> &mut Self {
        unsafe {
            // does not link:
            // self.native_mut().discard()
            C_SkCanvas_discard(self.native_mut())
        }
        self
    }

    pub fn draw_paint(&mut self, paint: &Paint) -> &mut Self {
        unsafe { self.native_mut().drawPaint(paint.native()) }
        self
    }

    pub fn draw_points(&mut self, mode: PointMode, pts: &[Point], paint: &Paint) -> &mut Self {
        unsafe {
            self.native_mut().drawPoints(
                mode.into_native(),
                pts.len(),
                pts.native().as_ptr(),
                paint.native(),
            )
        }
        self
    }

    pub fn draw_point(&mut self, p: impl Into<Point>, paint: &Paint) -> &mut Self {
        let p = p.into();
        unsafe { self.native_mut().drawPoint(p.x, p.y, paint.native()) }
        self
    }

    pub fn draw_line(
        &mut self,
        p1: impl Into<Point>,
        p2: impl Into<Point>,
        paint: &Paint,
    ) -> &mut Self {
        let (p1, p2) = (p1.into(), p2.into());
        unsafe {
            self.native_mut()
                .drawLine(p1.x, p1.y, p2.x, p2.y, paint.native())
        }
        self
    }

    pub fn draw_rect(&mut self, rect: impl AsRef<Rect>, paint: &Paint) -> &mut Self {
        unsafe {
            self.native_mut()
                .drawRect(rect.as_ref().native(), paint.native())
        }
        self
    }

    pub fn draw_irect(&mut self, rect: impl AsRef<IRect>, paint: &Paint) -> &mut Self {
        unsafe {
            self.native_mut()
                .drawIRect(rect.as_ref().native(), paint.native())
        }
        self
    }

    pub fn draw_region(&mut self, region: &Region, paint: &Paint) -> &mut Self {
        unsafe {
            self.native_mut()
                .drawRegion(region.native(), paint.native())
        }
        self
    }

    pub fn draw_oval(&mut self, oval: impl AsRef<Rect>, paint: &Paint) -> &mut Self {
        unsafe {
            self.native_mut()
                .drawOval(oval.as_ref().native(), paint.native())
        }
        self
    }

    pub fn draw_rrect(&mut self, rrect: impl AsRef<RRect>, paint: &Paint) -> &mut Self {
        unsafe {
            self.native_mut()
                .drawRRect(rrect.as_ref().native(), paint.native())
        }
        self
    }

    pub fn draw_drrect(
        &mut self,
        outer: impl AsRef<RRect>,
        inner: impl AsRef<RRect>,
        paint: &Paint,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawDRRect(
                outer.as_ref().native(),
                inner.as_ref().native(),
                paint.native(),
            )
        }
        self
    }

    pub fn draw_circle(
        &mut self,
        center: impl Into<Point>,
        radius: scalar,
        paint: &Paint,
    ) -> &mut Self {
        let center = center.into();
        unsafe {
            // does not link:
            // self.native_mut().drawCircle1(center.into_native(), radius, paint.native())
            self.native_mut()
                .drawCircle(center.x, center.y, radius, paint.native())
        }
        self
    }

    pub fn draw_arc(
        &mut self,
        oval: impl AsRef<Rect>,
        start_angle: scalar,
        sweep_angle: scalar,
        use_center: bool,
        paint: &Paint,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawArc(
                oval.as_ref().native(),
                start_angle,
                sweep_angle,
                use_center,
                paint.native(),
            )
        }
        self
    }

    pub fn draw_round_rect(
        &mut self,
        rect: impl AsRef<Rect>,
        rx: scalar,
        ry: scalar,
        paint: &Paint,
    ) -> &mut Self {
        unsafe {
            self.native_mut()
                .drawRoundRect(rect.as_ref().native(), rx, ry, paint.native())
        }
        self
    }

    pub fn draw_path(&mut self, path: &Path, paint: &Paint) -> &mut Self {
        unsafe { self.native_mut().drawPath(path.native(), paint.native()) }
        self
    }

    pub fn draw_image(
        &mut self,
        image: &Image,
        left_top: impl Into<Point>,
        paint: Option<&Paint>,
    ) -> &mut Self {
        let left_top = left_top.into();
        unsafe {
            self.native_mut().drawImage(
                image.native(),
                left_top.x,
                left_top.y,
                paint.native_ptr_or_null(),
            )
        }
        self
    }

    pub fn draw_image_rect(
        &mut self,
        image: &Image,
        src: Option<(&Rect, SrcRectConstraint)>,
        dst: impl AsRef<Rect>,
        paint: &Paint,
    ) -> &mut Self {
        match src {
            Some((src, constraint)) => unsafe {
                self.native_mut().drawImageRect(
                    image.native(),
                    src.native(),
                    dst.as_ref().native(),
                    paint.native(),
                    constraint.into_native(),
                )
            },
            None => unsafe {
                self.native_mut().drawImageRect2(
                    image.native(),
                    dst.as_ref().native(),
                    paint.native(),
                )
            },
        }
        self
    }

    pub fn draw_image_nine(
        &mut self,
        image: &Image,
        center: impl AsRef<IRect>,
        dst: impl AsRef<Rect>,
        paint: Option<&Paint>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawImageNine(
                image.native(),
                center.as_ref().native(),
                dst.as_ref().native(),
                paint.native_ptr_or_null(),
            )
        }
        self
    }

    pub fn draw_bitmap(
        &mut self,
        bitmap: &Bitmap,
        left_top: impl Into<Point>,
        paint: Option<&Paint>,
    ) -> &mut Self {
        let left_top = left_top.into();
        unsafe {
            self.native_mut().drawBitmap(
                bitmap.native(),
                left_top.x,
                left_top.y,
                paint.native_ptr_or_null(),
            )
        }
        self
    }

    pub fn draw_bitmap_rect(
        &mut self,
        bitmap: &Bitmap,
        src: Option<&Rect>,
        dst: impl AsRef<Rect>,
        paint: &Paint,
        constraint: impl Into<Option<SrcRectConstraint>>,
    ) -> &mut Self {
        let constraint = constraint.into().unwrap_or(SrcRectConstraint::Strict);
        match src {
            Some(src) => unsafe {
                self.native_mut().drawBitmapRect(
                    bitmap.native(),
                    src.as_ref().native(),
                    dst.as_ref().native(),
                    paint.native(),
                    constraint.into_native(),
                )
            },
            None => unsafe {
                self.native_mut().drawBitmapRect2(
                    bitmap.native(),
                    dst.as_ref().native(),
                    paint.native(),
                    constraint.into_native(),
                )
            },
        }
        self
    }

    pub fn draw_bitmap_nine(
        &mut self,
        bitmap: &Bitmap,
        center: impl AsRef<IRect>,
        dst: impl AsRef<Rect>,
        paint: Option<&Paint>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawBitmapNine(
                bitmap.native(),
                center.as_ref().native(),
                dst.as_ref().native(),
                paint.native_ptr_or_null(),
            )
        }
        self
    }

    pub fn draw_bitmap_lattice(
        &mut self,
        bitmap: &Bitmap,
        lattice: &Lattice,
        dst: impl AsRef<Rect>,
        paint: Option<&Paint>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawBitmapLattice(
                bitmap.native(),
                &lattice.native().native,
                dst.as_ref().native(),
                paint.native_ptr_or_null(),
            )
        }
        self
    }

    pub fn draw_image_lattice(
        &mut self,
        image: &Image,
        lattice: &Lattice,
        dst: impl AsRef<Rect>,
        paint: Option<&Paint>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawImageLattice(
                image.native(),
                &lattice.native().native,
                dst.as_ref().native(),
                paint.native_ptr_or_null(),
            )
        }
        self
    }

    // TODO: drawSimpleText?

    // rust specific, based on drawSimpleText with fixed UTF8 encoding,
    // implementation is similar to Font's *_str methods.
    pub fn draw_str(
        &mut self,
        str: impl AsRef<str>,
        origin: impl Into<Point>,
        font: &Font,
        paint: &Paint,
    ) -> &mut Self {
        let origin = origin.into();
        let bytes = str.as_ref().as_bytes();
        unsafe {
            self.native_mut().drawSimpleText(
                bytes.as_ptr() as _,
                bytes.len(),
                TextEncoding::UTF8.into_native(),
                origin.x,
                origin.y,
                font.native(),
                paint.native(),
            )
        }
        self
    }

    pub fn draw_text_blob(&mut self, blob: &TextBlob, origin: impl Into<Point>, paint: &Paint) {
        let origin = origin.into();
        unsafe {
            self.native_mut()
                .drawTextBlob(blob.native(), origin.x, origin.y, paint.native())
        }
    }

    pub fn draw_picture(
        &mut self,
        picture: &Picture,
        matrix: Option<&Matrix>,
        paint: Option<&Paint>,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawPicture2(
                picture.native(),
                matrix.native_ptr_or_null(),
                paint.native_ptr_or_null(),
            )
        }
        self
    }

    pub fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        bones: Option<&[vertices::Bone]>,
        mode: BlendMode,
        paint: &Paint,
    ) -> &mut Self {
        match bones {
            Some(bones) => unsafe {
                self.native_mut().drawVertices2(
                    vertices.native(),
                    bones.native().as_ptr(),
                    bones.len().try_into().unwrap(),
                    mode.into_native(),
                    paint.native(),
                )
            },
            None => unsafe {
                self.native_mut().drawVertices(
                    vertices.native(),
                    mode.into_native(),
                    paint.native(),
                )
            },
        }
        self
    }

    pub fn draw_patch(
        &mut self,
        cubics: &[Point; 12],
        colors: &[Color; 4],
        tex_coords: &[Point; 4],
        mode: impl Into<Option<BlendMode>>,
        paint: &Paint,
    ) -> &mut Self {
        unsafe {
            self.native_mut().drawPatch(
                cubics.native().as_ptr(),
                colors.native().as_ptr(),
                tex_coords.native().as_ptr(),
                mode.into().unwrap_or(BlendMode::Modulate).into_native(),
                paint.native(),
            )
        }
        self
    }

    // TODO: drawAtlas

    pub fn draw_drawable(&mut self, drawable: &mut Drawable, matrix: Option<&Matrix>) {
        unsafe {
            self.native_mut()
                .drawDrawable(drawable.native_mut(), matrix.native_ptr_or_null())
        }
    }

    pub fn draw_drawable_at(&mut self, drawable: &mut Drawable, offset: impl Into<Point>) {
        let offset = offset.into();
        unsafe {
            self.native_mut()
                .drawDrawable1(drawable.native_mut(), offset.x, offset.y)
        }
    }

    pub fn draw_annotation(
        &mut self,
        rect: impl AsRef<Rect>,
        key: &str,
        value: &Data,
    ) -> &mut Self {
        let key = CString::new(key).unwrap();
        unsafe {
            self.native_mut().drawAnnotation(
                rect.as_ref().native(),
                key.as_ptr(),
                value.native_mut_force(),
            )
        }
        self
    }

    pub fn is_clip_empty(&self) -> bool {
        unsafe { C_SkCanvas_isClipEmpty(self.native()) }
    }

    pub fn is_clip_rect(&self) -> bool {
        unsafe { C_SkCanvas_isClipEmpty(self.native()) }
    }

    pub fn total_matrix(&self) -> &Matrix {
        Matrix::from_native_ref(unsafe { &*self.native().getTotalMatrix() })
    }

    //
    // internal helper
    //

    pub(crate) fn own_from_native_ptr<'lt>(native: *mut SkCanvas) -> Option<OwnedCanvas<'lt>> {
        if !native.is_null() {
            Some(OwnedCanvas::<'lt>(
                Self::borrow_from_native(unsafe { &mut *native }),
                PhantomData,
            ))
        } else {
            None
        }
    }

    pub(crate) fn borrow_from_native(native: &mut SkCanvas) -> &mut Self {
        unsafe { transmute_ref_mut(native) }
    }
}

impl QuickReject<Rect> for Canvas {
    fn quick_reject(&self, other: &Rect) -> bool {
        unsafe { self.native().quickReject(other.native()) }
    }
}

impl QuickReject<Path> for Canvas {
    fn quick_reject(&self, other: &Path) -> bool {
        unsafe { self.native().quickReject1(other.native()) }
    }
}

//
// Lattice
//

pub mod lattice {
    use crate::prelude::*;
    use crate::{Color, IRect};
    use skia_bindings::{SkCanvas_Lattice, SkCanvas_Lattice_RectType};
    use std::marker::PhantomData;

    #[derive(Debug)]
    pub struct Lattice<'a> {
        pub x_divs: &'a [i32],
        pub y_divs: &'a [i32],
        pub rect_types: Option<&'a [RectType]>,
        pub bounds: Option<IRect>,
        pub colors: Option<&'a [Color]>,
    }

    pub(crate) struct Ref<'a> {
        pub native: SkCanvas_Lattice,
        pd: PhantomData<&'a Lattice<'a>>,
    }

    impl<'a> Lattice<'a> {
        pub(crate) fn native(&self) -> Ref {
            if let Some(rect_types) = self.rect_types {
                let rect_count = (self.x_divs.len() + 1) * (self.y_divs.len() + 1);
                assert_eq!(rect_count, rect_types.len());
                // even though rect types may not include any FixedColor refs,
                // we expect the colors slice with a proper size here, this
                // saves us for going over the types array and looking for FixedColor
                // entries.
                assert_eq!(rect_count, self.colors.unwrap().len());
            }

            let native = SkCanvas_Lattice {
                fXDivs: self.x_divs.as_ptr(),
                fYDivs: self.y_divs.as_ptr(),
                fRectTypes: self.rect_types.native().as_ptr_or_null(),
                fXCount: self.x_divs.len().try_into().unwrap(),
                fYCount: self.y_divs.len().try_into().unwrap(),
                fBounds: self.bounds.native().as_ptr_or_null(),
                fColors: self.colors.native().as_ptr_or_null(),
            };
            Ref {
                native,
                pd: PhantomData,
            }
        }
    }

    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    #[repr(u8)]
    pub enum RectType {
        Default = SkCanvas_Lattice_RectType::kDefault as _,
        Transparent = SkCanvas_Lattice_RectType::kTransparent as _,
        FixedColor = SkCanvas_Lattice_RectType::kFixedColor as _,
    }

    impl NativeTransmutable<SkCanvas_Lattice_RectType> for RectType {}
    #[test]
    fn test_lattice_rect_type_layout() {
        RectType::test_layout();
    }

    impl Default for RectType {
        fn default() -> Self {
            RectType::Default
        }
    }
}

//
// AutoRestoredCanvas
//

/// A reference to a Canvas that restores the Canvas's state when
/// it's being dropped.
pub struct AutoRestoredCanvas<'a> {
    canvas: &'a mut Canvas,
    restore: SkAutoCanvasRestore,
}

impl<'a> Deref for AutoRestoredCanvas<'a> {
    type Target = Canvas;
    fn deref(&self) -> &Self::Target {
        self.canvas
    }
}

impl<'a> DerefMut for AutoRestoredCanvas<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.canvas
    }
}

impl<'a> NativeAccess<SkAutoCanvasRestore> for AutoRestoredCanvas<'a> {
    fn native(&self) -> &SkAutoCanvasRestore {
        &self.restore
    }

    fn native_mut(&mut self) -> &mut SkAutoCanvasRestore {
        &mut self.restore
    }
}

impl<'a> Drop for AutoRestoredCanvas<'a> {
    fn drop(&mut self) {
        unsafe { C_SkAutoCanvasRestore_destruct(self.native_mut()) }
    }
}

impl<'a> AutoRestoredCanvas<'a> {
    pub fn restore(&mut self) {
        unsafe {
            // does not link:
            // self.native_mut().restore()
            C_SkAutoCanvasRestore_restore(self.native_mut())
        }
    }
}

pub enum AutoCanvasRestore {}

impl AutoCanvasRestore {
    // TODO: rename to save(), add a method to Canvas, perhaps named auto_restored()?
    // Note: Can't use AsMut here for the canvas, because it would break
    //       the lifetime dependency.
    pub fn guard(canvas: &mut Canvas, do_save: bool) -> AutoRestoredCanvas {
        let restore = unsafe {
            // does not link on Linux
            // SkAutoCanvasRestore::new(canvas.native_mut(), do_save)
            let mut acr: SkAutoCanvasRestore = mem::zeroed();
            C_SkAutoCanvasRestore_Construct(&mut acr, canvas.native_mut(), do_save);
            acr
        };

        AutoRestoredCanvas { canvas, restore }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AlphaType, Canvas, ClipOp, Color, ColorType, ImageInfo, Matrix, OwnedCanvas, Rect,
        SaveLayerRec,
    };

    #[test]
    fn test_raster_direct_creation_and_clear_in_memory() {
        let info = ImageInfo::new((2, 2), ColorType::RGBA8888, AlphaType::Unpremul, None);
        assert_eq!(8, info.min_row_bytes());
        let mut bytes: [u8; 8 * 2] = Default::default();
        {
            let mut canvas = Canvas::from_raster_direct(&info, bytes.as_mut(), None, None).unwrap();
            canvas.clear(Color::RED);
        }

        assert_eq!(0xff, bytes[0]);
        assert_eq!(0x00, bytes[1]);
        assert_eq!(0x00, bytes[2]);
        assert_eq!(0xff, bytes[3]);
    }

    #[test]
    fn test_raster_direct_n32_creation_and_clear_in_memory() {
        let mut pixels: [u32; 4] = Default::default();
        {
            let mut canvas = Canvas::from_raster_direct_n32((2, 2), pixels.as_mut(), None).unwrap();
            canvas.clear(Color::RED);
        }

        // TODO: equals to 0xff0000ff on macOS, but why? Endianess should be the same.
        // assert_eq!(0xffff0000, pixels[0]);
    }

    #[test]
    fn test_empty_canvas_creation() {
        let canvas = OwnedCanvas::default();
        drop(canvas)
    }

    #[test]
    fn test_save_layer_rec_lifetimes() {
        let rect = Rect::default();
        {
            let matrix = Matrix::default();

            let _rec = SaveLayerRec::default().clip_matrix(&matrix).bounds(&rect);
        }
    }

    #[test]
    fn test_total_matrix_transmutation() {
        let mut c = Canvas::new((2, 2), None).unwrap();
        let matrix_ref = c.total_matrix();
        assert_eq!(Matrix::default(), *matrix_ref);
        c.rotate(0.1, None);
        let matrix_ref = c.total_matrix();
        assert_ne!(Matrix::default(), *matrix_ref);
    }

    #[test]
    fn test_make_surface() {
        let mut pixels: [u32; 4] = Default::default();
        let mut canvas = Canvas::from_raster_direct_n32((2, 2), pixels.as_mut(), None).unwrap();
        let ii = canvas.image_info();
        let mut surface = canvas.new_surface(&ii, None).unwrap();
        dbg!(&mut canvas as *mut _);
        drop(canvas);

        let canvas = surface.canvas();
        dbg!(canvas as *mut _);
        canvas.clear(Color::RED);
    }

    #[test]
    fn clip_options_overloads() {
        let mut c = OwnedCanvas::default();
        // do_anti_alias
        c.clip_rect(Rect::default(), None, true);
        // clip_op
        c.clip_rect(Rect::default(), ClipOp::Difference, None);
        // both
        c.clip_rect(Rect::default(), ClipOp::Difference, true);
    }
}
