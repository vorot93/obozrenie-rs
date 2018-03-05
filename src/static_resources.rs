use gdk_pixbuf::Pixbuf;
use gio::{resources_register, Error, Resource};
use glib::Bytes;
use gtk;
use std::collections::HashMap;
use std::rc::Rc;

const RES_ROOT_PATH: &str = "/io/obozrenie";

pub struct Resources {
    pub game_icons: HashMap<String, Pixbuf>,
    pub ui: gtk::Builder,
}

pub(crate) fn init() -> Result<Rc<Resources>, Error> {
    // load the gresource binary at build time and include/link it into the final binary.
    let res_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/resources.gresource"));

    // Create Resource, it will live as long the value lives.
    let gbytes = Bytes::from(res_bytes.as_ref());
    let resource = Resource::new_from_data(&gbytes)?;

    // Register the resource so It wont be dropped and will continue to live in memory.
    resources_register(&resource);

    let out = Rc::new(Resources {
        game_icons: ["openttd", "q3a"]
            .into_iter()
            .map(|entry| {
                for format in ["png", "svg"].into_iter() {
                    if let Ok(pixbuf) =
                        Pixbuf::new_from_resource_at_scale(&format!("{}/game_icons/{}.{}", RES_ROOT_PATH, entry, format), 24, 24, false)
                    {
                        return (entry.to_string(), pixbuf);
                    }
                }

                panic!("Failed to load icon for game {}", entry);
            })
            .collect(),
        ui: gtk::Builder::new_from_resource(&format!("{}/ui/app.ui", RES_ROOT_PATH)),
    });

    Ok(out)
}
