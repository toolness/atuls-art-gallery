# What this is

This module contains logic that extracts data about the [Sum of all paintings][] from a Wikidata dump file, for inclusion in my art gallery.

It's essentially an extract-transform-load (ETL) pipeline that extracts data from a ~139 GB compressed Wikidata dump file and transforms it into a ~6 mb compressed CSV with ~150k rows that can be imported into the gallery.

[Here's an example of what the final CSV looks like.](https://docs.google.com/spreadsheets/d/1Gzu3aULsK3WlU5dWwdVrZwOCSZTLa4t8BkExHfmQOrE/edit?usp=sharing)

# Why I made this

I originally tried writing SPARQL to extract the data I needed. The query to retrieve just the IDs of all public domain paintings with images completed successfully on [query.wikidata.org][], but as soon as I started asking for more data, such as the title of the painting, the query timed out.

I didn't want to use Wikidata's API to retrieve the additional data, because there were 150k paintings to retrieve data for, and I didn't want to get my IP banned (nor did I want to hammer their servers). The Wikidata documentation recommended downloading a dump file and extracting the data I needed from it, so I decided to combine the two approaches by obtaining the list of IDs via SPARQL, and retrieving all the metadata by processing the dump file.

# Design principles

1. **Don't require decompressing the dump file.** The dump file is about 139 gb gzipped; I don't know how large it is decompressed, but I'm pretty sure it's larger than my hard drive (even if it's not, I want to have room on my hard drive for other things).

2. **Iterating on the data extraction and transformation should be fast.** If I realize I made a typo in my code, or if I want to transform some data in a slightly different way, I shouldn't have to wait very long. Ideally, re-running the ETL pipeline should only take a few seconds.

# How it works

1.  Create an index that maps every wikidata ID to the entity's location in the compressed dump file.

    This makes it easy to figure out where in the dump each Wikidata entity is. This index is essentially one giant vector, indexed by ID (this takes advantage of the fact that the IDs are mostly contiguous positive integers).

    Each entry contains:

      1. The byte offset of the start of the gzip member that includes the entry.

         For context: when decompressed, the Wikidata dump file is just one massive JSON array. But when compressed, it's actually a series of "chunks" of gzipped members. This means that if we only want a particular entity, we only need to decompress the chunk it's in, rather than needing to decompress the entire file.

      2. The offset into the decompressed gzip member where the entry begins.

    This process is heavily reliant on the particular encoding of the dump file; as such, it's a bit fragile. For example, it assumes that every decompressed JSON entity is on its own line, and that the beginning of each serialized entity is formatted in the exact same way (this allows us to extract the entity's ID via a simple string match, rather than by parsing JSON).

    Despite the optimizations, this is the slowest part of the whole process because every gzip member in the dump file needs to be decompressed serially (sadly, the decompression can't be parallelized because we don't know the size of each gzip member until we've fully decompressed it).

2.  Extract a CSV of the IDs of all Wikidata entities representing public domain paintings from [query.wikidata.org][].

    The SPARQL for this is located in the how-to instructions in the next section.

3.  Create a cache containing the uncompressed entities for all the entities (and related entities) in the CSV.

    Because we don't want to have to decompress large amounts of data whenever we re-run the ETL pipeline, and because we only need to look at a relatively small percentage of the dump file's data, we'll keep an uncompressed cache of just the entries we need from the dump file.

    Note that we also need to look up some related entities: for example, every painting has an artist, which is its own Wikidata entity. We need to look these up too, so we can include metadata about the artist in the output CSV.

    The implementation for this is relatively fast because the decompression of the data is parallelized across all the gzip members of the dump file.

4.  Output a CSV containing metadata about the sum of all paintings.

    This is a fairly straightforward transformation. Because all the data we need is cached, it runs very quickly.

# Creating the Wikidata "Sum of all paintings" CSV

This documents how to create a CSV containing the Wikidata [Sum of all paintings][] data.

First, download the entire ~139 GB wikidata dumpfile:

```
wget --continue https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.gz
```

Next, you will need to index it:

```
cargo run --release wikidata-index /path/to/latest-all.json.gz
```

Next, you will need to run a SPARQL query that exports a CSV of Wikidata entities that you want to process. Visit [query.wikidata.org][] and paste in the following:

```sparql
SELECT DISTINCT ?item WHERE {
  # is a painting
  ?item p:P31 ?statement0.
  ?statement0 (ps:P31/(wdt:P279*)) wd:Q3305213.

  # is public domain
  ?item p:P6216 ?statement1.
  ?statement1 (ps:P6216/(wdt:P279*)) wd:Q19652.

  # has an image
  ?item p:P18 ?statement2.
  ?statement2 (ps:P18) _:anyValueP18.

  # is part of an art museum, archival, or bibliographic collection
  ?item p:P195 ?statement3.
  ?statement3 (ps:P195/(wdt:P279*)) _:anyValueP195.

  # has width
  ?item p:P2049 ?statement4.
  ?statement4 (psv:P2049/wikibase:quantityAmount) _:anyValueP2049.

  # has height
  ?item p:P2048 ?statement5.
  ?statement5 (psv:P2048/wikibase:quantityAmount) _:anyValueP2048.

  SERVICE wikibase:label { bd:serviceParam wikibase:language "[AUTO_LANGUAGE]". }
}
```

Click "Download -> CSV file" and save it to your device.

Now you'll need to prepare a query, which caches all the data needed to process your entities, including their dependencies:

```
cargo run --release -- wikidata-prepare /path/to/latest-all.json.gz --output sum.json --csv /path/to/sparql/export.csv
```

Now you can execute the query, which processes all the entities and outputs a CSV:

```
cargo run --release -- wikidata-execute sum.json sum.csv
```

[Sum of all paintings]: https://www.wikidata.org/wiki/Wikidata:WikiProject_sum_of_all_paintings
[query.wikidata.org]: https://query.wikidata.org
