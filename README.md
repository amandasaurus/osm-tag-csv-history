# osm-tag-csv-history

Use CSV tools to see who's mapping what in OpenStreetMap.

Given a OSM history file, it produces a CSV file, where each row refers to a
change (addition, removal or modification) to a tag all OSM objects in an OSM
data file with history.

## Getting data

[Planet.OpenStreetMap.org](https://planet.openstreetmap.org/planet/full-history/)
provides a “full history” file, updated every week, where you can download the
[latest full history file (⚠ 99+ GB!
⚠)](https://planet.openstreetmap.org/pbf/full-history/history-latest.osm.pbf),
although it's quite large.

Download it over BitTorrent with:

	aria2c --seed-time 0 https://planet.openstreetmap.org/pbf/full-history/history-latest.osm.pbf.torrent

Geofabrik provides an [download
service](https://osm-internal.download.geofabrik.de/) which includes full
history files for lots of regions & countries. You must log into that with your
OpenStreetMap account. You can also use this tool on regular, non-history, OSM
data files.

## Installation

If you have Rust installed, you can install it with:

    cargo install osm-tag-csv-history

You can download prebuild binary released from the [Github release page](https://github.com/amandasaurus/osm-tag-csv-history/releases), (e.g. [download the v0.3.0 release](https://github.com/amandasaurus/osm-tag-csv-history/releases/download/v0.3.0/osm-tag-csv-history)).

## Usage

    osm-tag-csv-history -i mydata.osm.pbf -o mydata.csv.gz

The output is automatically compressed with gzip if the file ends in `.gz`.

### Tag Filtering

By default, all tag changes are included. With the `--tag`/`-t` argument, only any changes to those tags are included in the output

To produce a CSV with only changes to the `highway` or `building` tag, run this command

    osm-tag-csv-history -i mydata.osm.pbf -o mydata.csv -t highway -t building


### Changeset tag column


### Example

Many programmes can use CSV files. It's also possible to use hacky unix command
line programmes to calculate who's adding fuel stations (`amenity=fuel` in OSM)
in Ireland:

    osm-tag-csv-history -i ./ireland-and-northern-ireland-internal.osh.pbf -o - --no-header | grep '^amenity,fuel,' | cut -d, -f9 | sort | uniq -c | sort -n | tail -n 20


Here can find all times someone has upgraded a building from `building=yes` to
something else.

    osm-tag-csv-history -i data.osh.pbf -o - --no-header | grep -P '^building,[^,]+,yes,' | cat -n
    
And with some other command line commands, we can get a list of who's doing the
most to make OSM more descriptive by upgrading `building=yes`.

    osm-tag-csv-history -i data.osh.pbf -o - --no-header | grep -P '^building,[^,]+,yes,' | xsv select 8 | sort | uniq -c | sort -n | tail -n 20

#### Using with `osmium getid`

The `id` column (column 4) can be used [by `osmium-tool` to filter an OSM file by object id](https://osmcode.org/osmium-tool/manual.html#getting-osm-objects-by-id). This is how you get a file of all the pet shops in OSM in a file:

    osm-tag-csv-history -i country-latest.osm.pbf -o - --no-header | grep '^shop,pet,' | xsv select 4 | osmium getid -i - country-latest.osm.pbf -o pets.osm.pbf -r

(For this simple case, [`osmiums`'s tag
filtering](https://osmcode.org/osmium-tool/manual.html#filtering-by-tags) is
probably better)

### Non-history files

This programme can run on non-history files just fine. The `old_value`, and
`old_version` will be empty. This can be a way to convert OSM data into CSV
format for further processing.

### Using on privacy preserving files.

The [Geofabrik Public Download Service](http://download.geofabrik.de/) provides
non-history files which do not include some metadata, like usernames, uids or
changeset\_ids. This tool can run on them and just give an empty value for
username, and `0` for uid & changeset\_id.

If you have an OSM account, you can get full metada from the
[internal](https://osm-internal.download.geofabrik.de/index.html) service.

## Output file format

Records are separated by a newline (`\n`). A header line is included by default, but it
can be turned off with `--no-header` (or forcibly included with `--header`).

If any string (e.g. tag value, username) has a newline or characters like that,
it will be escaped with a backslash (i.e. a newline is written as 2 characters,
`\` then `n`).

### Columns

(in order)

* `key` The tag key
* `new_value` The current/new version. `""` (empty string) if the current
  version doesn't have this key (i.e. it has been removed from the object)
* `old_value` The previous value. `""` (empty string) if the previous version
  didn't have this key
* `id` The object type and id. First character is the type (`n`/`w`/`r`), then
  the id. `n123` is node with id 123. This format is used [by `osmium-tool` to filter an OSM file by object id](https://osmcode.org/osmium-tool/manual.html#getting-osm-objects-by-id)
* `new_version` The current/new version number
* `old_version` The previous version number. `""` (empty string) for the first version of an object
* Either:
    * `datetime` Date time (RFC3339 format in UTC) the object was created. Default, or if `--timestamp-format dateime` was used.
    * `epoch_time` Date time (Unix epoch time) the object was created. Only if `--timestamp-format epoch_time` was used.
* `username` The username of the user who changes it (remember: in OSM, users
  can change their username, UIDs remain constant)
* `uid` The user id of the user.
* `changeset_id` Changeset id where this change was made

### Timestamp format

One column contains the timestamp. Be default, the column will be the timestamp
in RFC3339 (a subset of ISO 8601 format), i.e. `YYYY-MM-DDTHH:MM:SSZ`.

With `--timestampformat epoch_time`, the column will be called `epoch_time`,
and the timestamp will be in unix epoch time. This is how the data is stored in
an OSM PBF file. This makes processing about 15% faster.

### Example

Imagine this simple file:

```xml
<?xml version='1.0' encoding='UTF-8'?>
<osm version="0.6" generator="osmium/1.7.1">
  <node id="1" version="1" timestamp="2019-01-01T00:00:00Z" lat="0.0" lon="0.0" user="Alice" uid="12" changeset="2">
      <tag k="place" v="city"/>
      <tag k="name" v="Nice City"/>
  </node>
  <node id="1" version="2" timestamp="2019-03-01T12:30:00Z" lat="0.0" lon="0.0" user="Bob" uid="2" changeset="10">
      <tag k="place" v="city"/>
      <tag k="name" v="Nice City"/>
      <tag k="population" v="1000000"/>
  </node>
  <node id="2" version="1" timestamp="2019-04-01T00:00:00Z" lat="0.0" lon="0.0" user="Alice" uid="12" changeset="20">
      <tag k="amenity" v="restaurant"/>
      <tag k="name" v="TastyEats"/>
  </node>
  <node id="2" version="2" timestamp="2019-04-01T02:00:00Z" lat="0.0" lon="0.0" user="Alice" uid="12" changeset="21">
      <tag k="amenity" v="restaurant"/>
      <tag k="name" v="TastyEats"/>
      <tag k="cuisine" v="regional"/>
  </node>
  <node id="2" version="3" timestamp="2019-04-01T03:00:00Z" lat="0.0" lon="0.0" user="Alice" uid="12" changeset="22">
      <tag k="amenity" v="restaurant"/>
      <tag k="name" v="TastyEats"/>
      <tag k="cuisine" v="burger"/>
  </node>
  <node id="2" version="4" timestamp="2019-04-01T03:00:00Z" lat="1.0" lon="0.0" user="Alice" uid="12" changeset="22">
      <tag k="amenity" v="restaurant"/>
      <tag k="name" v="TastyEats"/>
      <tag k="cuisine" v="burger"/>
  </node>
  <node id="3" version="1" timestamp="2019-04-01T00:00:00Z" lat="0.0" lon="0.0" user="Alice" uid="12" changeset="50">
      <tag k="amenity" v="bench"/>
  </node>
  <node id="3" version="2" timestamp="2019-06-01T00:00:00Z" lat="0.0" lon="0.0" user="Alice" uid="12" changeset="100" visible="false">
  </node>
</osm>
```

NB: This programme cannot read XML files, only PBF. This file was converted to PBF with `osmium cat example.osm.xml -o example.osm.pbf`.

Running `osm-tag-csv-history` on it produces this CSV file (formatted here as a table by with [`csvtomd`](https://github.com/mplewis/csvtomd)):

key         |  new_value   |  old_value  |  id  |  new_version  |  old_version  |  datetime              |  username  |  uid  |  changeset_id
------------|--------------|-------------|------|---------------|---------------|------------------------|------------|-------|--------------
name        |  Nice City   |             |  n1  |  1            |               |  2019-01-01T00:00:00Z  |  Alice     |  12   |  2
place       |  city        |             |  n1  |  1            |               |  2019-01-01T00:00:00Z  |  Alice     |  12   |  2
population  |  1000000     |             |  n1  |  2            |  1            |  2019-03-01T12:30:00Z  |  Bob       |  2    |  10
amenity     |  restaurant  |             |  n2  |  1            |               |  2019-04-01T00:00:00Z  |  Alice     |  12   |  20
name        |  TastyEats   |             |  n2  |  1            |               |  2019-04-01T00:00:00Z  |  Alice     |  12   |  20
cuisine     |  regional    |             |  n2  |  2            |  1            |  2019-04-01T02:00:00Z  |  Alice     |  12   |  21
cuisine     |  burger      |  regional   |  n2  |  3            |  2            |  2019-04-01T03:00:00Z  |  Alice     |  12   |  22
amenity     |  bench       |             |  n3  |  1            |               |  2019-04-01T00:00:00Z  |  Alice     |  12   |  50
amenity     |              |  bench      |  n3  |  2            |  1            |  2019-06-01T00:00:00Z  |  Alice     |  12   |  100

Some things to note:

* There can be more than one record (line) per version (n1 v1 has 2 lines, one for each tag that was added).
* If no tags are changed, then there are no lines. There is no line for node 2 v4 because the location, not the tags was changed.
* An empty value for `old_version` means there was no previous, or earlier, version.
* When an object (and hence tag) is deleted, the previous value is in `old_value`, and the `new_value` is empty, as for n3 v2.

## Possible useful tools

The following other tools might be useful:

* [`xsv`](https://github.com/BurntSushi/xsv). a command line tool for slicing & filtering CSV data.
* [`osmium`](https://osmcode.org/osmium-tool/) a programme to process OSM data. You can use this to filter an OSM history file to a certain area, or time range.
* [`datamash`](https://www.gnu.org/software/datamash/), command line CSV statistical tool.

## Misc

Copyright 2020, GNU Affero General Public Licence (AGPL) v3 or later. See [LICENCE.txt](./LICENCE.txt).
Source code is on [Sourcehut](https://git.sr.ht/~ebel/osm-tag-csv-history), and [Github](https://github.com/amandasaurus/osm-tag-csv-history).

The output file should be viewed as a Derived Database of the OpenStreetMap database, and hence under the [ODbL 1.0](https://opendatacommons.org/licenses/odbl/) licence, the same as the [OpenStreetMap copyright](https://www.openstreetmap.org/copyright)
