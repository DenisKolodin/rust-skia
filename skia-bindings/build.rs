extern crate bindgen;
extern crate cc;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use bindgen::EnumVariation;
use cc::Build;

fn main() {

  prerequisites::require_python();

  assert!(Command::new("git")
    .arg("submodule")
    .arg("init")
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .status().unwrap().success(), "`git submodule init` failed");

  assert!(Command::new("git")
    .args(&["submodule", "update", "--depth", "1"])
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .status().unwrap().success(), "`git submodule update` failed");

  assert!(Command::new("python")
    .arg("skia/tools/git-sync-deps")
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .status().unwrap().success(), "`skia/tools/git-sync-deps` failed");

  let target_str = env::var("TARGET").unwrap();

  let (architecture, vendor, system, abi) = {
    let target : Vec<&str> =
        target_str
            .split("-")
            .collect();

    if target.len() < 3 {
      panic!("Failed to parse TARGET {}", target_str);
    }

    (target[0], target[1], target[2], if target.len() > 3 { Some(target[3]) } else { None })
  };

  match (architecture, vendor, system, abi) {
    (_, "unknown", "linux", Some("gnu")) => {
      cargo::add_link_libs(&["stdc++", "bz2", "GL", "fontconfig", "freetype"]);
    },
    (_, _, "apple", Some("darwin")) => {
      cargo::add_link_libs(&["c++", "framework=OpenGL", "framework=ApplicationServices"]);
    },
    (_, _, "windows", abi) => {
      cargo::add_link_libs(&["usp10", "ole32", "user32", "gdi32", "fontsub", "opengl32"]);
      if abi == Some("gnu") {
        cargo::add_link_lib("stdc++");
      }
    },
    ("wasm32", _, _, _) => {
    },
    _ => {
      panic!("unsupported target: {}", target_str)
    }
  };

  let is_windows_target = system == "windows";

  let cc_flag =
      if is_windows_target {
        Some("-std=c++14")
      } else {
        None
      };

  let gn_args = {

    let keep_inline_functions = true;

    let mut args =
      r#"--args=is_official_build=true skia_use_expat=false skia_use_icu=false skia_use_system_libjpeg_turbo=false skia_use_system_libpng=false skia_use_libwebp=false skia_use_system_zlib=false cc="clang" cxx="clang++""#
      .to_owned();

    if cfg!(feature="vulkan") {
      args.push_str(" skia_use_vulkan=true skia_enable_spirv_validation=false");
    }

    // Skia supports msvc compatible clang flags when we are on Windows _and_ the target is Windows, so that needs
    // special treatment.
    let skia_uses_msvc = cfg!(windows) && is_windows_target;

    let mut flags : Vec<&str> = vec![];

    if skia_uses_msvc {
      flags.push(if cfg!(build="debug") { "/MTd" } else { "/MD" });

      if keep_inline_functions {
        // sadly, this also disables inlining completely and is probably a real performance bummer.
        flags.push("/Ob0")
      };

    } else {
      if keep_inline_functions {
        flags.push("-fno-inline-functions");
      }
    }

    if cfg!(windows) {
      args.push_str(r#" clang_win="C:\Program Files\LLVM""#);
    }

    if architecture == "wasm32" {
      args.push_str(r#" target_cpu="wasm""#);
      flags.push("--target=wasm32");
    }

    let flags: String = {
      fn quote(s: &str) -> String { String::from("\"") + s + "\"" }

      let v: Vec<String> = flags.into_iter().map(quote).collect();
      v.join(",")
    };

    args.push_str(&format!(" extra_cflags=[{}]", flags));
    args
  };

  let gn_command = if cfg!(windows) {
    "skia/bin/gn"
  } else {
    "bin/gn"
  };

  let skia_out_dir : String =
    PathBuf::from(env::var("OUT_DIR").unwrap())
      .join("skia/Static")
      .to_str().unwrap().into();

  dbg!(&skia_out_dir);
  dbg!(&gn_args);

  let output = Command::new(gn_command)
    .args(&["gen", &skia_out_dir, &gn_args])
    .envs(env::vars())
    .current_dir(PathBuf::from("./skia"))
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .output()
    .expect("gn error");

  if output.status.code() != Some(0) {
    panic!("{:?}", String::from_utf8(output.stdout).unwrap());
  }

  let ninja_command = if cfg!(windows) {
    "depot_tools/ninja"
  } else {
    "../depot_tools/ninja"
  };

  assert!(Command::new(ninja_command)
    .current_dir(PathBuf::from("./skia"))
    .args(&["-C", &skia_out_dir])
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .status()
    .expect("failed to run `ninja`, is the directory depot_tools/ available?")
    .success(), "`ninja` returned an error, please check the output for details.");

  let current_dir = env::current_dir().unwrap();
  let current_dir_name = current_dir.to_str().unwrap();

  println!("cargo:rustc-link-search={}", &skia_out_dir);
  cargo::add_link_libs(&["static=skia", "static=skiabinding"]);

  bindgen_gen(&current_dir_name, &skia_out_dir, cc_flag)
}

fn bindgen_gen(current_dir_name: &str, skia_out_dir: &str, cc_flag: Option<&str>) {

  let mut builder = bindgen::Builder::default()
    .generate_inline_functions(true)

    .default_enum_style(EnumVariation::Rust)

    .constified_enum(".*Mask")
    .constified_enum(".*Flags")
    .constified_enum("SkCanvas_SaveLayerFlagsSet")
    .constified_enum("GrVkAlloc_Flag")
    .constified_enum("GrGLBackendState")

    .whitelist_function("C_.*")
    .whitelist_function("SkColorTypeBytesPerPixel")
    .whitelist_function("SkColorTypeIsAlwaysOpaque")
    .whitelist_function("SkColorTypeValidateAlphaType")
    .whitelist_function("SkRGBToHSV")
    // this function does not whitelist (probably because of inlining):
    .whitelist_function("SkColorToHSV")
    .whitelist_function("SkHSVToColor")
    .whitelist_function("SkPreMultiplyARGB")
    .whitelist_function("SkPreMultiplyColor")
    .whitelist_function("SkBlendMode_Name")

    // functions for which the doc generation fails.
    .blacklist_function("SkColorFilter_asComponentTable")

    .whitelist_type("SkColorSpacePrimaries")
    .whitelist_type("SkVector4")
    .whitelist_type("SkPictureRecorder")
    .whitelist_type("SkAutoCanvasRestore")

    .whitelist_type("SkPath1DPathEffect")
    .whitelist_type("SkLine2DPathEffect")
    .whitelist_type("SkPath2DPathEffect")
    .whitelist_type("SkCornerPathEffect")
    .whitelist_type("SkDashPathEffect")
    .whitelist_type("SkDiscretePathEffect")
    .whitelist_type("SkGradientShader")
    .whitelist_type("SkPerlinNoiseShader")
    .whitelist_type("SkTableColorFilter")

    .whitelist_type("GrGLBackendState")

    .whitelist_type("GrVkDrawableInfo")
    .whitelist_type("GrVkExtensionFlags")
    .whitelist_type("GrVkFeatureFlags")

    .whitelist_var("SK_Color.*")
    .whitelist_var("kAll_GrBackendState")

    .use_core()
    .clang_arg("-std=c++14");

  let mut cc_build = Build::new();

  let bindings_source = "src/bindings.cpp";
  cargo::add_dependent_path(bindings_source);

  builder = builder.header(bindings_source);

  for include_dir in fs::read_dir("skia/include").expect("Unable to read skia/include") {
    let dir = include_dir.unwrap();
    cargo::add_dependent_path(dir.path().to_str().unwrap());
    let include_path = format!("{}/{}", &current_dir_name, &dir.path().to_str().unwrap());
    builder = builder.clang_arg(format!("-I{}", &include_path));
    cc_build.include(&include_path);
  }

  if cfg!(feature="vulkan") {
    cc_build.define("SK_VULKAN", "1");
    builder = builder.clang_arg("-DSK_VULKAN");
    cc_build.define("SKIA_IMPLEMENTATION", "1");
    builder = builder.clang_arg("-DSKIA_IMPLEMENTATION=1");
  }

  let cc_build = cc_build
    .cpp(true)
    .file(bindings_source)
    .out_dir(skia_out_dir);

  let cc_build =
      match cc_flag {
        Some(flag) => { cc_build.flag(flag) },
        None => { cc_build }
      };

  cc_build.compile("skiabinding");

  let bindings = builder.generate().expect("Unable to generate bindings");

  let out_path = PathBuf::from("src");
  bindings
    .write_to_file(out_path.join("bindings.rs"))
    .expect("Couldn't write bindings!");
}

mod cargo {
  pub fn add_dependent_path(path: &str) {
    println!("cargo:rerun-if-changed={}", path);
  }

  pub fn add_link_libs<'a, L: IntoIterator<Item = &'a &'a str>>(libs: L) {
    libs.into_iter().for_each(|s| add_link_lib(*s))
  }

  pub fn add_link_lib(lib: &str) {
    println!("cargo:rustc-link-lib={}", lib);
  }
}

mod prerequisites {
  use std::process::{Command, Stdio};

  pub fn require_python() {
    Command::new("python")
    .arg("--version")
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .status().expect(">>>>> Please install python to build this crate. <<<<<");
  }
}
