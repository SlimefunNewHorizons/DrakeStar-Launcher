# PR #5: Prism metadata regression probes

Independent review of `e1845ae4da70528f9331130d0c83054ddb8e8ed0`.
The production importer is unchanged. These tests use synthetic local files,
without downloads, accounts or existing game instances.

```sh
cargo test --locked -p drakestar-mc
cargo test --locked -p drakestar-mc --test import_metadata_review -- \
  --include-ignored --skip fifo_preview_worker
```

The first command passes 38 unit tests and one new control; four entries are
explicitly ignored (three unresolved safety assertions and their subprocess
helper). The second command intentionally reproduces three failures on this
commit, with one passing control:

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
