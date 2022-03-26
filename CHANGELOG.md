# v0.5.0 - 26 March 2022

* Output in TSV format possible
* Output columns can now be specified on the command line, and reordered. The
  method to specify epoch timestamps & changeset tags has been changed
* New output columns:
  * `tag_count_delta` which can easy tell you if the tag was
     added, removed, or merely changed
  * `raw_id` ID of the object just as a number
  * `object_type_short` & `object_type_long` the object type (either `n`/`w`/`r`, or `node`/`way`/`relation`)
* Can now filter by object type with `-T nwr`/`--object-type nwr`

# v0.4.0 - 8 November 2021

* Add `--tag` to only show changes that affect specific OSM tags. Useful to
  create smaller CSV files
* Add `--changeset-tag` to include a column for the tag of the changeset that
  made that change, which needs a pre-processed changeset file created by `osmio`.
* Update `osmio` dependency
* Improvments to `--help` (etc.) output

# v0.3.0 - 21 Jan 2020

* The output timestamp can be switched to unix epoch timestamp format, which is
  ~15+ faster. Default is still the regular RFC3339 format

* Improvements to info messages printed to user:

 * Use `--log-frequency SEC` to control how often to print status message
 * Info message at end with how long it took to run
 * Progress messages include the ETA & how long it'll take to run

* Internal refactorings, resulting in increased performance

# v0.2.0 - 11 Jan 2020

* Escape newlines etc.
* Reorder columns, swapping `new_value` & `old_value`, & `new_version` & `old_version`
* New `id` column format, merging object type & id

# v0.1.0 - 6 Jan 2020

* Initial version
