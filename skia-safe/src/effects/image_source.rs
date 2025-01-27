use crate::prelude::*;
use crate::{FilterQuality, Image, ImageFilter, Rect};
use skia_bindings::{C_SkImageSource_Make, C_SkImageSource_Make2, SkImage, SkImageFilter};

impl RCHandle<SkImageFilter> {
    pub fn from_image(image: &Image) -> Option<Self> {
        from_image(image)
    }

    pub fn from_image_rect(
        image: &Image,
        src_rect: impl AsRef<Rect>,
        dst_rect: impl AsRef<Rect>,
        filter_quality: FilterQuality,
    ) -> Option<Self> {
        from_image_rect(image, src_rect, dst_rect, filter_quality)
    }
}

impl RCHandle<SkImage> {
    pub fn as_filter(&self) -> Option<ImageFilter> {
        from_image(self)
    }

    pub fn as_filter_rect(
        &self,
        src_rect: impl AsRef<Rect>,
        dst_rect: impl AsRef<Rect>,
        filter_quality: FilterQuality,
    ) -> Option<ImageFilter> {
        from_image_rect(self, src_rect, dst_rect, filter_quality)
    }
}

pub fn from_image(image: &Image) -> Option<ImageFilter> {
    ImageFilter::from_ptr(unsafe { C_SkImageSource_Make(image.shared_native()) })
}

pub fn from_image_rect(
    image: &Image,
    src_rect: impl AsRef<Rect>,
    dst_rect: impl AsRef<Rect>,
    filter_quality: FilterQuality,
) -> Option<ImageFilter> {
    ImageFilter::from_ptr(unsafe {
        C_SkImageSource_Make2(
            image.shared_native(),
            src_rect.as_ref().native(),
            dst_rect.as_ref().native(),
            filter_quality.into_native(),
        )
    })
}
