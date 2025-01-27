use crate::prelude::*;
use crate::{Contains, IPoint, IRect, IVector, Path, QuickReject};
use skia_bindings::{
    C_SkRegion_Cliperator_destruct, C_SkRegion_Equals, C_SkRegion_Iterator_Construct,
    C_SkRegion_Iterator_rgn, C_SkRegion_Spanerator_destruct, C_SkRegion_destruct, SkRegion,
    SkRegion_Cliperator, SkRegion_Iterator, SkRegion_Op, SkRegion_Spanerator,
};
use std::marker::PhantomData;
use std::{iter, mem, ptr};

pub type Region = Handle<SkRegion>;

impl NativeDrop for SkRegion {
    fn drop(&mut self) {
        // does not link:
        // unsafe { SkRegion::destruct(self) }
        unsafe { C_SkRegion_destruct(self) }
    }
}

impl NativeClone for SkRegion {
    fn clone(&self) -> Self {
        unsafe { SkRegion::new1(self) }
    }
}

impl NativePartialEq for SkRegion {
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { C_SkRegion_Equals(self, rhs) }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum RegionOp {
    Difference = SkRegion_Op::kDifference_Op as _,
    Intersect = SkRegion_Op::kIntersect_Op as _,
    Union = SkRegion_Op::kUnion_Op as _,
    XOR = SkRegion_Op::kXOR_Op as _,
    ReverseDifference = SkRegion_Op::kReverseDifference_Op as _,
    Replace = SkRegion_Op::kReplace_Op as _,
}

impl NativeTransmutable<SkRegion_Op> for RegionOp {}
#[test]
fn test_region_op_layout() {
    RegionOp::test_layout()
}

impl Handle<SkRegion> {
    pub fn new() -> Region {
        Self::from_native(unsafe { SkRegion::new() })
    }

    pub fn from_rect(rect: impl AsRef<IRect>) -> Region {
        Self::from_native(unsafe { SkRegion::new2(rect.as_ref().native()) })
    }

    pub fn set(&mut self, src: &Region) -> bool {
        unsafe { self.native_mut().set(src.native()) }
    }

    pub fn swap(&mut self, other: &mut Region) {
        unsafe { self.native_mut().swap(other.native_mut()) }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { self.native().isEmpty() }
    }

    pub fn is_rect(&self) -> bool {
        unsafe { self.native().isRect() }
    }

    pub fn is_complex(&self) -> bool {
        unsafe { self.native().isComplex() }
    }

    pub fn bounds(&self) -> IRect {
        IRect::from_native(unsafe { *self.native().getBounds() })
    }

    pub fn compute_region_complexity(&self) -> usize {
        unsafe { self.native().computeRegionComplexity().try_into().unwrap() }
    }

    #[deprecated(since = "0.12.0", note = "use get_boundary_path()")]
    pub fn boundary_path(&self, path: &mut Path) -> bool {
        self.get_boundary_path(path)
    }

    pub fn get_boundary_path(&self, path: &mut Path) -> bool {
        unsafe { self.native().getBoundaryPath(path.native_mut()) }
    }

    pub fn set_empty(&mut self) -> bool {
        unsafe { self.native_mut().setEmpty() }
    }

    pub fn set_rect(&mut self, rect: impl AsRef<IRect>) -> bool {
        unsafe { self.native_mut().setRect(rect.as_ref().native()) }
    }

    pub fn set_rect_ltbr(&mut self, left: i32, top: i32, right: i32, bottom: i32) -> bool {
        // does not link:
        // unsafe { self.native_mut().setRect1(left, top, right, bottom) }
        self.set_rect(IRect::new(left, top, right, bottom))
    }

    pub fn set_rects(&mut self, rects: &[IRect]) -> bool {
        unsafe {
            self.native_mut()
                .setRects(rects.native().as_ptr(), rects.len().try_into().unwrap())
        }
    }

    pub fn set_region(&mut self, region: &Region) -> bool {
        unsafe { self.native_mut().setRegion(region.native()) }
    }

    pub fn set_path(&mut self, path: &Path, clip: &Region) -> bool {
        unsafe { self.native_mut().setPath(path.native(), clip.native()) }
    }

    // there is also a trait for intersects() below.

    pub fn intersects_rect(&self, rect: impl AsRef<IRect>) -> bool {
        unsafe { self.native().intersects(rect.as_ref().native()) }
    }

    pub fn intersects_region(&self, other: &Region) -> bool {
        unsafe { self.native().intersects1(other.native()) }
    }

    // contains() trait below.

    pub fn contains_point(&self, point: IPoint) -> bool {
        unsafe { self.native().contains(point.x, point.y) }
    }

    pub fn contains_rect(&self, rect: impl AsRef<IRect>) -> bool {
        unsafe { self.native().contains1(rect.as_ref().native()) }
    }

    pub fn contains_region(&self, other: &Region) -> bool {
        unsafe { self.native().contains2(other.native()) }
    }

    pub fn quick_contains(&self, rect: impl AsRef<IRect>) -> bool {
        unsafe { self.native().quickContains(rect.as_ref().native()) }
    }

    pub fn quick_contains_ltrb(&self, left: i32, top: i32, right: i32, bottom: i32) -> bool {
        unsafe { self.native().quickContains1(left, top, right, bottom) }
    }

    // quick_reject() trait below.

    pub fn quick_reject_rect(&self, rect: impl AsRef<IRect>) -> bool {
        unsafe { self.native().quickReject(rect.as_ref().native()) }
    }

    pub fn quick_reject_region(&self, other: &Region) -> bool {
        // does not link:
        // unsafe { self.native().quickReject1(other.native()) }
        self.is_empty() || other.is_empty() || !IRect::intersects(&self.bounds(), &other.bounds())
    }

    pub fn translate(&mut self, d: impl Into<IVector>) {
        let d = d.into();
        unsafe { self.native_mut().translate(d.x, d.y) }
    }

    pub fn translated(&self, d: impl Into<IVector>) -> Region {
        let mut r = self.clone();
        r.translate(d);
        r
    }

    pub fn op_rect(&mut self, rect: impl AsRef<IRect>, op: RegionOp) -> bool {
        unsafe {
            self.native_mut()
                .op(rect.as_ref().native(), op.into_native())
        }
    }

    pub fn op_region(&mut self, region: &Region, op: RegionOp) -> bool {
        unsafe { self.native_mut().op2(region.native(), op.into_native()) }
    }

    pub fn op_rect_region(
        &mut self,
        rect: impl AsRef<IRect>,
        region: &Region,
        op: RegionOp,
    ) -> bool {
        unsafe {
            self.native_mut()
                .op3(rect.as_ref().native(), region.native(), op.into_native())
        }
    }

    pub fn op_region_rect(
        &mut self,
        region: &Region,
        rect: impl AsRef<IRect>,
        op: RegionOp,
    ) -> bool {
        unsafe {
            self.native_mut()
                .op4(region.native(), rect.as_ref().native(), op.into_native())
        }
    }

    pub fn write_to_memory(&self, buf: &mut Vec<u8>) {
        unsafe {
            let size = self.native().writeToMemory(ptr::null_mut());
            buf.resize(size, 0);
            let written = self.native().writeToMemory(buf.as_mut_ptr() as _);
            debug_assert!(written == size);
        }
    }

    pub fn read_from_memory(&mut self, buf: &[u8]) -> usize {
        unsafe {
            self.native_mut()
                .readFromMemory(buf.as_ptr() as _, buf.len())
        }
    }
}

//
// combine overloads (static)
//

pub trait Combine<A, B>: Sized {
    fn combine(a: &A, op: RegionOp, b: &B) -> Self;

    fn difference(a: &A, b: &B) -> Self {
        Self::combine(a, RegionOp::Difference, b)
    }

    fn intersect(a: &A, b: &B) -> Self {
        Self::combine(a, RegionOp::Intersect, b)
    }

    fn xor(a: &A, b: &B) -> Self {
        Self::combine(a, RegionOp::XOR, b)
    }

    fn union(a: &A, b: &B) -> Self {
        Self::combine(a, RegionOp::Union, b)
    }

    fn reverse_difference(a: &A, b: &B) -> Self {
        Self::combine(a, RegionOp::ReverseDifference, b)
    }

    fn replace(a: &A, b: &B) -> Self {
        Self::combine(a, RegionOp::Replace, b)
    }
}

impl Combine<IRect, Region> for Handle<SkRegion> {
    fn combine(rect: &IRect, op: RegionOp, region: &Region) -> Self {
        let mut r = Region::new();
        r.op_rect_region(rect, region, op);
        r
    }
}

impl Combine<Region, IRect> for Handle<SkRegion> {
    fn combine(region: &Region, op: RegionOp, rect: &IRect) -> Self {
        let mut r = Region::new();
        r.op_region_rect(region, rect, op);
        r
    }
}

impl Combine<Region, Region> for Handle<SkRegion> {
    fn combine(a: &Region, op: RegionOp, b: &Region) -> Self {
        let mut a = a.clone();
        a.op_region(b, op);
        a
    }
}

//
// intersects overloads
//

pub trait Intersects<T> {
    fn intersects(&self, other: &T) -> bool;
}

impl Intersects<IRect> for Region {
    fn intersects(&self, rect: &IRect) -> bool {
        self.intersects_rect(rect)
    }
}

impl Intersects<Region> for Region {
    fn intersects(&self, other: &Region) -> bool {
        self.intersects_region(other)
    }
}

//
// contains overloads
//

impl Contains<IPoint> for Region {
    fn contains(&self, point: IPoint) -> bool {
        self.contains_point(point)
    }
}

impl Contains<&IRect> for Region {
    fn contains(&self, rect: &IRect) -> bool {
        self.contains_rect(rect)
    }
}

impl Contains<&Region> for Region {
    fn contains(&self, other: &Region) -> bool {
        self.contains_region(other)
    }
}

//
// quick_reject overloads
//

impl QuickReject<IRect> for Region {
    fn quick_reject(&self, rect: &IRect) -> bool {
        self.quick_reject_rect(rect)
    }
}

impl QuickReject<Region> for Region {
    fn quick_reject(&self, other: &Region) -> bool {
        self.quick_reject_region(other)
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct Iterator<'a>(SkRegion_Iterator, PhantomData<&'a Region>);

impl<'a> NativeTransmutable<SkRegion_Iterator> for Iterator<'a> {}
#[test]
fn test_iterator_layout() {
    Iterator::test_layout();
}

impl<'a> Iterator<'a> {
    pub fn new_empty() -> Iterator<'a> {
        Iterator::from_native(unsafe {
            // does not link:
            // SkRegion_Iterator::new()
            let mut iterator = mem::zeroed();
            C_SkRegion_Iterator_Construct(&mut iterator);
            iterator
        })
    }

    pub fn new(region: &'a Region) -> Iterator<'a> {
        Iterator::from_native(unsafe { SkRegion_Iterator::new1(region.native()) })
    }

    pub fn rewind(&mut self) -> bool {
        unsafe { self.native_mut().rewind() }
    }

    pub fn reset(mut self, region: &Region) -> Iterator {
        unsafe {
            self.native_mut().reset(region.native());
            let r = mem::transmute_copy(&self);
            mem::forget(self);
            r
        }
    }

    pub fn is_done(&self) -> bool {
        unsafe { self.native().done() }
    }

    pub fn next(&mut self) {
        unsafe {
            self.native_mut().next();
        }
    }

    pub fn rect(&self) -> &IRect {
        IRect::from_native_ref(unsafe { &*self.native().rect() })
    }

    pub fn rgn(&self) -> Option<&Region> {
        unsafe {
            // does not link:
            // let r = self.native().rgn();
            let r = C_SkRegion_Iterator_rgn(self.native());
            if r.is_null() {
                return None;
            }
            Some(transmute_ref(&*r))
        }
    }
}

impl<'a> iter::Iterator for Iterator<'a> {
    type Item = IRect;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }
        let r = *self.rect();
        Iterator::next(self);
        Some(r)
    }
}

#[test]
fn test_iterator() {
    let r1 = IRect::new(10, 10, 12, 14);
    let r2 = IRect::new(100, 100, 120, 140);
    let mut r = Region::new();
    r.set_rects(&[r1, r2]);
    let rects: Vec<IRect> = Iterator::new(&r).collect();
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0], r1);
    assert_eq!(rects[1], r2);
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct Cliperator<'a>(SkRegion_Cliperator, PhantomData<&'a Region>);

impl<'a> NativeTransmutable<SkRegion_Cliperator> for Cliperator<'a> {}
#[test]
fn test_cliperator_layout() {
    Cliperator::test_layout();
}

impl<'a> Drop for Cliperator<'a> {
    fn drop(&mut self) {
        unsafe { C_SkRegion_Cliperator_destruct(self.native_mut()) }
    }
}

impl<'a> Cliperator<'a> {
    pub fn new(region: &'a Region, clip: impl AsRef<IRect>) -> Cliperator<'a> {
        Cliperator::from_native(unsafe {
            SkRegion_Cliperator::new(region.native(), clip.as_ref().native())
        })
    }

    // TODO: why does this function need &mut self?
    #[allow(clippy::wrong_self_convention)]
    pub fn is_done(&mut self) -> bool {
        unsafe { self.native_mut().done() }
    }

    pub fn next(&mut self) {
        unsafe { self.native_mut().next() }
    }

    pub fn rect(&self) -> &IRect {
        IRect::from_native_ref(unsafe { &*self.native().rect() })
    }
}

impl<'a> iter::Iterator for Cliperator<'a> {
    type Item = IRect;
    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }
        let rect = *self.rect();
        self.next();
        Some(rect)
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct Spanerator<'a>(SkRegion_Spanerator, PhantomData<&'a Region>);

impl<'a> NativeTransmutable<SkRegion_Spanerator> for Spanerator<'a> {}
#[test]
fn test_spanerator_layout() {
    Spanerator::test_layout();
}

impl<'a> Drop for Spanerator<'a> {
    fn drop(&mut self) {
        unsafe { C_SkRegion_Spanerator_destruct(self.native_mut()) }
    }
}

impl<'a> Spanerator<'a> {
    pub fn new(region: &'a Region, y: i32, left: i32, right: i32) -> Spanerator<'a> {
        Spanerator::from_native(unsafe {
            SkRegion_Spanerator::new(region.native(), y, left, right)
        })
    }
}

impl<'a> iter::Iterator for Spanerator<'a> {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut left = 0;
            let mut right = 0;
            self.native_mut()
                .next(&mut left, &mut right)
                .if_true_some((left, right))
        }
    }
}

#[test]
fn new_clone_drop() {
    let region = Region::new();
    let _cloned = region.clone();
}

#[test]
fn can_compare() {
    let r1 = Region::new();
    let r2 = r1.clone();
    assert!(r1 == r2);
}
