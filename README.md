## Introduction

_Atul's Art Gallery_ is a virtual art gallery that you can curate with your
friends. It offers access to the following public domain art collections:

* The [Open Access collection of the Metropolitan Museum of Art](https://www.metmuseum.org/about-the-met/policies-and-documents/open-access)

* The [Sum of all Paintings](https://www.wikidata.org/wiki/Wikidata:WikiProject_sum_of_all_paintings) from Wikidata

In total, about 171,000 works of art are accessible.

## Motivation

In [How to Enjoy Art][], Ben Street writes:

> ... art speaks in space. Stepping backwards to take something in, or moving
> forwards to get a better look, are both automatic, unthinking responses to
> a work of art's scale. When we do those things, we are already involved in
> making meaning from works of art.

<!-- The obove passage is from page 40. -->

Street goes on to compare a work of art's scale to its tone of voice:

> It might even be compared to a literal voice, whispering and delicate
> for the [6.6 x 5.2 cm] Hilliard, or loud and dynamic, like a speech
> at a political rally, for the Statue of Liberty.

<!-- The above passage is from page 56. -->

Yet our experience of reproductions of works of art online is largely
disconnected from this voice: two-dimensional web pages have no sense
of scale.

This project is an experiment that situates public domain art in a
virtual gallery that the viewer can explore from a first-person
perspective, similar to that of a video game. Because the viewer
has a physical representation in the virtual world, as does the
artwork, this aspect of the artwork's "voice" can be restored.

[How to Enjoy Art]: https://yalebooks.yale.edu/book/9780300267617/how-to-enjoy-art/

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

## Regenerating Wikidata's Sum of All Paintings (SOAP)

The Wikidata SOAP metadata is generated from a Wikidata database
dump. For more details on how to regenerate it, see
[`rust/cli/src/wikidata_dump/README.md`](rust/cli/src/wikidata_dump/README.md).
