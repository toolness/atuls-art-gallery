## Quick start

From the project root, run:

```
cd rust

# Download the Metropolitan Museum of Art open access CSV
curl https://media.githubusercontent.com/media/metmuseum/openaccess/master/MetObjects.csv --output cache/MetObjects.csv

# Import the CSV into sqlite (--release makes it very fast)
cargo run --release csv

# Lay out the art gallery
cargo run layout

# Build the Godot extension
cargo build
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
