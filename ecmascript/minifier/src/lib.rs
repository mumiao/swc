//! Javascript minifier implemented in rust.
//!
//! # Cargo features
//!
//! ## `debug`
//!
//! If you enable this cargo feature and set the environemnt variable named
//! `SWC_RUN` to `1`, the minifier will validate the code using node before each
//! step.
//!
//! Note: Passes should be visited only with [Module] and it's an error to feed
//! them something other. Don't call methods like `visit_mut_script` nor
//! `visit_mut_module_items`.

use crate::compress::compressor;
use crate::marks::Marks;
use crate::metadata::info_marker;
use crate::option::ExtraOptions;
use crate::option::MinifyOptions;
use crate::pass::compute_char_freq::compute_char_freq;
use crate::pass::expand_names::name_expander;
use crate::pass::global_defs;
use crate::pass::hygiene::hygiene_optimizer;
pub use crate::pass::hygiene::optimize_hygiene;
use crate::pass::mangle_names::name_mangler;
use crate::pass::mangle_props::mangle_properties;
use crate::pass::precompress::precompress_optimizer;
use crate::util::now;
use analyzer::analyze;
use pass::postcompress::postcompress_optimizer;
use std::time::Instant;
use swc_common::comments::Comments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_common::GLOBALS;
use swc_ecma_ast::Module;
use swc_ecma_visit::FoldWith;
use swc_ecma_visit::VisitMutWith;
use timing::Timings;

mod analyzer;
mod compress;
mod debug;
pub mod marks;
mod metadata;
pub mod option;
mod pass;
pub mod timing;
mod util;

const DISABLE_BUGGY_PASSES: bool = true;
const MAX_PAR_DEPTH: u8 = 3;

#[inline]
pub fn optimize(
    mut m: Module,
    _cm: Lrc<SourceMap>,
    comments: Option<&dyn Comments>,
    mut timings: Option<&mut Timings>,
    options: &MinifyOptions,
    extra: &ExtraOptions,
) -> Module {
    let marks = Marks::new();

    let start = now();
    if let Some(defs) = options.compress.as_ref().map(|c| &c.global_defs) {
        // Apply global defs.
        //
        // As terser treats `CONFIG['VALUE']` and `CONFIG.VALUE` differently, we don't
        // have to see if optimized code matches global definition and wecan run
        // this at startup.

        if !defs.is_empty() {
            let defs = defs.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            m.visit_mut_with(&mut global_defs::globals_defs(defs, extra.top_level_mark));
        }
    }
    if let Some(start) = start {
        log::info!("global_defs took {:?}", Instant::now() - start);
    }

    if let Some(options) = &options.compress {
        let start = now();
        m.visit_mut_with(&mut precompress_optimizer(options, marks));
        if let Some(start) = start {
            log::info!("precompress took {:?}", Instant::now() - start);
        }
    }

    m.visit_mut_with(&mut info_marker(comments, marks, extra.top_level_mark));

    if options.wrap {
        // TODO: wrap_common_js
        // toplevel = toplevel.wrap_commonjs(options.wrap);
    }

    if options.enclose {
        // TODO: enclose
        // toplevel = toplevel.wrap_enclose(options.enclose);
    }

    // We don't need validation.

    if let Some(ref mut _t) = timings {
        // TODO: store `rename`
    }

    // Noop.
    // https://github.com/mishoo/UglifyJS2/issues/2794
    if options.rename && false {
        // toplevel.figure_out_scope(options.mangle);
        // TODO: Pass `options.mangle` to name expander.
        m.visit_mut_with(&mut name_expander());
    }

    if let Some(ref mut t) = timings {
        t.section("compress");
    }
    if let Some(options) = &options.compress {
        let start = now();
        m = GLOBALS.with(|globals| m.fold_with(&mut compressor(globals, marks, &options)));
        if let Some(start) = start {
            log::info!("compressor took {:?}", Instant::now() - start);
        }
        // Again, we don't need to validate ast

        let start = now();
        m.visit_mut_with(&mut postcompress_optimizer(options));
        if let Some(start) = start {
            log::info!("postcompressor took {:?}", Instant::now() - start);
        }
    }

    if let Some(ref mut _t) = timings {
        // TODO: store `scope`
    }
    if options.mangle.is_some() {
        // toplevel.figure_out_scope(options.mangle);
    }

    if let Some(ref mut t) = timings {
        t.section("mangle");
    }

    if let Some(mangle) = &options.mangle {
        // TODO: base54.reset();

        let char_freq_info = compute_char_freq(&m);
        m.visit_mut_with(&mut name_mangler(mangle.clone(), char_freq_info, marks));
    }

    if let Some(property_mangle_options) = options.mangle.as_ref().and_then(|o| o.props.as_ref()) {
        mangle_properties(&mut m, property_mangle_options.clone());
    }

    if let Some(ref mut t) = timings {
        t.section("hygiene");
    }

    {
        let data = analyze(&m, None);
        m.visit_mut_with(&mut hygiene_optimizer(data, extra.top_level_mark));
    }

    if let Some(ref mut t) = timings {
        t.end_section();
    }

    m
}
