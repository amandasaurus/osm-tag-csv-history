# osm-tag-csv-history

Use CSV tools to see who's mapping what in OpenStreetMap.

Given a OSM history file, it produces a CSV file, where each row refers to a
change (addition, removal or modification) to a tag all OSM objects in an OSM
data file with history.

## Getting data

[Planet.OpenStreetMap.org](https://planet.openstreetmap.org/planet/full-history/) provides a “full history” file, updated every week, where you can download the [latest full history file (⚠ 80+ GB! ⚠)](https://planet.openstreetmap.org/pbf/full-history/history-latest.osm.pbf), although it's quite large.

Geofabrik provides an [download service](https://osm-internal.download.geofabrik.de/) which includes full history files for lots of regions & countries. You must log into that with your OpenStreetMap account.

## Installation

If you have Rust installed, you can install it with:

    cargo install osm-tag-csv-history

You can download prebuild binary released from the [Github release page](https://github.com/rory/osm-tag-csv-history/releases), (e.g. [download the v0.1.0 release](https://github.com/rory/osm-tag-csv-history/releases/download/v0.1.0/osm-tag-csv-history)).

## Usage

    osm-tag-csv-history -i mydata.osm.pbf -o mydata.csv

## Example

Many programmes can use CSV files. It's also possible to use hacky unix command
line programmes to calculate who's adding fuel stations (`amenity=fuel` in OSM)
in Ireland:

    osm-tag-csv-history -i ~/osm/data/ireland-and-northern-ireland-internal.osh.pbf -o - --no-header | grep '^amenity,,fuel,' | cut -d, -f9 | sort | uniq -c | sort -n | tail -n 20

## Output file format

Records are separated by a `\n`. A header line is included by default, but it
can be turned off with `--no-header` (or forcibly included with `--header`).

### Columns

* `key` The tag key
* `old_value` The previous value. `""` (empty string) if the previous version
  didn't have this key
* `new_value` The current/new version. `""` (empty string) if the current
  version doesn't have this key (i.e. it has been removed from the object)
* `object_type` The object type. `n` for node, `w` way, `r` relation
* `id` The current/new object id
* `old_version` The previous version number. `""` (empty string) for the first version of an object
* `new_version` The current/new version number
* `datetime` Date time (RFC3339 UTC format) the object was created
* `username` The username of the user who changes it (remember: in OSM, users
  can change their username, UIDs remain constant)
* `uid` The user id of the user.
* `changeset_id` Changeset id where this change was made

### Example

Imagine this simple 

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

Running `osm-tag-csv-history` on it produces this CSV file (formatted here as a table):

| key         | old_value  | new_value   | object_type  | id  | old_version  | new_version  | datetime              | username  | uid  | changeset_id |
| ----------- | ---------- | ----------- | ------------ | --- | ------------ | ------------ | --------------------- | --------- | ---- | ------------ |
| name        |            | Nice City   | n            | 1   |              | 1            | 2019-01-01T00:00:00Z  | Alice     | 12   | 2            |
| place       |            | city        | n            | 1   |              | 1            | 2019-01-01T00:00:00Z  | Alice     | 12   | 2            |
| population  |            | 1000000     | n            | 1   | 1            | 2            | 2019-03-01T12:30:00Z  | Bob       | 2    | 10           |
| amenity     |            | restaurant  | n            | 2   |              | 1            | 2019-04-01T00:00:00Z  | Alice     | 12   | 20           |
| name        |            | TastyEats   | n            | 2   |              | 1            | 2019-04-01T00:00:00Z  | Alice     | 12   | 20           |
| cuisine     |            | regional    | n            | 2   | 1            | 2            | 2019-04-01T02:00:00Z  | Alice     | 12   | 21           |
| cuisine     | regional   | burger      | n            | 2   | 2            | 3            | 2019-04-01T03:00:00Z  | Alice     | 12   | 22           |
| amenity     |            | bench       | n            | 3   |              | 1            | 2019-04-01T00:00:00Z  | Alice     | 12   | 50           |
| amenity     | bench      |             | n            | 3   | 1            | 2            | 2019-06-01T00:00:00Z  | Alice     | 12   | 100          |


Some things to note:

* There can be more than one record (line) per version (n1 v1 has 2 lines, one for each tag that was added).
* If no tags are changed, then there are no lines. There is no line for nide 2 v4 because the location, not the tags was changed.
* An empty value for `old_version` means there was no previous, or earlier, version.
* When an object (and hence tag) is deleted, the previous value is in `old_value`, and the `new_value` is empty, as for n3 v2.

## Possible useful tools

The following other tools might be useful:

* [`xsv`](https://github.com/BurntSushi/xsv). a command line tool for slicing & filtering CSV data.
* [`osmium`](https://osmcode.org/osmium-tool/) a programme to process OSM data. You can use this to filter an OSM history file to a certain area, or time range.
* [`datamash`](https://www.gnu.org/software/datamash/), command line CSV statistical tool.

## Misc

Copyright 2020, GNU Affero General Public Licence (AGPL) v3 or later. See [LICENCE.txt](./LICENCE.txt).
Source code is on [Sourcehut](https://git.sr.ht/~ebel/osm-tag-csv-history), and [Github](https://github.com/rory/osm-tag-csv-history).

