#[macro_use] extern crate log;
extern crate env_logger;
extern crate osmio;
extern crate csv;
#[macro_use] extern crate anyhow;
extern crate clap;
extern crate flate2;

use std::io::{BufReader};
use std::fs::File;
use std::collections::{HashMap, BTreeSet};
use std::cmp::Ordering;

use clap::{Arg, App};
use osmio::{OSMReader, OSMObj, OSMObjBase};

use flate2::write::GzEncoder;
use flate2::Compression;
use anyhow::Result;

fn main() -> Result<()> {

    let matches = App::new("osm-tag-csv-history")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Create a CSV file detailing tagging changes in an OSM file")
        .arg(Arg::with_name("input")
             .short("i").long("input")
             .value_name("INPUT.osm.pbf")
             .help("Input file to convert")
             .takes_value(true).required(true)
             )
        .arg(Arg::with_name("output")
             .short("o").long("output")
             .value_name("OUTPUT.csv")
             .help("Where to write the output. Use - for stdout")
             .takes_value(true).required(true)
             )
        .arg(Arg::with_name("verbosity")
             .short("v").multiple(true)
             .help("Increase verbosity")
             )
        .arg(Arg::with_name("header")
             .long("header")
             .takes_value(false).required(false)
             .help("Include a CSV header (default)")
             .conflicts_with("no-header")
             )
        .arg(Arg::with_name("no-header")
             .long("no-header")
             .takes_value(false).required(false)
             .help("Do not include a CSV header")
             .conflicts_with("header")
             )
        .arg(Arg::with_name("compression")
             .short("c").long("compression")
             .takes_value(true).required(false)
             .possible_values(&["none", "auto", "gzip"])
             .default_value("auto")
             .help("Should the output file be compressed?")
             )
        .get_matches();

    env_logger::builder()
        .filter_level(match matches.occurrences_of("verbosity"){
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
                })
        .init();

    let input_path = matches.value_of("input").unwrap();
    info!("Begining processing of {}", input_path);

    let file = File::open(input_path)?;
    let mut osm_obj_reader = osmio::pbf::PBFReader::new(BufReader::new(file));
    let mut objects_iter = osm_obj_reader.objects();

    let include_header = match (matches.is_present("header"), matches.is_present("no-header")) {
        (false, false) => true,
        (true, false) => true,
        (false, true) => false,
        (true, true) => unreachable!(),
    };

    let output_path = matches.value_of("output").unwrap();
    let output_writer: Box<dyn std::io::Write> = if output_path == "-" {
        Box::new(std::io::stdout())
    } else {
        Box::new(File::create(matches.value_of("output").unwrap())?)
    };
    let output_writer = match matches.value_of("compression") {
        Some("auto") => {
            if output_path == "-" {
                // stdout, so no compression
                trace!("Output is '-', no compression");
                output_writer
            } else if output_path.ends_with(".csv.gz") {
                trace!("Output file ends with .csv.gz so using regular gzip");
                Box::new(GzEncoder::new(output_writer, Compression::default()))
            } else if output_path.ends_with(".csv") {
                // uncompressed
                trace!("Output file ends with .csv so no compression");
                output_writer
            } else {
                bail!("Cannot auto-detect output compression format: {:?}", output_path);
            }
        }
        Some("none") => output_writer,
        Some("gzip") => Box::new(GzEncoder::new(output_writer, Compression::default())),
        _ => unreachable!(),
    };
    let mut output = csv::Writer::from_writer(output_writer);

    if include_header {
        trace!("Writing CSV header");
        output.write_record(&[
            "key",
            "old_value", "new_value",
            "object_type", "id",
            "old_version", "new_version",
            "datetime", "username", "uid", "changeset_id",
        ])?;
    }

    let mut curr = objects_iter.next().unwrap();
    let mut last: Option<osmio::obj_types::RcOSMObj> = None;

    loop {

        let (last_tags, last_version) = match last {
            None => (HashMap::new(), "".to_string()),
            Some(ref last) => {
                ensure!(sorted_objects(last, &curr) == Ordering::Less, "Non sorted input");
                if last.object_type() == curr.object_type() && last.id() == curr.id() {
                    ( last.tags().collect(), last.version().unwrap().to_string())
                } else {
                    (HashMap::new(), "".to_string())
                }
            },
        };

        let curr_tags: HashMap<_, _> = curr.tags().collect();
        let mut keys: BTreeSet<_> = curr_tags.keys().collect();
        keys.extend(last_tags.keys());

        for key in keys.into_iter() {
            let last_value = last_tags.get(key).unwrap_or(&"");
            let curr_value = curr_tags.get(key).unwrap_or(&"");
            if last_value == curr_value {
                continue;
            }

            trace!("Write tag change {} {:?} → {:?}", key, last_value, curr_value);
            output.write_record(&[
                key,
                last_value, curr_value,
                format!("{:?}", curr.object_type()).as_str(), curr.id().to_string().as_str(),
                last_version.as_str(),
                curr.version().unwrap().to_string().as_str(),
                curr.timestamp().as_ref().unwrap().to_iso_string().as_str(),
                curr.user().unwrap(), curr.uid().unwrap().to_string().as_str(),
                curr.changeset_id().unwrap().to_string().as_str(),
            ])?;

        }

        last = Some(curr);
        curr = match objects_iter.next() {
            None => { break; },
            Some(o) => o,
        };

    }


    info!("Finished");
    Ok(())
}

fn sorted_objects(a: &impl OSMObj, b: &impl OSMObj) -> std::cmp::Ordering {
    a.object_type().cmp(&b.object_type()).then(a.id().cmp(&b.id())).then(a.version().cmp(&b.version()))
}
