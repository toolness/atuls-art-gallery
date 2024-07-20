# Creating the Wikidata "Sum of all paintings" CSV

First, download this ~139 GB file:

```
wget --continue https://dumps.wikimedia.org/wikidatawiki/entities/latest-all.json.gz
```

Next, you will need to index it.  This will create a file that makes it easy to figure out where in the dump each Wikidata entity is:

```
cargo run --release wikidata-index /path/to/latest-all.json.gz
```

Next, you will need to run a SPARQL query that exports a CSV of Wikidata entities that you want to process.  Visit [query.wikidata.org](https://query.wikidata.org) and paste in the following:

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

[Here's an example of what the CSV looks like.](https://docs.google.com/spreadsheets/d/1Gzu3aULsK3WlU5dWwdVrZwOCSZTLa4t8BkExHfmQOrE/edit?usp=sharing)
