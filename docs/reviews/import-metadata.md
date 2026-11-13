# PR #5: instance metadata regression probes

Independent review of `e1845ae4da70528f9331130d0c83054ddb8e8ed0`.
The production importer is unchanged. These tests use synthetic local files,
without downloads, accounts or existing game instances.

```sh
cargo test --locked -p drakestar-mc
cargo test --locked -p drakestar-mc --test import_metadata_review -- \
  --include-ignored --skip fifo_preview_worker
```

The first command passes 38 unit tests and three controls; five entries are
explicitly ignored (four unresolved safety assertions and a subprocess
helper). The second command explicitly runs all safety assertions. The
original Prism results are below; the new JSON assertion is recorded separately:

| Input | Observed result |
|---|---|
| Regular small `instance.cfg` | Correct preview; no instance created |
| Sparse `instance.cfg` of 8 MiB + 1 byte | Preview accepts it despite the JSON metadata size limit |
| `instance.cfg` symbolic link to a synthetic file outside the source | Preview reads the external name |
| `instance.cfg` FIFO without a writer | Preview blocks; the test kills and reaps its child after one second |

FIFO and symbolic link probes run on Unix only. `mkfifo` is required for the
FIFO probe. The worker has a separate ten second startup ceiling and a ready
marker; it never blocks the main test process on the FIFO itself. Temporary
fixtures are removed after every probe.

Before merging the importer, require regular metadata files, reject links and
special files before opening them, and bound the actual read as well as the
reported size. Re-run the explicit safety assertions after that implementation
change and remove their ignores when they pass. Optional missing configuration
should still use the instance name fallback. These probes do not certify GUI
responsiveness, clean installation or the separate Modrinth download budget.

## Shared JSON reader: independent follow-up

The new `json_metadata_` tests cover `mmc-pack.json` (Prism),
`minecraftinstance.json` (CurseForge), and `modrinth.index.json` (Modrinth).
They do not repeat the earlier `instance.cfg` probes:

```sh
cargo test --offline --locked -p drakestar-mc --test import_metadata_review json_metadata_
cargo test --offline --locked -p drakestar-mc --test import_metadata_review \
  json_metadata_rejects_external_symlinks_in_all_formats -- --ignored --exact
```

On the same importer commit, the first command passes two tests, each covering
all three formats: regular JSON previews preserve the source and create no
instance; JSON files of 8 MiB + 1 byte fail with the size error before parsing.
The link assertion is explicitly ignored in that command. Its separate run
fails once and lists all three accepted external JSON links (exit 101).
Every link targets a synthetic sibling file; the target stays unchanged and
no instance or temporary fixture remains. The ignored assertion must be run
explicitly after a fix, then enabled when it passes.

`is_file()` and `metadata()` both follow symbolic links, so the existing JSON
size check does not enforce the intended regular-file/no-links policy. This
extends ticket #785 to the shared JSON reader; it does not demonstrate a FIFO
hang in JSON or fix the importer. A preview is not a full import or GUI test.
