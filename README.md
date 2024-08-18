## Quick start

You will need to install:
* [Git LFS](https://git-lfs.com/)
* [Godot 4.3](https://godotengine.org/)
* [Rust 1.80.1](https://www.rust-lang.org/)

From the project root, run:

```
# This is only really needed if you installed Git LFS *after* cloning the repo.
git lfs fetch
git lfs checkout

# Download the Metropolitan Museum of Art open access CSV
curl https://media.githubusercontent.com/media/metmuseum/openaccess/master/MetObjects.csv --output rust/cache/MetObjects.csv

# Unzip the Wikidata CSV.
unzip rust/cache/WikidataObjects.zip -d rust/cache/

sh build-initial-db.sh
sh build-plugin.sh
```

Now you can open the Godot project and open it in the editor:

```
cd ..
godot -e
```

## Exporting the project

From the project root, run:

```
sh build-initial-db.sh

# For macOS
godot --export-debug "macOS"

# For windows
godot --export-debug "Windows Desktop"
```

The exported project will be in the `dist/files` directory.
