extends Node


const ENABLE_MET_OBJECTS := true


class MetObjectRecord:
	var object_id: int
	var title: String
	var date: String
	var width: float
	var height: float
	var small_image: String

	static func from_json(json_data: Variant) -> MetObjectRecord:
		var o := MetObjectRecord.new()
		o.object_id = json_data.object_id
		o.title = json_data.title
		o.date = json_data.date
		o.width = json_data.width / 100.0
		o.height = json_data.height / 100.0
		o.small_image = json_data.small_image
		return o

	func load_small_image() -> Image:
		var image := Image.load_from_file(small_image)
		image.generate_mipmaps()
		return image

	func load_small_image_texture() -> ImageTexture:
		return ImageTexture.create_from_image(load_small_image())


var keyed_met_objects := {}


func try_to_get_next_object(key: String, max_width: float, max_height: float) -> MetObjectRecord:
	if not ENABLE_MET_OBJECTS:
		return null
	if not keyed_met_objects.has(key):
		RustMetObjects.next()
		var obj_str: String
		while not obj_str:
			obj_str = RustMetObjects.poll()
			await get_tree().process_frame
		keyed_met_objects[key] = MetObjectRecord.from_json(JSON.parse_string(obj_str))
	var met_object: MetObjectRecord = keyed_met_objects[key]
	if met_object.width > max_width or met_object.height > max_height:
		# The art is too wide/tall to fit on the wall.
		# TODO: We should remember the object and reuse it in another context if possible.
		return null

	return keyed_met_objects[key]


func _ready() -> void:
	pass
