// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Used by `rustc` when loading a plugin.

use driver::session::Session;
use metadata::creader::PluginMetadataReader;
use plugin::registry::Registry;

use std::mem;
use std::os;
use std::dynamic_lib::DynamicLibrary;
use syntax::ast;
use syntax::attr;
use syntax::visit;
use syntax::visit::Visitor;
use syntax::ext::expand::ExportedMacros;
use syntax::attr::AttrMetaMethods;

/// Plugin-related crate metadata.
pub struct PluginMetadata {
    /// Source code of macros exported by the crate.
    pub macros: Vec<String>,
    /// Path to the shared library file.
    pub lib: Option<Path>,
    /// Symbol name of the plugin registrar function.
    pub registrar_symbol: Option<String>,
}

/// Pointer to a registrar function.
pub type PluginRegistrarFun =
    fn(&mut Registry);

/// Information about loaded plugins.
pub struct Plugins {
    /// Source code of exported macros.
    pub macros: Vec<ExportedMacros>,
    /// Registrars, as function pointers.
    pub registrars: Vec<PluginRegistrarFun>,
}

struct PluginLoader<'a> {
    sess: &'a Session,
    reader: PluginMetadataReader<'a>,
    plugins: Plugins,
}

impl<'a> PluginLoader<'a> {
    fn new(sess: &'a Session) -> PluginLoader<'a> {
        PluginLoader {
            sess: sess,
            reader: PluginMetadataReader::new(sess),
            plugins: Plugins {
                macros: vec!(),
                registrars: vec!(),
            },
        }
    }
}

/// Read plugin metadata and dynamically load registrar functions.
pub fn load_plugins(sess: &Session, krate: &ast::Crate) -> Plugins {
    let mut loader = PluginLoader::new(sess);
    visit::walk_crate(&mut loader, krate, ());
    loader.plugins
}

impl<'a> Visitor<()> for PluginLoader<'a> {
    fn visit_view_item(&mut self, vi: &ast::ViewItem, _: ()) {
        match vi.node {
            ast::ViewItemExternCrate(name, _, _) => {
                let mut plugin_phase = false;

                for attr in vi.attrs.iter().filter(|a| a.check_name("phase")) {
                    let phases = attr.meta_item_list().unwrap_or(&[]);
                    if attr::contains_name(phases, "plugin") {
                        plugin_phase = true;
                    }
                    if attr::contains_name(phases, "syntax") {
                        plugin_phase = true;
                        self.sess.span_warn(attr.span,
                            "phase(syntax) is a deprecated synonym for phase(plugin)");
                    }
                }

                if !plugin_phase { return; }

                let PluginMetadata { macros, lib, registrar_symbol } =
                    self.reader.read_plugin_metadata(vi);

                self.plugins.macros.push(ExportedMacros {
                    crate_name: name,
                    macros: macros,
                });

                match (lib, registrar_symbol) {
                    (Some(lib), Some(symbol))
                        => self.dylink_registrar(vi, lib, symbol),
                    _ => (),
                }
            }
            _ => (),
        }
    }
}

impl<'a> PluginLoader<'a> {
    // Dynamically link a registrar function into the compiler process.
    fn dylink_registrar(&mut self, vi: &ast::ViewItem, path: Path, symbol: String) {
        // Make sure the path contains a / or the linker will search for it.
        let path = os::make_absolute(&path);

        let lib = match DynamicLibrary::open(Some(&path)) {
            Ok(lib) => lib,
            // this is fatal: there are almost certainly macros we need
            // inside this crate, so continue would spew "macro undefined"
            // errors
            Err(err) => self.sess.span_fatal(vi.span, err.as_slice())
        };

        unsafe {
            let registrar =
                match lib.symbol(symbol.as_slice()) {
                    Ok(registrar) => {
                        mem::transmute::<*u8,PluginRegistrarFun>(registrar)
                    }
                    // again fatal if we can't register macros
                    Err(err) => self.sess.span_fatal(vi.span, err.as_slice())
                };

            self.plugins.registrars.push(registrar);

            // Intentionally leak the dynamic library. We can't ever unload it
            // since the library can make things that will live arbitrarily long
            // (e.g. an @-box cycle or a task).
            mem::forget(lib);

        }
    }
}
