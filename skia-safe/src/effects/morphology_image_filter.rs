use crate::image_filter::CropRect;
use crate::prelude::*;
use skia_bindings::SkImageFilter;

impl RCHandle<SkImageFilter> {
    pub fn dilate<'a>(
        &self,
        crop_rect: impl Into<Option<&'a CropRect>>,
        radii: (i32, i32),
    ) -> Option<Self> {
        dilate_image_filter::new(radii, self, crop_rect)
    }

    pub fn erode<'a>(
        &self,
        crop_rect: impl Into<Option<&'a CropRect>>,
        radii: (i32, i32),
    ) -> Option<Self> {
        erode_image_filter::new(radii, self, crop_rect)
    }
}

pub mod dilate_image_filter {
    use crate::image_filter::CropRect;
    use crate::prelude::*;
    use crate::ImageFilter;
    use skia_bindings::C_SkDilateImageFilter_Make;

    pub fn new<'a>(
        (radius_x, radius_y): (i32, i32),
        input: &ImageFilter,
        crop_rect: impl Into<Option<&'a CropRect>>,
    ) -> Option<ImageFilter> {
        ImageFilter::from_ptr(unsafe {
            C_SkDilateImageFilter_Make(
                radius_x,
                radius_y,
                input.shared_native(),
                crop_rect.into().native_ptr_or_null(),
            )
        })
    }
}

pub mod erode_image_filter {
    use crate::image_filter::CropRect;
    use crate::prelude::NativePointerOrNull2;
    use crate::ImageFilter;
    use skia_bindings::C_SkErodeImageFilter_Make;

    pub fn new<'a>(
        (radius_x, radius_y): (i32, i32),
        input: &ImageFilter,
        crop_rect: impl Into<Option<&'a CropRect>>,
    ) -> Option<ImageFilter> {
        ImageFilter::from_ptr(unsafe {
            C_SkErodeImageFilter_Make(
                radius_x,
                radius_y,
                input.shared_native(),
                crop_rect.into().native_ptr_or_null(),
            )
        })
    }
}
