extends Node


var objects: Array[MetObjectRecord] = []


var ENABLE_MET_OBJECTS = true

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

	# TODO: Uhh, these will never get GC'd if we don't dispose of the MetObjectRecord,
	# which we currently don't...
	var _cached_small_image: Image
	var _cached_small_image_texture: ImageTexture

	static func from_json_array(json_array: Variant) -> Array[MetObjectRecord]:
		var results: Array[MetObjectRecord] = []
		for json_data in json_array:
			var o: MetObjectRecord = MetObjectRecord.new()
			o.object_id = json_data.object_id
			o.title = json_data.title
			o.date = json_data.date
			o.width = json_data.width / 100.0
			o.height = json_data.height / 100.0
			o.small_image = json_data.small_image
			results.push_back(o)
		return results

	func get_small_image() -> Image:
		if not _cached_small_image:
			_cached_small_image = Image.load_from_file(MetObjects.get_rust_cache_path(small_image))
			_cached_small_image.generate_mipmaps()
		return _cached_small_image

	func get_small_image_texture() -> ImageTexture:
		if not _cached_small_image_texture:
			_cached_small_image_texture = ImageTexture.create_from_image(get_small_image())
		return _cached_small_image_texture


func _ready():
	var json_path: String = MetObjects.get_rust_cache_path("_simple-index.json")
	if FileAccess.file_exists(json_path) and ENABLE_MET_OBJECTS:
		var file = FileAccess.open(json_path, FileAccess.READ)
		var content: String = file.get_as_text()
		var json = JSON.new()
		var error = json.parse(content)
		if error == OK:
			objects = MetObjectRecord.from_json_array(json.data)
			print("Loaded ", objects.size(), " Met objects.")
