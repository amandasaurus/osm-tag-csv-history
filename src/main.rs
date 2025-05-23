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
extern crate smallvec;
extern crate smol_str;

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;
use std::time::Instant;

use clap::{App, Arg};
use osmio::{OSMObj, OSMObjBase, OSMObjectType, OSMReader};

use anyhow::{Context, Result};
use flate2::Compression;
use flate2::write::GzEncoder;
use read_progress::ReaderWithSize;
use rusqlite::{Connection, OptionalExtension};
use smallvec::SmallVec;
use smol_str::SmolStr;

#[allow(clippy::upper_case_acronyms)]
enum OutputFormat {
    CSV,
    TSV,
}

#[derive(Debug, PartialEq)]
enum Column {
    Key,
    NewValue,
    OldValue,
    Value,
    Id,
    RawId,
    ObjectTypeShort,
    ObjectTypeLong,
    NewVersion,
    OldVersion,
    IsoDatetime,
    EpochDatetime,
    Username,
    Uid,
    ChangesetId,

    ChangesetTag(String),

    TagCountDelta,
    ValueCountDelta,
}

impl FromStr for Column {
    type Err = anyhow::Error;
    fn from_str(val: &str) -> Result<Self, Self::Err> {
        match val.to_lowercase().trim() {
            "key" => Ok(Column::Key),
            "new_value" => Ok(Column::NewValue),
            "old_value" => Ok(Column::OldValue),
            "value" => Ok(Column::Value),
            "id" => Ok(Column::Id),
            "raw_id" | "osm_raw_id" => Ok(Column::RawId),
            "new_version" => Ok(Column::NewVersion),
            "old_version" => Ok(Column::OldVersion),
            "datetime" | "iso_datetime" | "iso_timestamp" => Ok(Column::IsoDatetime),
            "epoch" | "epoch_datetime" | "epoch_timestamp" => Ok(Column::EpochDatetime),
            "username" => Ok(Column::Username),
            "uid" => Ok(Column::Uid),
            "changeset_id" => Ok(Column::ChangesetId),
            col if col.starts_with("changeset.") => Ok(Column::ChangesetTag(
                col.strip_prefix("changeset.").unwrap().to_string(),
            )),
            "tag_count_delta" => Ok(Column::TagCountDelta),
            "value_count_delta" => Ok(Column::ValueCountDelta),
            "object_type_short" | "osm_type_short" => Ok(Column::ObjectTypeShort),
            "object_type_long" | "osm_type_long" => Ok(Column::ObjectTypeLong),

            col => Err(anyhow::anyhow!("Unknown column value: {}", col)),
        }
    }
}

impl Column {
    fn is_changeset_tag(&self) -> bool {
        matches!(self, Column::ChangesetTag(_))
    }

    fn header(&self) -> Cow<str> {
        match self {
            Column::Key => "key".into(),
            Column::NewValue => "new_value".into(),
            Column::OldValue => "old_value".into(),
            Column::Value => "value".into(),
            Column::Id => "id".into(),
            Column::RawId => "raw_id".into(),
            Column::NewVersion => "new_version".into(),
            Column::OldVersion => "old_version".into(),
            Column::IsoDatetime => "iso_datetime".into(),
            Column::EpochDatetime => "epoch_datetime".into(),
            Column::Username => "username".into(),
            Column::Uid => "uid".into(),
            Column::ChangesetId => "changeset_id".into(),
            Column::ChangesetTag(t) => format!("changeset_{}", t).into(),
            Column::TagCountDelta => "tag_count_delta".into(),
            Column::ValueCountDelta => "value_count_delta".into(),
            Column::ObjectTypeShort => "object_type_short".into(),
            Column::ObjectTypeLong => "object_type_long".into(),
        }
    }
}

enum LineType {
    OldNewValue,
    SeparateLines,
}

fn main() -> Result<()> {
    let matches = App::new("osm-tag-csv-history")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Create a CSV file detailing tagging changes in an OSM file")

        .arg(Arg::with_name("input")
             .short('i').long("input")
             .value_name("INPUT.osh.pbf")
             .help("Input file to convert.")
             .long_help("Read OSM data from this file. If it's a .osh.pbf history file, the full history will be output. Regular non-history files can be processed too")
             .takes_value(true).required(true)
             )

        .arg(Arg::with_name("output")
             .short('o').long("output")
             .value_name("OUTPUT.csv[.gz]")
             .help("Where to write the output. Use - for stdout. with auto compression (default), if this file ends with .gz, then it will be gzip compressed")
             .takes_value(true).required(true)
             )

        .arg(Arg::with_name("verbosity")
             .short('v').multiple_occurrences(true)
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
             .short('c').long("compression")
             .takes_value(true).required(false)
             .possible_values(["none", "auto", "gzip"])
             .hidden_short_help(true)
             .default_value("auto")
             .value_name("{none,auto,gzip}")
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

        .arg(Arg::with_name("key")
             .short('k').long("k")
             .value_name("KEY")
             .takes_value(true).required(false)
             .multiple(true).number_of_values(1)
             .help("Only include changes to this tag key (can be specified multiple times)")
             )
        .arg(Arg::with_name("tag")
             .short('t').long("tag")
             .value_name("KEY=VALUE")
             .takes_value(true).required(false)
             .multiple(true).number_of_values(1)
             .help("Only include changes with this KEY & VALUE (can be specified multiple times)")
             )


        .arg(Arg::with_name("changeset_filename")
             .long("changesets")
             .value_name("changesets-latest.osm.bz2")
             .takes_value(true).required(false)
             .help("Filename of the changeset file")
             )

        .arg(Arg::with_name("uid")
             .long("uid")
             .value_name("USERID")
             .takes_value(true).required(false)
             .multiple(true).number_of_values(1)
             .help("Only include changes made by this OSM user (by userid)")
             )


        .arg(Arg::with_name("output_format")
             .long("output-format")
             .takes_value(true).required(false)
             .help("output format")
             .possible_values(["auto", "csv", "tsv"])
             .hidden_short_help(true)
             .default_value("auto")
             )

        .arg(Arg::with_name("columns")
             .short('C').long("columns")
             .value_name("COL,COL,...")
             .takes_value(true).required(false)
             .default_value("key,new_value,old_value,id,new_version,old_version,tag_count_delta,iso_datetime,username,uid,changeset_id")
             .long_help("Output the following columns, in order:
    key: Tag key
    new_value: Old value of the tag
    old_value: New value of the tag
    id: OSM object type & id (e.g. w123)
    raw_id: just the numeric if
    object_type_short, osm_type_short: N, W, R for the object
    object_type_long, osm_type_long: node, way, relation for the object
    new_version: Old version number
    old_version: New version number:
    iso_datetime, datetime, iso_timestamp: ISO Timestamp of the new object
    epoch, epoch_datetime, epoch_timestamp: Unix Epoch timestamp (seconds since 1 Jan 1970) of the new object
    username: Username of the new object
    uid: UID of new object.
    changeset_id: Changeset ID of the new object
    changeset.TAG: TAG of the changeset 
    tag_count_delta: What is the totaly change to the number
                ")
             )

        .arg(Arg::with_name("object-types")
             .short('T').long("object-types")
             .value_name("[nwr]")
             .help("Only include these OSM Object types")
             .long_help("Only include these OSM Object types. Specify a letter for each type (n)ode/(w)way/(r)elation, e.g. -T wr = only ways & relations")
             .takes_value(true).required(false)
             .default_value("nwr")
             )

        .arg(Arg::with_name("line-type")
             .long("line-type")
             .takes_value(true)
             .value_parser(["oldnew", "separate"])
             .default_value("oldnew")
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
    info!("Begining processing of {}", input_path);

    let log_frequency: f32 = matches.value_of("log-frequency").unwrap().parse()?;

    let file =
        File::open(input_path).with_context(|| format!("opening input file {}", input_path))?;
    let mut osm_obj_reader =
        osmio::pbf::PBFReader::new(BufReader::new(ReaderWithSize::from_file(file)?));
    let mut objects_iter = osm_obj_reader.objects();

    let only_include_keys: SmallVec<[SmolStr; 2]> = matches
        .get_many::<SmolStr>("key")
        .into_iter()
        .flatten()
        .cloned()
        .collect();

    let only_include_tags: SmallVec<[(SmolStr, SmolStr); 2]> = matches
        .get_many("tag")
        .into_iter()
        .flatten()
        .map(|kv: &String| {
            let mut parts = kv.splitn(2, "=").map(SmolStr::from);
            (parts.next().unwrap(), parts.next().unwrap())
        })
        .collect();

    let only_include_uids: Option<SmallVec<[u32; 1]>> = match matches.values_of("uid") {
        None => None,
        Some(vals) => Some(vals.map(|u| Ok(u.parse()?)).collect::<Result<_>>()?),
    };

    let only_include_types = match matches.value_of("object-types") {
        None => (true, true, true),
        Some(object_types) => {
            let object_types = object_types.to_lowercase();
            (
                object_types.contains('n'),
                object_types.contains('w'),
                object_types.contains('r'),
            )
        }
    };

    let columns: SmallVec<[Column; 12]> = matches
        .value_of("columns")
        .unwrap()
        .split(',')
        .map(|col_str| col_str.parse())
        .collect::<Result<_>>()?;
    debug!("columns: {:?}", columns);

    let line_type = if columns.iter().any(|c| *c == Column::ValueCountDelta) {
        LineType::SeparateLines
    } else {
        LineType::OldNewValue
    };

    if !only_include_tags.is_empty() {
        info!(
            "Only including changes to these {} tag(s): {:?}",
            only_include_tags.len(),
            only_include_tags
        );
    }
    if !only_include_keys.is_empty() {
        info!(
            "Only including changes to these {} keys(s): {:?}",
            only_include_keys.len(),
            only_include_keys
        );
    }

    if let Some(only_include_uids) = only_include_uids.as_ref() {
        info!(
            "Only including changes made by user id {:?}",
            only_include_uids
        );
    }

    // MUST be replaced with above columns
    // changesets?
    let changeset_lookup = if columns.iter().any(Column::is_changeset_tag) {
        let lookup =
            ChangesetTagLookup::from_filename(matches.value_of("changeset_filename").unwrap())?;
        debug!(
            "Reading changeset sqlite from {}",
            matches.value_of("changeset_filename").unwrap()
        );
        Some(lookup)
    } else {
        None
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

    let output_format = match (
        matches.value_of("output_format"),
        matches.value_of("output"),
    ) {
        (Some("csv"), _) => OutputFormat::CSV,
        (Some("tsv"), _) => OutputFormat::TSV,
        (Some("auto"), Some("-")) => OutputFormat::CSV,
        (Some("auto"), Some(filename)) if filename.starts_with("/dev/fd/") => OutputFormat::CSV,
        (Some("auto"), Some(filename))
            if filename.ends_with(".csv") || filename.ends_with(".csv.gz") =>
        {
            OutputFormat::CSV
        }
        (Some("auto"), Some(filename))
            if filename.ends_with(".tsv") || filename.ends_with(".tsv.gz") =>
        {
            OutputFormat::TSV
        }
        (format, filename) => unreachable!(
            "Unable to determine output format: format={:?} filename={:?}",
            format, filename
        ),
    };

    let output_path = matches.value_of("output").unwrap();
    let output_writer: Box<dyn std::io::Write> = if output_path == "-" {
        Box::new(std::io::stdout())
    } else {
        Box::new(File::create(matches.value_of("output").unwrap())?)
    };
    let output_writer = match matches.value_of("compression") {
        Some("auto") => {
            if output_path == "-" || output_path.starts_with("/dev/fd/") {
                // stdout, so no compression
                trace!("Output is '-' or a FD, no compression");
                output_writer
            } else if output_path.ends_with(".csv.gz") || output_path.ends_with(".tsv.gz") {
                trace!("Output file ends with .[ct]sv.gz so using regular gzip");
                Box::new(GzEncoder::new(output_writer, Compression::default()))
            } else if output_path.ends_with(".csv") || output_path.ends_with(".tsv") {
                // uncompressed
                trace!("Output file ends with .[ct]sv so no compression");
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
    let mut output = csv::WriterBuilder::new();
    match output_format {
        OutputFormat::CSV => {}
        OutputFormat::TSV => {
            output.delimiter(b'\t');
        }
    }
    let mut output = output.from_writer(output_writer);

    if include_header {
        trace!("Writing CSV header");
        for c in columns.iter() {
            output.write_field(c.header().as_ref())?;
        }

        output.write_record(None::<&[u8]>)?;
    }

    let mut curr = objects_iter.next().unwrap();
    let mut last: Option<osmio::obj_types::StringOSMObj> = None;

    let mut num_objects = 0;

    let mut time_counter = do_every::DoEvery::new();

    let mut field_bytes = Vec::with_capacity(25);
    let mut utf8_bytes_buffer = vec![0; 4];
    let started_processing = Instant::now();
    let mut passes_uid_check;
    let mut passes_type_check;

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

        passes_type_check = matches!(
            (curr.object_type(), only_include_types),
            (OSMObjectType::Node, (true, _, _))
                | (OSMObjectType::Way, (_, true, _))
                | (OSMObjectType::Relation, (_, _, true))
        );

        let has_tags = match last {
            None => curr.tagged(),
            Some(ref l) => l.tagged() || curr.tagged(),
        };
        let process_object = has_tags && passes_uid_check && passes_type_check;

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

            let mut last_value: &str;
            let mut last_value_existed;
            let mut curr_value: &str;
            let mut curr_value_exists;

            for key in keys.into_iter() {
                // Should we skip this tag?
                if !only_include_keys.is_empty() && !only_include_keys.iter().any(|k| key == k) {
                    continue;
                }
                if let Some(&value) = last_tags.as_ref().and_then(|lt| lt.get(key)) {
                    last_value = value;
                    last_value_existed = true;
                } else {
                    last_value = "";
                    last_value_existed = false;
                };

                if let Some(value) = curr_tags.get(key) {
                    curr_value = value;
                    curr_value_exists = true;
                } else {
                    curr_value = "";
                    curr_value_exists = false;
                };
                if last_value == curr_value {
                    continue;
                }
                //dbg!(key); dbg!(last_value); dbg!(curr_value);
                //dbg!(&only_include_tags);
                if !only_include_tags.is_empty()
                    && !only_include_tags
                        .iter()
                        .any(|(k, v)| k == key && (v == last_value || v == curr_value))
                {
                    continue;
                }

                trace!(
                    "Write tag change {} {:?} → {:?} ({}→{})",
                    key, last_value, curr_value, last_value_existed, curr_value_exists,
                );

                let mut i: u8 = 0;

                loop {
                    match (&line_type, i) {
                        (LineType::OldNewValue, 0) => {}
                        (LineType::OldNewValue, 1) => {
                            break;
                        }
                        (LineType::OldNewValue, _) => {
                            unreachable!()
                        }
                        (LineType::SeparateLines, 0) => {
                            if !last_value_existed {
                                i += 1;
                                continue;
                            }
                        }
                        (LineType::SeparateLines, 1) => {
                            if !curr_value_exists {
                                i += 1;
                                continue;
                            }
                        }
                        (LineType::SeparateLines, 2) => {
                            break;
                        }
                        (LineType::SeparateLines, _) => {
                            unreachable!()
                        }
                    }

                    for column in columns.iter() {
                        field_bytes.clear();
                        match column {
                            Column::Key => {
                                encode_field(key, &mut field_bytes, &mut utf8_bytes_buffer);
                            }
                            Column::NewValue => {
                                encode_field(curr_value, &mut field_bytes, &mut utf8_bytes_buffer);
                            }
                            Column::OldValue => {
                                encode_field(last_value, &mut field_bytes, &mut utf8_bytes_buffer);
                            }
                            Column::Value => {
                                encode_field(
                                    match i {
                                        0 => last_value,
                                        1 => curr_value,
                                        _ => unreachable!(),
                                    },
                                    &mut field_bytes,
                                    &mut utf8_bytes_buffer,
                                );
                            }
                            Column::Id => {
                                field_bytes.extend(
                                    format!("{:?}{}", curr.object_type(), curr.id())
                                        .as_str()
                                        .bytes(),
                                );
                            }
                            Column::RawId => {
                                field_bytes.extend(curr.id().to_string().as_str().bytes())
                            }
                            Column::NewVersion => {
                                field_bytes.extend(curr.version().unwrap().to_string().bytes());
                            }
                            Column::OldVersion => {
                                field_bytes.extend(last_version.as_str().bytes());
                            }
                            Column::IsoDatetime => {
                                field_bytes.extend(
                                    curr.timestamp().as_ref().unwrap().to_iso_string().bytes(),
                                );
                            }
                            Column::EpochDatetime => {
                                field_bytes.extend(
                                    curr.timestamp()
                                        .as_ref()
                                        .unwrap()
                                        .to_epoch_number()
                                        .to_string()
                                        .bytes(),
                                );
                            }
                            Column::Username => {
                                encode_field(
                                    curr.user().unwrap(),
                                    &mut field_bytes,
                                    &mut utf8_bytes_buffer,
                                );
                            }
                            Column::Uid => {
                                field_bytes.extend(curr.uid().unwrap().to_string().bytes());
                            }
                            Column::ChangesetId => {
                                field_bytes
                                    .extend(curr.changeset_id().unwrap().to_string().bytes());
                            }
                            Column::ObjectTypeShort => {
                                field_bytes.extend(match curr.object_type() {
                                    OSMObjectType::Node => b"n",
                                    OSMObjectType::Way => b"w",
                                    OSMObjectType::Relation => b"r",
                                });
                            }
                            Column::ObjectTypeLong => {
                                field_bytes.extend(match curr.object_type() {
                                    OSMObjectType::Node => b"node".iter(),
                                    OSMObjectType::Way => b"way".iter(),
                                    OSMObjectType::Relation => b"relation".iter(),
                                });
                            }
                            Column::ChangesetTag(changeset_tag) => {
                                match changeset_lookup
                                    .as_ref()
                                    .unwrap()
                                    .tags(curr.changeset_id().unwrap())?
                                {
                                    None => {
                                        trace!(
                                            "No tags found for changeset {:?}",
                                            curr.changeset_id()
                                        );
                                    }
                                    Some(tags_for_changeset) => {
                                        if let Some(v) = tags_for_changeset
                                            .iter()
                                            .filter_map(|(k, v)| {
                                                if k == changeset_tag { Some(v) } else { None }
                                            })
                                            .next()
                                        {
                                            field_bytes.extend(v.bytes());
                                        }
                                    }
                                }
                            }
                            Column::TagCountDelta => {
                                field_bytes.extend(match (last_value_existed, curr_value_exists) {
                                    (false, false) => unreachable!(),
                                    (false, true) => b"+1".iter(),
                                    (true, false) => b"-1".iter(),
                                    (true, true) => b"0".iter(),
                                });
                            }

                            Column::ValueCountDelta => {
                                field_bytes.extend(match i {
                                    0 => b"-1".iter(),
                                    1 => b"+1".iter(),
                                    _ => unreachable!(),
                                });
                            }
                        }
                        output.write_field(&field_bytes)?;
                    }

                    output.write_record(None::<&[u8]>)?;

                    i += 1;
                }
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

fn encode_field(field: &str, bytes: &mut Vec<u8>, utf8_bytes_buffer: &mut [u8]) {
    bytes.clear();

    for c in field.chars() {
        if c == '\t' {
            bytes.push(b'\\');
            bytes.push(b't');
        } else if c == '\n' {
            bytes.push(b'\\');
            bytes.push(b'n');
        } else {
            c.encode_utf8(utf8_bytes_buffer);
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
