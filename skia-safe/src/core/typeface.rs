use crate::interop::{MemoryStream, NativeStreamBase};
use crate::prelude::*;
use crate::{font_arguments, interop, FontArguments};
use crate::{font_parameters::VariationAxis, Data, FontStyle, GlyphId, Rect, Unichar};
use skia_bindings::{
    C_SkTypeface_LocalizedStrings_next, C_SkTypeface_LocalizedStrings_unref,
    C_SkTypeface_MakeDefault, C_SkTypeface_MakeDeserialize, C_SkTypeface_MakeFromData,
    C_SkTypeface_MakeFromName, C_SkTypeface_isBold, C_SkTypeface_isItalic, C_SkTypeface_makeClone,
    C_SkTypeface_serialize, SkRefCntBase, SkTypeface, SkTypeface_LocalizedStrings,
    SkTypeface_SerializeBehavior,
};
use std::{ffi, ptr};

pub type FontId = skia_bindings::SkFontID;
pub type FontTableTag = skia_bindings::SkFontTableTag;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum SerializeBehavior {
    DoIncludeData = SkTypeface_SerializeBehavior::kDoIncludeData as _,
    DontIncludeData = SkTypeface_SerializeBehavior::kDontIncludeData as _,
    IncludeDataIfLocal = SkTypeface_SerializeBehavior::kIncludeDataIfLocal as _,
}

impl NativeTransmutable<SkTypeface_SerializeBehavior> for SerializeBehavior {}
#[test]
fn test_typeface_serialize_behavior_layout() {
    SerializeBehavior::test_layout()
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocalizedString {
    pub string: String,
    pub language: String,
}

pub type Typeface = RCHandle<SkTypeface>;

impl NativeRefCountedBase for SkTypeface {
    type Base = SkRefCntBase;

    fn ref_counted_base(&self) -> &Self::Base {
        &self._base._base._base
    }
}

impl Default for RCHandle<SkTypeface> {
    fn default() -> Self {
        Typeface::from_ptr(unsafe { C_SkTypeface_MakeDefault() }).unwrap()
    }
}

impl RCHandle<SkTypeface> {
    pub fn font_style(&self) -> FontStyle {
        unsafe { FontStyle::from_native(self.native().fontStyle()) }
    }

    pub fn is_bold(&self) -> bool {
        // does not link:
        // unsafe { self.native().isBold() }
        unsafe { C_SkTypeface_isBold(self.native()) }
    }

    pub fn is_italic(&self) -> bool {
        // does not link:
        // unsafe { self.native().isItalic() }
        unsafe { C_SkTypeface_isItalic(self.native()) }
    }

    pub fn is_fixed_pitch(&self) -> bool {
        unsafe { self.native().isFixedPitch() }
    }

    pub fn variation_design_position(
        &self,
    ) -> Option<Vec<font_arguments::variation_position::Coordinate>> {
        unsafe {
            let r = self.native().getVariationDesignPosition(ptr::null_mut(), 0);
            if r != -1 {
                let mut v = vec![font_arguments::variation_position::Coordinate::default(); r as _];
                let elements = self
                    .native()
                    .getVariationDesignPosition(v.native_mut().as_mut_ptr(), r);
                assert_eq!(elements, r);
                Some(v)
            } else {
                None
            }
        }
    }

    pub fn variation_design_parameters(&self) -> Option<Vec<VariationAxis>> {
        unsafe {
            let r = self
                .native()
                .getVariationDesignParameters(ptr::null_mut(), 0);
            if r != -1 {
                let mut v = vec![VariationAxis::default(); r as _];
                let elements = self
                    .native()
                    .getVariationDesignParameters(v.native_mut().as_mut_ptr(), r);
                assert_eq!(elements, r);
                Some(v)
            } else {
                None
            }
        }
    }

    pub fn unique_id(self) -> FontId {
        unsafe { self.native().uniqueID() }
    }

    // TODO: wrap SkTypeface::UniqueID()?

    // Decided not to support PartialEq instead of this function,
    // because Skia does not support the operator ==.
    pub fn equal(face_a: &Typeface, face_b: &Typeface) -> bool {
        unsafe { SkTypeface::Equal(face_a.native(), face_b.native()) }
    }

    pub fn from_name(familiy_name: &str, font_style: FontStyle) -> Option<Typeface> {
        let familiy_name = ffi::CString::new(familiy_name).ok()?;
        Typeface::from_ptr(unsafe {
            C_SkTypeface_MakeFromName(familiy_name.as_ptr(), *font_style.native())
        })
    }

    // from_file is unsupported, because it is unclear what the
    // encoding of the path name is. from_data can be used instead.

    // TODO: MakeFromStream()?

    pub fn from_data(data: &Data, index: impl Into<Option<usize>>) {
        Typeface::from_ptr(unsafe {
            C_SkTypeface_MakeFromData(
                data.shared_native(),
                index.into().unwrap_or_default().try_into().unwrap(),
            )
        });
    }

    pub fn clone_with_arguments(&self, arguments: &FontArguments) -> Option<Typeface> {
        Typeface::from_ptr(unsafe { C_SkTypeface_makeClone(self.native(), arguments.native()) })
    }

    // TODO: serialize(Write)?

    // TODO: return Data as impl Deref<[u8]> / Borrow<[u8]> here?
    pub fn serialize(&self, behavior: SerializeBehavior) -> Data {
        Data::from_ptr(unsafe { C_SkTypeface_serialize(self.native(), behavior.into_native()) })
            .unwrap()
    }

    // TODO: Deserialize(Read?)

    pub fn deserialize(data: &[u8]) -> Option<Typeface> {
        let mut stream = MemoryStream::from_bytes(data);
        Typeface::from_ptr(unsafe {
            C_SkTypeface_MakeDeserialize(stream.native_mut().as_stream_mut())
        })
    }

    pub fn unichars_to_glyphs(&self, uni: &[Unichar], glyphs: &mut [GlyphId]) {
        assert_eq!(uni.len(), glyphs.len());
        unsafe {
            self.native().unicharsToGlyphs(
                uni.as_ptr(),
                uni.len().try_into().unwrap(),
                glyphs.as_mut_ptr(),
            )
        }
    }

    pub fn unichar_to_glyph(&self, unichar: Unichar) -> GlyphId {
        unsafe { self.native().unicharToGlyph(unichar) }
    }

    pub fn count_glyphs(&self) -> usize {
        unsafe { self.native().countGlyphs().try_into().unwrap() }
    }

    pub fn count_tables(&self) -> usize {
        unsafe { self.native().countTables().try_into().unwrap() }
    }

    pub fn table_tags(&self) -> Option<Vec<FontTableTag>> {
        let mut v: Vec<FontTableTag> = vec![0; self.count_tables()];
        (unsafe { self.native().getTableTags(v.as_mut_ptr()) } != 0).if_true_some(v)
    }

    #[deprecated(note = "use get_table_size()")]
    pub fn table_size(&self, tag: FontTableTag) -> Option<usize> {
        self.get_table_size(tag)
    }

    pub fn get_table_size(&self, tag: FontTableTag) -> Option<usize> {
        let size = unsafe { self.native().getTableSize(tag) };
        if size != 0 {
            Some(size)
        } else {
            None
        }
    }

    #[deprecated(note = "use get_table_data()")]
    pub fn table_data(&self, tag: FontTableTag, data: &mut [u8]) -> usize {
        self.get_table_data(tag, data)
    }

    pub fn get_table_data(&self, tag: FontTableTag, data: &mut [u8]) -> usize {
        unsafe {
            self.native()
                .getTableData(tag, 0, data.len(), data.as_mut_ptr() as _)
        }
    }

    pub fn units_per_em(&self) -> Option<i32> {
        let units = unsafe { self.native().getUnitsPerEm() };
        if units != 0 {
            Some(units)
        } else {
            None
        }
    }

    #[deprecated(note = "use get_kerning_pair_adjustments()")]
    pub fn kerning_pair_adjustments(&self, glyphs: &[GlyphId], adjustments: &mut [i32]) -> bool {
        self.get_kerning_pair_adjustments(glyphs, adjustments)
    }

    // note: adjustments slice length must be equal to glyph's len - 1.
    pub fn get_kerning_pair_adjustments(
        &self,
        glyphs: &[GlyphId],
        adjustments: &mut [i32],
    ) -> bool {
        (adjustments.len() + 1 == glyphs.len())
            && unsafe {
                self.native().getKerningPairAdjustments(
                    glyphs.as_ptr(),
                    glyphs.len().try_into().unwrap(),
                    adjustments.as_mut_ptr(),
                )
            }
    }

    pub fn new_family_name_iterator(&self) -> impl Iterator<Item = LocalizedString> {
        LocalizedStringsIter(unsafe { self.native().createFamilyNameIterator() })
    }

    pub fn family_name(&self) -> String {
        let mut name = interop::String::default();
        unsafe {
            self.native().getFamilyName(name.native_mut());
        };
        name.as_str().into()
    }

    // TODO: openStream()
    // TODO: makeFontData()
    // TODO: createScalerContext()

    pub fn bounds(&self) -> Rect {
        Rect::from_native(unsafe { self.native().getBounds() })
    }
}

#[repr(transparent)]
struct LocalizedStringsIter(*mut SkTypeface_LocalizedStrings);

impl NativeAccess<SkTypeface_LocalizedStrings> for LocalizedStringsIter {
    fn native(&self) -> &SkTypeface_LocalizedStrings {
        unsafe { &*self.0 }
    }

    fn native_mut(&mut self) -> &mut SkTypeface_LocalizedStrings {
        unsafe { &mut *self.0 }
    }
}

impl Drop for LocalizedStringsIter {
    fn drop(&mut self) {
        unsafe { C_SkTypeface_LocalizedStrings_unref(self.0) }
    }
}

impl Iterator for LocalizedStringsIter {
    type Item = LocalizedString;

    fn next(&mut self) -> Option<Self::Item> {
        let mut string = interop::String::default();
        let mut language = interop::String::default();
        unsafe {
            C_SkTypeface_LocalizedStrings_next(
                self.native_mut(),
                string.native_mut(),
                language.native_mut(),
            )
        }
        .if_true_then_some(|| LocalizedString {
            string: string.as_str().into(),
            language: language.as_str().into(),
        })
    }
}

#[test]
fn serialize_and_deserialize_default_typeface() {
    let tf = Typeface::default();
    let serialized = tf.serialize(SerializeBehavior::DoIncludeData);
    let deserialized = Typeface::deserialize(&serialized).unwrap();
    // why aren't they not equal?
    // assert!(Typeface::equal(&tf, &deserialized));
    assert_eq!(tf.family_name(), deserialized.family_name());
}

#[test]
fn family_name_iterator_owns_the_strings_and_returns_at_least_one_name_for_the_default_typeface() {
    let tf = Typeface::default();
    let family_names = tf.new_family_name_iterator();
    drop(tf);

    let mut any = false;
    for name in family_names {
        println!("family: {}, language: {}", name.string, name.language);
        any = true
    }
    assert!(any);
}
