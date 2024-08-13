## Quick start

From the project root, run:

```
# Fetch git submodules
git submodule init
git submodule update

# Download the Metropolitan Museum of Art open access CSV
curl https://media.githubusercontent.com/media/metmuseum/openaccess/master/MetObjects.csv --output rust/cache/MetObjects.csv

# TODO: Download Wikidata CSV - I need to put this somewhere publicly accessible!

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
