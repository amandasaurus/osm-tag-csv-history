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
