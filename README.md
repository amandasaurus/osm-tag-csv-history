# osm-tag-csv-history

Use CSV tools to see who's mapping what in OpenStreetMap.

Given a OSM history file, it produces a CSV file, where each row refers to a
change (addition, removal or modification) to a tag all OSM objects in an OSM
data file with history.

## Getting data

[Planet.OpenStreetMap.org](https://planet.openstreetmap.org/planet/full-history/) provides a “full history” file, updated every week, where you can download the [latest full history file (⚠ 80+ GB! ⚠)](https://planet.openstreetmap.org/pbf/full-history/history-latest.osm.pbf), although it's quite large.

Geofabrik provides an [download service](https://osm-internal.download.geofabrik.de/) which includes full history files for lots of regions & countries. You must log into that with your OpenStreetMap account.

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

## Possible useful tools

The following other tools might be useful:

* [`xsv`](https://github.com/BurntSushi/xsv). a command line tool for slicing & filtering CSV data.
* [`osmium`](https://osmcode.org/osmium-tool/) a programme to process OSM data. You can use this to filter an OSM history file to a certain area, or time range.
* [`datamash`](https://www.gnu.org/software/datamash/), command line CSV statistical tool.

## Misc

Copyright 2020, GNU Affero General Public Licence (AGPL) v3 or later. See [LICENCE.txt](./LICENCE.txt).
Source code is on [Sourcehut](https://git.sr.ht/~ebel/osm-tag-csv-history)

