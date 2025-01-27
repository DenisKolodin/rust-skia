//! Wrapper for pathops/SkPathOps.h

use crate::prelude::*;
use crate::{Path, Rect};
use skia_bindings::{
    C_SkOpBuilder_Construct, C_SkOpBuilder_destruct, SkOpBuilder, SkPath, SkPathOp,
};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum PathOp {
    Difference = SkPathOp::kDifference_SkPathOp as _,
    Intersect = SkPathOp::kIntersect_SkPathOp as _,
    Union = SkPathOp::kUnion_SkPathOp as _,
    XOR = SkPathOp::kXOR_SkPathOp as _,
    ReverseDifference = SkPathOp::kReverseDifference_SkPathOp as _,
}

impl NativeTransmutable<SkPathOp> for PathOp {}
#[test]
fn test_path_op_layout() {
    PathOp::test_layout();
}

// TODO: I am not so sure if we should export these global functions.

pub fn op(one: &Path, two: &Path, op: PathOp) -> Option<Path> {
    let mut result = Path::default();
    unsafe {
        skia_bindings::Op(
            one.native(),
            two.native(),
            op.into_native(),
            result.native_mut(),
        )
    }
    .if_true_some(result)
}

pub fn simplify(path: &Path) -> Option<Path> {
    let mut result = Path::default();
    unsafe { skia_bindings::Simplify(path.native(), result.native_mut()) }.if_true_some(result)
}

pub fn tight_bounds(path: &Path) -> Option<Rect> {
    let mut result = Rect::default();
    unsafe { skia_bindings::TightBounds(path.native(), result.native_mut()) }.if_true_some(result)
}

pub fn as_winding(path: &Path) -> Option<Path> {
    let mut result = Path::default();
    unsafe { skia_bindings::AsWinding(path.native(), result.native_mut()) }.if_true_some(result)
}

pub type OpBuilder = Handle<SkOpBuilder>;

impl NativeDrop for SkOpBuilder {
    fn drop(&mut self) {
        unsafe { C_SkOpBuilder_destruct(self) }
    }
}

impl Default for Handle<SkOpBuilder> {
    fn default() -> Self {
        Self::construct_c(C_SkOpBuilder_Construct)
    }
}

impl Handle<SkOpBuilder> {
    pub fn add(&mut self, path: &Path, operator: PathOp) -> &mut Self {
        unsafe {
            self.native_mut().add(path.native(), operator.into_native());
        }
        self
    }

    pub fn resolve(&mut self) -> Option<Path> {
        let mut path = Path::default();
        unsafe { self.native_mut().resolve(path.native_mut()) }.if_true_some(path)
    }
}

impl Handle<SkPath> {
    pub fn op(&self, path: &Path, path_op: PathOp) -> Option<Self> {
        op(self, path, path_op)
    }

    pub fn simplify(&self) -> Option<Self> {
        simplify(self)
    }

    pub fn tight_bounds(&self) -> Option<Rect> {
        tight_bounds(self)
    }

    pub fn as_winding(&self) -> Option<Path> {
        as_winding(self)
    }
}

#[test]
fn test_tight_bounds() {
    let mut path = Path::new();
    path.add_rect(Rect::from_point_and_size((10.0, 10.0), (10.0, 10.0)), None);
    path.add_rect(Rect::from_point_and_size((15.0, 15.0), (10.0, 10.0)), None);
    let tight_bounds: Rect = Rect::from_point_and_size((10.0, 10.0), (15.0, 15.0));
    assert_eq!(path.tight_bounds().unwrap(), tight_bounds);
}

#[test]
fn test_union() {
    let mut path = Path::new();
    path.add_rect(Rect::from_point_and_size((10.0, 10.0), (10.0, 10.0)), None);
    let mut path2 = Path::new();
    path2.add_rect(Rect::from_point_and_size((15.0, 15.0), (10.0, 10.0)), None);
    let union = path.op(&path2, PathOp::Union).unwrap();
    let expected: Rect = Rect::from_point_and_size((10.0, 10.0), (15.0, 15.0));
    assert_eq!(union.tight_bounds().unwrap(), expected);
}

#[test]
fn test_intersect() {
    let mut path = Path::new();
    path.add_rect(Rect::from_point_and_size((10.0, 10.0), (10.0, 10.0)), None);
    let mut path2 = Path::new();
    path2.add_rect(Rect::from_point_and_size((15.0, 15.0), (10.0, 10.0)), None);
    let intersected = path.op(&path2, PathOp::Intersect).unwrap();
    let expected: Rect = Rect::from_point_and_size((15.0, 15.0), (5.0, 5.0));
    assert_eq!(intersected.tight_bounds().unwrap(), expected);
}
