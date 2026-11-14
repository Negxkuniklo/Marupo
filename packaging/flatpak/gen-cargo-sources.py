#!/usr/bin/env python3
"""Regenerates packaging/flatpak/cargo-sources.json from Cargo.lock."""

import json
import pathlib
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
LOCK_PATH = ROOT / "Cargo.lock"
OUT_PATH = pathlib.Path(__file__).resolve().parent / "cargo-sources.json"


def main() -> None:
    lock_data = tomllib.loads(LOCK_PATH.read_text(encoding="utf-8"))
    sources: list[dict] = []

    for pkg in lock_data.get("package", []):
        if pkg.get("source") != "registry+https://github.com/rust-lang/crates.io-index":
            continue
        name = pkg["name"]
        version = pkg["version"]
        checksum = pkg["checksum"]
        dest = f"cargo/vendor/{name}-{version}"

        sources.append(
            {
                "type": "archive",
                "archive-type": "tar-gzip",
                "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
                "sha256": checksum,
                "dest": dest,
            }
        )
        sources.append(
            {
                "type": "inline",
                "contents": json.dumps({"files": {}, "package": checksum}),
                "dest": dest,
                "dest-filename": ".cargo-checksum.json",
            }
        )

    sources.append(
        {
            "type": "inline",
            "contents": (
                '[source.vendored-sources]\n'
                'directory = "cargo/vendor"\n\n'
                '[source.crates-io]\n'
                'replace-with = "vendored-sources"\n'
            ),
            "dest": ".cargo",
            "dest-filename": "config.toml",
        }
    )

    OUT_PATH.write_text(json.dumps(sources, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {len(sources)} entries ({(len(sources) - 1) // 2} crates) to {OUT_PATH}")


if __name__ == "__main__":
    main()
