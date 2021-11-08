#[macro_use]
extern crate log;
extern crate csv;
extern crate env_logger;
extern crate osmio;
#[macro_use]
extern crate anyhow;
extern crate clap;
extern crate do_every;
extern crate flate2;
extern crate read_progress;
extern crate rusqlite;
extern crate serde_json;

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

use clap::{App, Arg};
use osmio::{OSMObj, OSMObjBase, OSMReader};

use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use read_progress::ReaderWithSize;
use rusqlite::{Connection, OptionalExtension};

enum TimestampFormat {
    Datetime,
    EpochTime,
}

fn main() -> Result<()> {
    let matches = App::new("osm-tag-csv-history")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Create a CSV file detailing tagging changes in an OSM file")

        .arg(Arg::with_name("input")
             .short("i").long("input")
             .value_name("INPUT.osh.pbf")
             .help("Input file to convert.")
             .long_help("Read OSM data from this file. If it's a .osh.pbf history file, the full history will be output. Regular non-history files can be processed too")
             .takes_value(true).required(true)
             )

        .arg(Arg::with_name("output")
             .short("o").long("output")
             .value_name("OUTPUT.csv[.gz]")
             .help("Where to write the output. Use - for stdout. with auto compression (default), if this file ends with .gz, then it will be gzip compressed")
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
             .hidden_short_help(true)
             .default_value("auto")
             .help("Should the output file be compressed?")
             .long_help("Should the CSV output be compress?\nnone = don't compress the output\ngzip = always compress output with gzip\nauto (default) = uncompressed unless the output filename ends in .gz")
             )

        .arg(Arg::with_name("log-frequency")
             .long("log-frequency")
             .value_name("SEC")
             .takes_value(true).required(false)
             .hidden_short_help(true)
             .default_value("10")
             .help("with -v, how often (in sec.) to print progress messages")
             )

        .arg(Arg::with_name("timestamp_format")
             .long("timestamp-format")
             .takes_value(true).required(false)
             .possible_values(&["datetime", "epoch_time"])
             .default_value("datetime")
             .hidden_short_help(true)
             .help("What format to use for time column in output file?")
             )

        .arg(Arg::with_name("tag")
             .short("t").long("tag")
             .value_name("TAG")
             .takes_value(true).required(false)
             .multiple(true).number_of_values(1)
             .help("Only include changes to this tag (can be specified multiple times)")
             )


        .arg(Arg::with_name("changeset_filename")
             .long("changesets")
             .value_name("changesets-latest.osm.bz2")
             .takes_value(true).required(false)
             .help("Filename of the changeset file")
             )

        .arg(Arg::with_name("changeset_tag")
             .short("C").long("changeset-tag")
             .value_name("TAG")
             .takes_value(true).required(false)
             .multiple(true)
             .help("Include a column with this changeset tag")
             .long_help("Include a column (at the end) called changeset_X, with the changeset tag. e.g. -C created_by will add a column changeset_created_by with the value of the created_by tag to every object.\nRequires that the --changesets argument is given\nCan be given multiple times.")
             .requires("changeset_filename")
             )

        .arg(Arg::with_name("uid")
             .long("uid")
             .value_name("USERID")
             .takes_value(true).required(false)
             .multiple(true).number_of_values(1)
             .help("Only include changes made by this OSM user (by userid)")
             )



        .get_matches();

    env_logger::builder()
        .filter_level(match matches.occurrences_of("verbosity") {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        })
        .init();

    let input_path = matches.value_of("input").unwrap();
    info!("Beginning processing of {}", input_path);

    let log_frequency: f32 = matches.value_of("log-frequency").unwrap().parse()?;
    let timestamp_format = match matches.value_of("timestamp_format").unwrap() {
        "datetime" => TimestampFormat::Datetime,
        "epoch_time" => TimestampFormat::EpochTime,
        _ => unreachable!(),
    };

    let file =
        File::open(input_path).with_context(|| format!("opening input file {}", input_path))?;
    let mut osm_obj_reader =
        osmio::pbf::PBFReader::new(BufReader::new(ReaderWithSize::from_file(file)?));
    let mut objects_iter = osm_obj_reader.objects();

    let only_include_tags: Option<Vec<String>> = matches
        .values_of("tag")
        .map(|ts| ts.map(|s| s.to_string()).collect());

    let only_include_uids: Option<Vec<u32>> = match matches.values_of("uid") {
        None => None,
        Some(vals) => Some(vals.map(|u| Ok(u.parse()?)).collect::<Result<Vec<u32>>>()?),
    };

    if let Some(only_include_tags) = only_include_tags.as_ref() {
        info!(
            "Only including changes to these {} tag(s): {:?}",
            only_include_tags.len(),
            only_include_tags
        );
    }

    if let Some(only_include_uids) = only_include_uids.as_ref() {
        info!(
            "Only including changes made by user id {:?}",
            only_include_uids
        );
    }

    // changesets?
    let changeset_tags = match matches.values_of("changeset_tags") {
        None => None,
        Some(tags) => {
            let tags = tags
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>();
            info!(
                "Including columns for the following {} changeset tags: {:?}",
                tags.len(),
                tags
            );
            let lookup =
                ChangesetTagLookup::from_filename(matches.value_of("changeset_filename").unwrap())?;
            debug!(
                "Reading changeset sqlite from {}",
                matches.value_of("changeset_filename").unwrap()
            );
            Some((tags, lookup))
        }
    };

    let include_header = match (
        matches.is_present("header"),
        matches.is_present("no-header"),
    ) {
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
                bail!(
                    "Cannot auto-detect output compression format: {:?}",
                    output_path
                );
            }
        }
        Some("none") => output_writer,
        Some("gzip") => Box::new(GzEncoder::new(output_writer, Compression::default())),
        _ => unreachable!(),
    };
    let mut output = csv::Writer::from_writer(output_writer);

    if include_header {
        trace!("Writing CSV header");
        for header_field in &[
            "key",
            "new_value",
            "old_value",
            "id",
            "new_version",
            "old_version",
            match timestamp_format {
                TimestampFormat::Datetime => "datetime",
                TimestampFormat::EpochTime => "epoch_time",
            },
            "username",
            "uid",
            "changeset_id",
        ] {
            output.write_field(header_field)?;
        }

        if let Some((changeset_tags, _)) = changeset_tags.as_ref() {
            for changeset_tag in changeset_tags {
                output.write_field(format!("changeset_{}", changeset_tag))?;
            }
        }

        output.write_record(None::<&[u8]>)?;
    }

    let mut curr = objects_iter.next().unwrap();
    let mut last: Option<osmio::obj_types::ArcOSMObj> = None;

    let mut num_objects = 0;

    let mut time_counter = do_every::DoEvery::new();

    let mut field_bytes = Vec::with_capacity(25);
    let mut utf8_bytes_buffer = vec![0; 4];
    let started_processing = Instant::now();
    let mut passes_uid_check;

    loop {
        // Logging output
        num_objects += 1;
        if num_objects % 1000 == 0 && time_counter.should_do_every_sec(log_frequency) {
            let reader = objects_iter.inner().inner().get_ref();
            info!(
                "Running: {:.3}% done ETA: {} est. total: {}",
                reader.fraction() * 100.,
                reader
                    .eta()
                    .map(|d| format_time(&d))
                    .unwrap_or_else(|| "N/A".to_string()),
                reader
                    .est_total_time()
                    .map(|d| format_time(&d))
                    .unwrap_or_else(|| "N/A".to_string()),
            );
            num_objects = 1;
        }

        passes_uid_check = if let (Some(this_uid), Some(only_include_uids)) =
            (curr.uid(), only_include_uids.as_ref())
        {
            // We have uid's & we're filtering based on uids
            only_include_uids.iter().any(|u| u == &this_uid)
        } else {
            true
        };

        let has_tags = match last {
            None => curr.tagged(),
            Some(ref l) => l.tagged() || curr.tagged(),
        };
        let process_object = has_tags && passes_uid_check;

        // The 'only_include_tags' could be checked here to speed it up

        if process_object {
            let (last_tags, last_version) = match last {
                None => (None, "".to_string()),
                Some(ref last) => {
                    ensure!(
                        sorted_objects(last, &curr) == Ordering::Less,
                        "Non sorted input"
                    );
                    if last.object_type() == curr.object_type() && last.id() == curr.id() {
                        (
                            Some(last.tags().collect::<HashMap<_, _>>()),
                            last.version().unwrap().to_string(),
                        )
                    } else {
                        (None, "".to_string())
                    }
                }
            };

            let curr_tags: BTreeMap<_, _> = curr.tags().collect();
            let mut keys: Vec<_> = curr_tags.keys().collect();
            if let Some(ref lt) = last_tags {
                keys.extend(lt.keys());
            }
            keys.sort();
            keys.dedup();

            let mut last_value;
            let mut curr_value;

            for key in keys.into_iter() {
                // Should we skip this tag?
                if only_include_tags
                    .as_ref()
                    .map_or(false, |only_include_tags| {
                        !only_include_tags.iter().any(|t| t == key)
                    })
                {
                    continue;
                }
                last_value = if let Some(ref lt) = last_tags {
                    lt.get(key).unwrap_or(&"")
                } else {
                    &""
                };
                curr_value = curr_tags.get(key).unwrap_or(&"");
                if last_value == curr_value {
                    continue;
                }

                trace!(
                    "Write tag change {} {:?} → {:?}",
                    key,
                    last_value,
                    curr_value
                );

                for (should_escape, field) in [
                    (true, key),
                    (true, curr_value),
                    (true, last_value),
                    (
                        false,
                        &format!("{:?}{}", curr.object_type(), curr.id()).as_str(),
                    ),
                    (false, &curr.version().unwrap().to_string().as_str()),
                    (false, &last_version.as_str()),
                    (
                        false,
                        &(match timestamp_format {
                            TimestampFormat::Datetime => {
                                curr.timestamp().as_ref().unwrap().to_iso_string()
                            }
                            TimestampFormat::EpochTime => curr
                                .timestamp()
                                .as_ref()
                                .unwrap()
                                .to_epoch_number()
                                .to_string(),
                        })
                        .as_str(),
                    ),
                    (true, &curr.user().unwrap()),
                    (false, &curr.uid().unwrap().to_string().as_str()),
                    (false, &curr.changeset_id().unwrap().to_string().as_str()),
                ]
                .iter()
                {
                    if *should_escape {
                        encode_field(field, &mut field_bytes, &mut utf8_bytes_buffer);
                    } else {
                        field_bytes.clear();
                        field_bytes.extend(field.bytes());
                    }

                    output.write_field(&field_bytes)?;
                }

                if let Some((changeset_tags, changeset_tag_lookup)) = changeset_tags.as_ref() {
                    match changeset_tag_lookup.tags(curr.changeset_id().unwrap())? {
                        None => {
                            trace!("No tags found for changeset {:?}", curr.changeset_id());
                            // no changeset found
                            for _ in 0..changeset_tags.len() {
                                output.write_field("")?;
                            }
                        }
                        Some(tags_for_changeset) => {
                            for changeset_tag in changeset_tags {
                                match tags_for_changeset
                                    .iter()
                                    .filter_map(
                                        |(k, v)| if k == changeset_tag { Some(v) } else { None },
                                    )
                                    .next()
                                {
                                    None => output.write_field("")?,
                                    Some(v) => output.write_field(v)?,
                                }
                            }
                        }
                    }
                }

                output.write_record(None::<&[u8]>)?;
            }
        }

        last = Some(curr);
        curr = match objects_iter.next() {
            None => {
                break;
            }
            Some(o) => o,
        };
    }

    info!(
        "Finished in {}",
        format_time(&(Instant::now() - started_processing))
    );
    Ok(())
}

fn encode_field(field: &str, bytes: &mut Vec<u8>, mut utf8_bytes_buffer: &mut Vec<u8>) {
    bytes.clear();

    for c in field.chars() {
        if c == '\t' {
            bytes.push(b'\\');
            bytes.push(b't');
        } else if c == '\n' {
            bytes.push(b'\\');
            bytes.push(b'n');
        } else {
            c.encode_utf8(&mut utf8_bytes_buffer);
            bytes.extend(&utf8_bytes_buffer[..c.len_utf8()]);
        }
    }
}

fn sorted_objects(a: &impl OSMObj, b: &impl OSMObj) -> std::cmp::Ordering {
    a.object_type()
        .cmp(&b.object_type())
        .then(a.id().cmp(&b.id()))
        .then(a.version().cmp(&b.version()))
}

pub fn format_time(duration: &std::time::Duration) -> String {
    let sec = duration.as_secs_f32().round() as u64;
    if sec < 60 {
        format!("{:2}s", sec)
    } else {
        let (min, sec) = (sec / 60, sec % 60);
        if min < 60 {
            format!("{:2}m{:02}s", min, sec)
        } else {
            let (hr, min) = (min / 60, min % 60);
            if hr < 24 {
                format!("{}h{:02}m{:02}s", hr, min, sec)
            } else {
                let (day, hr) = (hr / 24, hr % 24);
                format!("{}d{}h{:02}m{:02}s", day, hr, min, sec)
            }
        }
    }
}

struct ChangesetTagLookup {
    conn: Connection,
}

impl ChangesetTagLookup {
    fn from_filename(filename: &str) -> Result<Self> {
        let conn = Connection::open(filename)?;
        Ok(ChangesetTagLookup { conn })
    }

    fn tags(&self, cid: u32) -> Result<Option<Vec<(String, String)>>> {
        let res: Option<Vec<u8>> = self
            .conn
            .query_row(
                "select other_tags from changeset_tags where id = ?1;",
                [cid],
                |row| row.get(0),
            )
            .optional()?;
        match res {
            None => Ok(None),
            Some(tags) => {
                let tags: Vec<(String, String)> = serde_json::from_slice(&tags)?;
                Ok(Some(tags))
            }
        }
    }
}
