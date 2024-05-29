extends Node


var objects: Array[MetObjectRecord] = []


var BASE_RUST_CACHE: String = ProjectSettings.globalize_path("res://") + "../rust/cache/"


func get_rust_cache_path(filename: String) -> String:
	return BASE_RUST_CACHE + filename


class MetObjectRecord:
	var object_id: int
	var title: String
	var date: String
	var width: float
	var height: float
	var small_image: String
	var _cached_small_image: Image

	static func from_json_array(json_array: Variant) -> Array[MetObjectRecord]:
		var results: Array[MetObjectRecord] = []
		for json_data in json_array:
			var o: MetObjectRecord = MetObjectRecord.new()
			o.object_id = json_data.object_id
			o.title = json_data.title
			o.date = json_data.date
			o.width = json_data.width
			o.height = json_data.height
			o.small_image = json_data.small_image
			results.push_back(o)
		return results

	func get_small_image() -> Image:
		if not _cached_small_image:
			_cached_small_image = Image.load_from_file(MetObjects.get_rust_cache_path(small_image))
		return _cached_small_image


func _ready():
	var json_path: String = MetObjects.get_rust_cache_path("_simple-index.json")
	if FileAccess.file_exists(json_path):
		var file = FileAccess.open(json_path, FileAccess.READ)
		var content: String = file.get_as_text()
		var json = JSON.new()
		var error = json.parse(content)
		if error == OK:
			objects = MetObjectRecord.from_json_array(json.data)
			print("Loaded ", objects.size(), " Met objects.")
